// @vitest-environment jsdom
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Channel } from "@tauri-apps/api/core";
import type { ProcessView } from "@/domain";
import { PTY_FRAME_CHUNK, PTY_FRAME_RESYNC } from "@/api";

// jsdom has no emulator surface, so the terminal is a write-recording fake; the hook's real
// attach / coalesce / flush logic is what runs. Instances accumulate so a test can tell the
// StrictMode-disposed terminal from the live one.
const { FakeTerminal } = vi.hoisted(() => {
  class FakeTerminal {
    static instances: FakeTerminal[] = [];
    writes: Array<string | Uint8Array> = [];
    disposed = false;
    options = {};
    cols = 80;
    rows = 24;
    constructor() {
      FakeTerminal.instances.push(this);
    }
    loadAddon() {}
    open() {}
    focus() {}
    dispose() {
      this.disposed = true;
    }
    reset() {
      // A real xterm reset clears the screen and scrollback; mirror that by dropping recorded
      // writes so a test sees only what is replayed after the reset.
      this.writes = [];
    }
    write(data: string | Uint8Array) {
      this.writes.push(data);
    }
    onData() {
      return { dispose() {} };
    }
  }
  return { FakeTerminal };
});

vi.mock("@xterm/xterm", () => ({ Terminal: FakeTerminal }));
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
  activateTerminalRenderer: () => Promise.resolve({ renderer: "dom", dispose() {} }),
}));
vi.mock("@/store/appearanceContext", async () => {
  const { DEFAULT_APPEARANCE } =
    await vi.importActual<typeof import("@/lib/appearance")>("@/lib/appearance");
  return { useAppearance: () => ({ appearance: DEFAULT_APPEARANCE, dark: true }) };
});

import { useTerminal } from "@/components/terminal/useTerminal";

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

function Probe({ process }: { process: ProcessView }) {
  const { hostRef } = useTerminal(process);
  return <div ref={hostRef} />;
}

function VisibilityProbe({ process, visible }: { process: ProcessView; visible: boolean }) {
  const { hostRef } = useTerminal(process, visible);
  return <div ref={hostRef} />;
}

type ChunkChannel = Channel<Uint8Array>;

interface AttachCall {
  channel: ChunkChannel;
  token: number;
}

let attaches: AttachCall[];
let detached: number[];

async function settle(ms = 50) {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, ms));
  });
}

// Sends a framed PTY message the way the backend does: byte 0 tags a live chunk or a
// scrollback-snapshot resync; the rest is the payload. `api.ts` strips the tag before the hook
// sees it, so tests must include it — using the same tag constants the backend mirror defines.
function deliver(call: AttachCall, text: string, kind: "chunk" | "resync" = "chunk") {
  const bytes = new TextEncoder().encode(text);
  const frame = new Uint8Array(bytes.length + 1);
  frame[0] = kind === "resync" ? PTY_FRAME_RESYNC : PTY_FRAME_CHUNK;
  frame.set(bytes, 1);
  call.channel.onmessage(frame);
}

function writtenText(term: InstanceType<typeof FakeTerminal>) {
  const decoder = new TextDecoder();
  return term.writes.map((w) => (typeof w === "string" ? w : decoder.decode(w))).join("");
}

