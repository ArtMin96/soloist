// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { FakeTerminal } from "@/test/fakeTerminal";
import type { ReactNode } from "react";
import type { DragDropEvent } from "@tauri-apps/api/webview";
import type { ProcessView } from "@/domain";

vi.mock("@xterm/xterm", async () => ({
  Terminal: (await import("@/test/fakeTerminal")).FakeTerminal,
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit() {}
  },
}));
vi.mock("@xterm/addon-search", () => ({
  SearchAddon: class {
    findNext() {}
    findPrevious() {}
    clearDecorations() {}
  },
}));
vi.mock("@xterm/addon-web-links", () => ({ WebLinksAddon: class {} }));
vi.mock("@/lib/terminalRenderer", () => ({
  activateTerminalRenderer: vi.fn().mockResolvedValue({ renderer: "dom", dispose() {} }),
}));
vi.mock("@/store/appearanceContext", () => ({
  useAppearance: () => ({ appearance: DEFAULT_APPEARANCE, dark: true }),
}));

// A stand-in for the OS drag-and-drop stream that behaves like the real one in the respect these
// tests turn on: a subscriber hears every event until it unsubscribes, and nothing after. That is
// what lets "the listener was disposed" be asserted as a listener that has genuinely stopped
// receiving, rather than as a function someone remembered to call.
const { dragDrop } = vi.hoisted(() => ({
  dragDrop: { subscribers: new Set<(event: DragDropEvent) => void>() },
}));
vi.mock("@/lib/fileDrop", () => ({
  onFileDrop: (handler: (event: DragDropEvent) => void) => {
    dragDrop.subscribers.add(handler);
    return Promise.resolve(() => dragDrop.subscribers.delete(handler));
  },
}));

import { FileDropProvider } from "@/store/FileDropProvider";
import { TerminalDropTarget } from "@/components/terminal/TerminalDropTarget";
import { useTerminal } from "@/components/terminal/useTerminal";
import { useTerminalFileDrop } from "@/components/terminal/useTerminalFileDrop";

const PROCESS: ProcessView = {
  id: 7,
  project: 1,
  kind: "Agent",
  label: "assistant",
  status: "Running",
  exit_code: null,
  requires_trust: false,
  resumable: false,
  ports: [],
  ready: "Ungated",
};

const POOLED: ProcessView = { ...PROCESS, id: 8, label: "worker" };

// Mirrors what the pane renders: the emulator's host, and the drop affordance while a drag is over
// it. The host is what the drop position is hit-tested against.
function Probe({
  process = PROCESS,
  visible = true,
}: {
  process?: ProcessView;
  visible?: boolean;
}) {
  const { hostRef, insert } = useTerminal(process, visible);
  const dropping = useTerminalFileDrop(hostRef, insert, visible);
  return (
    <div>
      <div ref={hostRef} data-testid={`host-${process.id}`} />
      {dropping && <TerminalDropTarget />}
    </div>
  );
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
}

// `show` re-renders *under the same provider*. Handing `rerender` a bare tree would tear the
// provider down and mount the panes against the empty default registry, which looks like a pass for
// any test about the registry forgetting something.
async function mount(ui: ReactNode) {
  const view = render(<FileDropProvider>{ui}</FileDropProvider>);
  await settle();
  const show = async (next: ReactNode) => {
    await act(async () => view.rerender(<FileDropProvider>{next}</FileDropProvider>));
  };
  return { ...view, show };
}

/** A drag position, in the physical pixels the window reports it in. */
function pointAt(x: number, y: number): PhysicalPosition {
  return new PhysicalPosition(x, y);
}

/** Deliver one drag-and-drop event the way the window would. */
async function drag(event: DragDropEvent) {
  await act(async () => {
    for (const subscriber of dragDrop.subscribers) subscriber(event);
  });
}

// Give a host a real box. jsdom performs no layout, so an element it has not been told about
// reports a zero-sized box at the origin — which is exactly what a `display: none` pane reports in
// a browser, and is why the hidden pooled pane below needs no special handling.
function boxAt(testId: string, left: number, top: number, width: number, height: number) {
  const host = screen.getByTestId(testId);
  host.getBoundingClientRect = () =>
    ({
      x: left,
      y: top,
      left,
      top,
      width,
      height,
      right: left + width,
      bottom: top + height,
      toJSON: () => ({}),
    }) as DOMRect;
}

