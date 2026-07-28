// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { NotificationToasts } from "@/components/NotificationToasts";
import { TOAST_LIFETIME_MS } from "@/lib/notifications";
import type { AttentionKind, ProcessView } from "@/domain";

// The bell is a platform capability jsdom has none of; the tests that care about it watch for the
// tone reaching the speakers rather than for a call.
const oscillators: { started: boolean }[] = [];

class FakeAudioContext {
  readonly currentTime = 0;
  readonly destination = {};
  resume() {
    return Promise.resolve();
  }
  createOscillator() {
    const node = {
      type: "",
      frequency: { value: 0 },
      started: false,
      onended: null,
      connect: (target: unknown) => target,
      disconnect: () => {},
      start: () => {
        node.started = true;
      },
      stop: () => {},
    };
    oscillators.push(node);
    return node;
  }
  createGain() {
    return {
      gain: {
        setValueAtTime: () => {},
        exponentialRampToValueAtTime: () => {},
      },
      connect: (target: unknown) => target,
      disconnect: () => {},
    };
  }
}

const WEB: ProcessView = {
  id: 4,
  project: 1,
  kind: "Command",
  label: "web",
  status: "Crashed",
  exit_code: 1,
  requires_trust: false,
  resumable: false,
  ports: [],
  ready: "Ungated",
};

interface Alert {
  process?: number;
  kind?: AttentionKind;
  title?: string;
  body?: string;
  sound?: string | null;
}

async function raise(alert: Alert = {}) {
  await act(async () => {
    await emit("domain-event", {
      type: "NotificationRaised",
      process: alert.process ?? WEB.id,
      kind: alert.kind ?? "crashed",
      title: alert.title ?? "web crashed",
      body: alert.body ?? "The process exited unexpectedly.",
      sound: alert.sound ?? null,
    });
    // The toast stack lands a tick after the alert does.
    await vi.advanceTimersByTimeAsync(0);
  });
}

async function advance(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

// A dismissed toast leaves the DOM two beats later: one that marks it gone, one that plays its exit
// out. Neither is scheduled until the one before it has rendered, so they are advanced separately.
const EXIT_MS = 400;

async function settleDismissal() {
  await advance(EXIT_MS);
  await advance(EXIT_MS);
}

// Subscribing to the core's event stream is asynchronous, so an alert emitted before it completes
// would simply never arrive. Settle the subscription before any test emits one.
async function mount(processes: ProcessView[] = [WEB]) {
  const selected: number[] = [];
  render(<NotificationToasts processes={processes} onSelectProcess={(id) => selected.push(id)} />);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
  return selected;
}

beforeEach(() => {
  oscillators.length = 0;
  Object.defineProperty(globalThis, "AudioContext", {
    value: FakeAudioContext,
    configurable: true,
  });
  mockIPC(() => undefined, { shouldMockEvents: true });
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  cleanup();
  clearMocks();
  Reflect.deleteProperty(globalThis, "AudioContext");
});

describe("in-app alerts", () => {
  it("shows a raised alert as a toast", async () => {
    await mount();

    await raise();

    expect(screen.getByText("web crashed")).toBeTruthy();
    expect(screen.getByText("The process exited unexpectedly.")).toBeTruthy();
  });

  it("shows the words the alert carries, whatever they say", async () => {
    await mount();

    await raise({ title: "Build finished", body: "3 packages, 0 warnings." });

    expect(screen.getByText("Build finished")).toBeTruthy();
    expect(screen.getByText("3 packages, 0 warnings.")).toBeTruthy();
    // Nothing about the kind leaks into the wording.
    expect(screen.queryByText(/crashed/)).toBeNull();
  });

  it("shows both of two alerts that arrive together, rather than folding them into one", async () => {
    await mount();

    await raise({ title: "web crashed" });
    await raise({ title: "web crashed again" });

    expect(screen.getByText("web crashed")).toBeTruthy();
    expect(screen.getByText("web crashed again")).toBeTruthy();
  });

  it("keeps a crash on screen while an alert that resolved itself goes away", async () => {
    await mount();

    await raise({ kind: "crashed", title: "web crashed" });
    await raise({ kind: "terminal_bell", title: "web rang the bell" });

    await advance(TOAST_LIFETIME_MS);
    await settleDismissal();

    expect(screen.queryByText("web rang the bell")).toBeNull();
    expect(screen.getByText("web crashed")).toBeTruthy();
  });

  it("holds an alert while the pointer is on the stack and gives it the full time back after", async () => {
    await mount();

    await raise({ kind: "terminal_bell", title: "web rang the bell" });

    fireEvent.pointerEnter(screen.getByText("web rang the bell"));
    await advance(TOAST_LIFETIME_MS * 3);
    expect(screen.getByText("web rang the bell")).toBeTruthy();

    fireEvent.pointerLeave(screen.getByText("web rang the bell"));
    await advance(TOAST_LIFETIME_MS - 1);
    expect(screen.getByText("web rang the bell")).toBeTruthy();

    await advance(1);
    await settleDismissal();
    expect(screen.queryByText("web rang the bell")).toBeNull();
  });

  it("takes the user to the process the alert came from", async () => {
    const selected = await mount();

    await raise();
    fireEvent.click(screen.getByText("web crashed"));

    expect(selected).toEqual([WEB.id]);
  });

  it("goes nowhere when that process has already left the stack", async () => {
    const selected = await mount([]);

    await raise();
    fireEvent.click(screen.getByText("web crashed"));

    expect(selected).toEqual([]);
    expect(screen.getByText("web crashed")).toBeTruthy();
  });

  it("starts a crash toast's countdown only once it has been opened", async () => {
    await mount();

    await raise();
    await advance(TOAST_LIFETIME_MS * 3);
    expect(screen.getByText("web crashed")).toBeTruthy();

    fireEvent.click(screen.getByText("web crashed"));
    await advance(TOAST_LIFETIME_MS - 1);
    expect(screen.getByText("web crashed")).toBeTruthy();

    await advance(1);
    await settleDismissal();
    expect(screen.queryByText("web crashed")).toBeNull();
  });

  it("lets the user be rid of an alert that would otherwise stay", async () => {
    await mount();

    await raise();
    fireEvent.click(screen.getByLabelText("Dismiss"));
    await settleDismissal();

    expect(screen.queryByText("web crashed")).toBeNull();
  });

  it("rings when the alert asks for a sound", async () => {
    await mount();

    await raise({ sound: "message-new-instant" });

    expect(oscillators.filter((node) => node.started)).toHaveLength(1);
  });

  it("stays silent when the alert asks for none", async () => {
    await mount();

    await raise({ sound: null });

    expect(oscillators).toHaveLength(0);
  });
});
