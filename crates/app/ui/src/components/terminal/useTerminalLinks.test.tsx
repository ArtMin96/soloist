// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { FakeTerminal } from "@/test/fakeTerminal";
import type { ILinkHandler } from "@xterm/xterm";
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
vi.mock("@/lib/terminalRenderer", () => ({
  activateTerminalRenderer: vi.fn().mockResolvedValue({ renderer: "dom", dispose() {} }),
}));
vi.mock("@/store/appearanceContext", () => ({
  useAppearance: () => ({ appearance: DEFAULT_APPEARANCE, dark: true }),
}));

// Records what reached the desktop, so the link tests assert whether a URL was opened rather than
// how the call was written.
const { opened } = vi.hoisted(() => ({ opened: [] as string[] }));
vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(async (url: string) => {
    opened.push(url);
  }),
}));

// The addon takes its handler as a constructor argument, and its own default is `window.open`.
// Holding onto what it was built with is the only way to drive the plain-text route without a real
// emulator to linkify against.
const { webLinks } = vi.hoisted(() => ({
  webLinks: {
    current: null as null | {
      activate: (event: MouseEvent, uri: string) => void;
      hover?: (event: MouseEvent, uri: string) => void;
      leave?: () => void;
    },
  },
}));
vi.mock("@xterm/addon-web-links", () => ({
  WebLinksAddon: class {
    constructor(
      handler: (event: MouseEvent, uri: string) => void,
      options: { hover?: (event: MouseEvent, uri: string) => void; leave?: () => void } = {},
    ) {
      webLinks.current = { activate: handler, hover: options.hover, leave: options.leave };
    }
  },
}));

import { useTerminal } from "@/components/terminal/useTerminal";
import { LinkTarget } from "@/components/terminal/LinkTarget";

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

// Mirrors what the pane renders: the hovered link's destination, or nothing.
function Probe({ visible = true }: { visible?: boolean }) {
  const { hostRef, linkTarget } = useTerminal(PROCESS, visible);
  return (
    <>
      <div ref={hostRef} />
      {linkTarget && <LinkTarget uri={linkTarget} />}
    </>
  );
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
}

/** The handler the emulator was built with — the OSC 8 route. */
function oscRoute(): ILinkHandler {
  const handler = FakeTerminal.live().options.linkHandler as ILinkHandler | null | undefined;
  if (!handler) throw new Error("the emulator was given no link handler");
  return handler;
}

const MOUSE = new MouseEvent("click");
const RANGE = { start: { x: 1, y: 1 }, end: { x: 20, y: 1 } };

function readout() {
  return screen.queryByTestId("terminal-link-target");
}

beforeEach(() => {
  FakeTerminal.instances = [];
  opened.length = 0;
  webLinks.current = null;
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
  mockIPC((cmd) => (cmd === "pty_attach" ? 1 : null));
});

afterEach(() => {
  cleanup();
  clearMocks();
  vi.unstubAllGlobals();
});

describe("OSC 8 hyperlinks", () => {
  // OSC 8 lets a program display one string and point somewhere else entirely. The emulator hands
  // the handler the *destination*, and the readout is fed from that — so the pane answers "where
  // does this go" with the truth even when the cells on screen say otherwise.
  it("reveals the real destination on hover, not the trusted-looking text on screen", async () => {
    render(<Probe />);
    await settle();

    await act(async () => {
      oscRoute().hover?.(MOUSE, "https://evil.example/steal", RANGE);
    });

    expect(readout()?.textContent).toBe("https://evil.example/steal");
    expect(readout()?.textContent).not.toContain("github.com");
  });

  it("shows nothing until a link is hovered", async () => {
    render(<Probe />);
    await settle();

    expect(readout()).toBeNull();
  });

  it("stops reporting a destination once the pointer leaves the link", async () => {
    render(<Probe />);
    await settle();
    await act(async () => {
      oscRoute().hover?.(MOUSE, "https://example.com", RANGE);
    });

    await act(async () => {
      oscRoute().leave?.(MOUSE, "https://example.com", RANGE);
    });

    expect(readout()).toBeNull();
  });

  // A pooled pane stays mounted while hidden, so nothing re-runs creation when it comes back. The
  // pointer never left the link it was resting on, so no `leave` is coming to clear the readout.
  it("forgets the hovered destination when the pane is switched away from", async () => {
    const view = render(<Probe />);
    await settle();
    await act(async () => {
      oscRoute().hover?.(MOUSE, "https://example.com/stale", RANGE);
    });

    await act(async () => view.rerender(<Probe visible={false} />));
    await act(async () => view.rerender(<Probe />));

    expect(readout()).toBeNull();
  });

  it("opens the destination, not whatever the link claimed to be", async () => {
    render(<Probe />);
    await settle();

    await act(async () => {
      oscRoute().activate(MOUSE, "https://real.example/target", RANGE);
    });

    expect(opened).toEqual(["https://real.example/target"]);
  });

  // Defence in depth, not the only gate: while `allowNonHttpProtocols` is unset the emulator drops
  // a non-http OSC 8 link before it is clickable, so this handler is not reachable with a `file:`
  // URI in production. Driving it directly is what proves the guard still holds if that option is
  // ever turned on — the one change that would hand this handler a URI of the program's choosing.
  it("opens nothing when handed a local file URI directly", async () => {
    render(<Probe />);
    await settle();

    await act(async () => {
      oscRoute().activate(MOUSE, "file:///etc/passwd", RANGE);
    });

    expect(opened).toEqual([]);
  });
});

describe("plain-text URLs", () => {
  // The addon is stubbed here, so nothing linkifies: what this asserts is that the handler the
  // addon was constructed with routes to the opener rather than to the addon's own `window.open`
  // default. Which text the real addon is willing to linkify is `terminalLinks.test.ts`.
  it("routes an activated link to the desktop rather than the addon's window.open default", async () => {
    render(<Probe />);
    await settle();

    await act(async () => {
      webLinks.current?.activate(MOUSE, "https://example.com/readme");
    });

    expect(opened).toEqual(["https://example.com/readme"]);
  });

  it("reveals the destination on hover and clears it on leave", async () => {
    render(<Probe />);
    await settle();

    await act(async () => {
      webLinks.current?.hover?.(MOUSE, "https://example.com/readme");
    });
    expect(readout()?.textContent).toBe("https://example.com/readme");

    await act(async () => {
      webLinks.current?.leave?.();
    });
    expect(readout()).toBeNull();
  });
});