/** Everything inserted into the pane's emulator, in order. */
function inserted(): string[] {
  return FakeTerminal.live().pasted;
}

function affordances() {
  return screen.queryAllByTestId("terminal-drop-target");
}

beforeEach(() => {
  FakeTerminal.instances = [];
  dragDrop.subscribers.clear();
  mockIPC((cmd) => (cmd === "pty_attach" ? 1 : null));
});

afterEach(() => {
  cleanup();
  clearMocks();
  vi.unstubAllGlobals();
});

describe("dropping files on a terminal pane", () => {
  it("inserts the dropped file's path, quoted for the shell", async () => {
    await mount(<Probe />);
    boxAt("host-7", 0, 0, 800, 600);

    await drag({ type: "drop", paths: ["/home/dell/shot.png"], position: pointAt(400, 300) });

    expect(inserted()).toEqual(["'/home/dell/shot.png'"]);
  });

  // Dragging a file is not a decision to run anything. Without a trailing newline the path is text
  // the user still has to act on; with one, a drag would have submitted a command line.
  it("appends no newline, so nothing is executed by the drop alone", async () => {
    await mount(<Probe />);
    boxAt("host-7", 0, 0, 800, 600);

    await drag({ type: "drop", paths: ["/home/dell/shot.png"], position: pointAt(400, 300) });

    expect(inserted().join("")).not.toMatch(/[\r\n]/);
  });

  it("inserts every path of a multi-file drop, separated as arguments", async () => {
    await mount(<Probe />);
    boxAt("host-7", 0, 0, 800, 600);

    await drag({
      type: "drop",
      paths: ["/tmp/a.png", "/tmp/b.png", "/tmp/c.png"],
      position: pointAt(10, 10),
    });

    expect(inserted()).toEqual(["'/tmp/a.png' '/tmp/b.png' '/tmp/c.png'"]);
  });

  // A screenshot directory with a space in its name is ordinary on a desktop, and is what an
  // unquoted insertion breaks on. That the quoting is applied on the way to the pane is this test's
  // contract; how each character is spelled inside the quotes belongs to `lib/shellQuote.test.ts`.
  it("quotes a path the shell would otherwise read as two words", async () => {
    await mount(<Probe />);
    boxAt("host-7", 0, 0, 800, 600);

    await drag({
      type: "drop",
      paths: ["/home/dell/My Screenshots/a.png"],
      position: pointAt(10, 10),
    });

    expect(inserted()).toEqual(["'/home/dell/My Screenshots/a.png'"]);
  });

  it("inserts nothing when the drop lands outside the pane", async () => {
    await mount(<Probe />);
    boxAt("host-7", 0, 300, 800, 300);

    await drag({ type: "drop", paths: ["/tmp/a.png"], position: pointAt(400, 100) });

    expect(inserted()).toEqual([]);
  });

  // Up to six terminals stay mounted in the keep-alive pool with only the selected one shown. A
  // drop must reach the pane the user is looking at and no other, or a file dropped on screen turns
  // up in a shell that is not on screen.
  it("never reaches a pane hidden in the keep-alive pool", async () => {
    await mount(
      <>
        <Probe process={POOLED} visible={false} />
        <Probe />
      </>,
    );
    boxAt("host-7", 0, 0, 800, 600);
    // Mounted in tree order, so the hidden pane's emulator is created before the visible one's.
    const [pooled, shown] = FakeTerminal.instances;

    await drag({ type: "drop", paths: ["/tmp/a.png"], position: pointAt(400, 300) });

    expect(pooled.pasted).toEqual([]);
    expect(shown.pasted).toEqual(["'/tmp/a.png'"]);
  });

  // The corner of the window is where a hidden pane would be hit if a box were treated as closed:
  // an unrendered pane reports a zero-sized box at the origin, which contains the origin unless the
  // box's far edges are excluded from it.
  it("never reaches a hidden pane when the drop lands on the window's corner", async () => {
    await mount(
      <>
        <Probe process={POOLED} visible={false} />
        <Probe />
      </>,
    );
    boxAt("host-7", 100, 100, 800, 600);
    const [pooled] = FakeTerminal.instances;

    await drag({ type: "drop", paths: ["/tmp/a.png"], position: pointAt(0, 0) });

    expect(pooled.pasted).toEqual([]);
  });

  // The drop position is in physical pixels while an element's box is in CSS pixels. On a display
  // whose scale factor is not 1 the two disagree, and comparing them unconverted routes the drop to
  // the wrong pane — or to none at all.
  it("lands in the right pane on a HiDPI display", async () => {
    vi.stubGlobal("devicePixelRatio", 2);
    await mount(<Probe />);
    boxAt("host-7", 50, 40, 200, 160);

    await drag({ type: "drop", paths: ["/tmp/a.png"], position: pointAt(300, 200) });

    expect(inserted()).toEqual(["'/tmp/a.png'"]);
  });
});