beforeEach(() => {
  FakeTerminal.instances = [];
  attaches = [];
  detached = [];
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
  // jsdom has no frame clock; a macrotask-based stand-in gives the hook's coalescing a
  // firing rAF like a visible window has.
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
    return setTimeout(() => cb(performance.now()), 0) as unknown as number;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => {
    clearTimeout(id as unknown as ReturnType<typeof setTimeout>);
  });
  let nextToken = 0;
  mockIPC((cmd, args) => {
    if (cmd === "pty_attach") {
      const token = ++nextToken;
      attaches.push({ channel: (args as { onChunk: ChunkChannel }).onChunk, token });
      return token;
    }
    if (cmd === "pty_detach") {
      detached.push((args as { token: number }).token);
    }
    return null;
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
  vi.unstubAllGlobals();
});

describe("useTerminal attach lifecycle", () => {
  // StrictMode runs mount → cleanup → mount, so two attachments race for the same pane; the
  // replay must land in the surviving emulator even when the process emits nothing afterwards
  // (an idle agent waiting for input).
  it("renders the scrollback replay after a remount when the process stays silent", async () => {
    render(
      <StrictMode>
        <Probe process={PROCESS} />
      </StrictMode>,
    );
    await settle();
    expect(attaches).toHaveLength(2);

    await act(async () => {
      deliver(attaches[0], "REPLAYED-HISTORY\r\n", "resync");
      deliver(attaches[1], "REPLAYED-HISTORY\r\n", "resync");
    });
    await settle();

    const live = FakeTerminal.instances.find((term) => !term.disposed);
    expect(live).toBeDefined();
    const text = writtenText(live as InstanceType<typeof FakeTerminal>);
    expect(text).toContain("REPLAYED-HISTORY");
    // The superseded attachment's replay must not double up in the surviving terminal.
    expect(text.match(/REPLAYED-HISTORY/g)).toHaveLength(1);
  });

  it("never writes a superseded attachment's bytes into any emulator", async () => {
    render(
      <StrictMode>
        <Probe process={PROCESS} />
      </StrictMode>,
    );
    await settle();
    expect(attaches).toHaveLength(2);

    await act(async () => {
      deliver(attaches[0], "STALE-BYTES\r\n");
    });
    await settle();

    for (const term of FakeTerminal.instances) {
      expect(writtenText(term)).not.toContain("STALE-BYTES");
    }
  });

  it("detaches with the token of its own attachment on unmount", async () => {
    const view = render(
      <StrictMode>
        <Probe process={PROCESS} />
      </StrictMode>,
    );
    await settle();
    view.unmount();
    await settle();

    // Every issued attachment is eventually detached with its own token, so a late detach
    // can never target a newer attachment.
    expect([...detached].sort((a, b) => a - b)).toEqual(attaches.map((a) => a.token));
  });

  it("detaches even when unmounted before the attachment resolves", async () => {
    // A pooled pane can be evicted before its pty_attach promise resolves; the token must still be
    // detached once it arrives, or the forwarder leaks with no token left to clear it.
    const view = render(<Probe process={PROCESS} />);
    // Unmount immediately — the invoke has registered the attachment (so a token exists) but its
    // promise has not resolved yet.
    view.unmount();
    await settle();

    expect(attaches).toHaveLength(1);
    expect(detached).toEqual([attaches[0].token]);
  });
});

describe("useTerminal hidden-pane pause", () => {
  // A hidden pool pane keeps its stream live but must not run the emulator's VT parser on the main
  // thread: bytes accumulate off the frame loop and are parsed only once the pane is shown. This is
  // what keeps a pool of chatty background processes from thrashing the foreground terminal.
  it("does not parse bytes while hidden, then drains the backlog on show", async () => {
    const view = render(<VisibilityProbe process={PROCESS} visible={false} />);
    await settle();
    expect(attaches).toHaveLength(1);

    await act(async () => {
      deliver(attaches[0], "HIDDEN-OUTPUT\r\n");
    });
    await settle();

    const term = FakeTerminal.instances.find((t) => !t.disposed) as InstanceType<
      typeof FakeTerminal
    >;
    // Hidden: the bytes were queued but never written to the emulator (no per-frame parsing).
    expect(writtenText(term)).not.toContain("HIDDEN-OUTPUT");

    await act(async () => {
      view.rerender(<VisibilityProbe process={PROCESS} visible={true} />);
    });
    await settle();
    // Shown: the accumulated backlog drains into the emulator, so no output is lost.
    expect(writtenText(term)).toContain("HIDDEN-OUTPUT");
  });

  // When a chatty background process overflows the bounded backlog while hidden, the oldest queued
  // bytes are dropped, so draining on show would splice a gap into the scrollback. Instead the pane
  // re-attaches and replays the core's coherent scrollback — the same gap-free view a fresh mount
  // shows — rather than draining a discontinuity.
  it("re-attaches and replays scrollback when the backlog overflows while hidden", async () => {
    const view = render(<VisibilityProbe process={PROCESS} visible={false} />);
    await settle();
    expect(attaches).toHaveLength(1);

    // Two chunks that together exceed the 512 KiB backlog cap, so the oldest is evicted while hidden.
    const chunk = "A".repeat(300 * 1024);
    await act(async () => {
      deliver(attaches[0], chunk);
      deliver(attaches[0], chunk);
    });
    await settle();

    await act(async () => {
      view.rerender(<VisibilityProbe process={PROCESS} visible={true} />);
    });
    await settle();

    // Shown after an overflow: the pane re-attached (a second pty_attach) and detached the first,
    // rather than draining the gappy backlog.
    expect(attaches).toHaveLength(2);
    expect(detached).toContain(attaches[0].token);

    // The stale bytes never reached the emulator; the fresh attachment's scrollback replay does.
    const term = FakeTerminal.instances.find((t) => !t.disposed) as InstanceType<
      typeof FakeTerminal
    >;
    await act(async () => {
      deliver(attaches[1], "COHERENT-REPLAY\r\n", "resync");
    });
    await settle();
    const text = writtenText(term);
    expect(text).toContain("COHERENT-REPLAY");
    expect(text).not.toContain("A".repeat(1024));
  });

  it("parses bytes on the frame loop while visible", async () => {
    render(<VisibilityProbe process={PROCESS} visible={true} />);
    await settle();

    await act(async () => {
      deliver(attaches[0], "VISIBLE-OUTPUT\r\n");
    });
    await settle();

    const term = FakeTerminal.instances.find((t) => !t.disposed) as InstanceType<
      typeof FakeTerminal
    >;
    expect(writtenText(term)).toContain("VISIBLE-OUTPUT");
  });

  // The forwarder fell behind and re-synced from the core's scrollback: the resync frame must reset
  // the emulator and replay from that snapshot, discarding the stale bytes that preceded the gap —
  // otherwise the emulator renders a spliced, corrupted stream.
  it("resets and replays from a resync frame", async () => {
    render(<VisibilityProbe process={PROCESS} visible={true} />);
    await settle();

    await act(async () => {
      deliver(attaches[0], "BEFORE-THE-GAP\r\n");
    });
    await settle();

    await act(async () => {
      deliver(attaches[0], "AFTER-RESYNC\r\n", "resync");
    });
    await settle();

    const term = FakeTerminal.instances.find((t) => !t.disposed) as InstanceType<
      typeof FakeTerminal
    >;
    const text = writtenText(term);
    expect(text).toContain("AFTER-RESYNC");
    // The reset dropped the pre-gap bytes, so the coherent snapshot is all that remains.
    expect(text).not.toContain("BEFORE-THE-GAP");
  });

  // A visible pane whose rAF is suspended (an occluded/minimized window) keeps accruing bytes; if
  // the backlog overflows, draining it on restore would splice a gap. The pane must re-attach for a
  // coherent replay, not write the gappy backlog.
  it("re-attaches when the backlog overflows while visible", async () => {
    render(<VisibilityProbe process={PROCESS} visible={true} />);
    await settle();
    expect(attaches).toHaveLength(1);

    // Two chunks that together exceed the 512 KiB cap, delivered before the frame loop can flush.
    const chunk = "B".repeat(300 * 1024);
    await act(async () => {
      deliver(attaches[0], chunk);
      deliver(attaches[0], chunk);
    });
    await settle();

    // The overflow desynced the backlog, so the flush re-attached instead of writing the gap.
    expect(attaches).toHaveLength(2);
    expect(detached).toContain(attaches[0].token);

    const term = FakeTerminal.instances.find((t) => !t.disposed) as InstanceType<
      typeof FakeTerminal
    >;
    await act(async () => {
      deliver(attaches[1], "COHERENT-REPLAY\r\n", "resync");
    });
    await settle();
    const text = writtenText(term);
    expect(text).toContain("COHERENT-REPLAY");
    expect(text).not.toContain("B".repeat(1024));
  });
});