describe("the drop affordance", () => {
  it("marks the pane a drag is over", async () => {
    await mount(<Probe />);
    boxAt("host-7", 0, 0, 800, 600);

    await drag({ type: "enter", paths: ["/tmp/a.png"], position: pointAt(400, 300) });

    expect(affordances()).toHaveLength(1);
  });

  it("leaves the pane unmarked while the drag is elsewhere in the window", async () => {
    await mount(<Probe />);
    boxAt("host-7", 0, 300, 800, 300);

    await drag({ type: "over", position: pointAt(400, 100) });

    expect(affordances()).toEqual([]);
  });

  it("clears the mark when the drag leaves the window", async () => {
    await mount(<Probe />);
    boxAt("host-7", 0, 0, 800, 600);
    await drag({ type: "enter", paths: ["/tmp/a.png"], position: pointAt(400, 300) });

    await drag({ type: "leave" });

    expect(affordances()).toEqual([]);
  });

  it("clears the mark once the files are dropped", async () => {
    await mount(<Probe />);
    boxAt("host-7", 0, 0, 800, 600);
    await drag({ type: "enter", paths: ["/tmp/a.png"], position: pointAt(400, 300) });

    await drag({ type: "drop", paths: ["/tmp/a.png"], position: pointAt(400, 300) });

    expect(affordances()).toEqual([]);
  });

  // A pooled pane stays mounted while hidden, so nothing re-runs on the way back to re-derive the
  // mark. The drag it was under can end anywhere — over another window, cancelled, dropped
  // elsewhere — and none of those events are addressed to a pane that is no longer on screen, so
  // the mark has to be given up as the pane is hidden or it is still there when the user returns.
  it("does not come back marked after being switched away from mid-drag", async () => {
    const view = await mount(<Probe />);
    boxAt("host-7", 0, 0, 800, 600);
    await drag({ type: "enter", paths: ["/tmp/a.png"], position: pointAt(400, 300) });
    expect(affordances()).toHaveLength(1);

    await view.show(<Probe visible={false} />);
    await view.show(<Probe />);

    expect(affordances()).toEqual([]);
  });

  it("is unmarked the moment the pane is hidden mid-drag", async () => {
    const view = await mount(<Probe />);
    boxAt("host-7", 0, 0, 800, 600);
    await drag({ type: "enter", paths: ["/tmp/a.png"], position: pointAt(400, 300) });

    await view.show(<Probe visible={false} />);

    expect(affordances()).toEqual([]);
  });
});

describe("the window subscription", () => {
  // The subscription is window-wide and lives for as long as the app does, so an undisposed one is
  // a leak nothing later reclaims.
  it("stops listening once the app shell unmounts", async () => {
    const view = await mount(<Probe />);

    view.unmount();
    await settle();

    expect(dragDrop.subscribers.size).toBe(0);
  });

  // Subscribing is asynchronous, so a shell torn down before it resolves has no handle to end the
  // subscription with: the listener arrives after the only code that would have disposed of it has
  // already run, and nothing later reclaims it. Nothing here may await between mounting and
  // unmounting — let the promise settle first and this is the test above, under another name.
  it("stops listening when the shell unmounts before the subscription resolves", async () => {
    const view = render(
      <FileDropProvider>
        <Probe />
      </FileDropProvider>,
    );

    view.unmount();
    await settle();

    expect(dragDrop.subscribers.size).toBe(0);
  });

  it("subscribes once for the whole window, not once per pane", async () => {
    await mount(
      <>
        <Probe />
        <Probe process={POOLED} visible={false} />
      </>,
    );

    expect(dragDrop.subscribers.size).toBe(1);
  });
});
