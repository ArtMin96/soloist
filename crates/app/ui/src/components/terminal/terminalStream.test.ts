import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { openPtyStream, PENDING_CAP_BYTES, type TerminalWriteTarget } from "./terminalStream";

// No React, no jsdom, no xterm: this is the plain module the backpressure/desync/resync rules were
// pulled into so they could be exercised against a stub terminal and a stub attach promise instead
// of a mounted hook.

/** Records what was written into the pane, mirroring the two calls `openPtyStream` makes. */
function createStubTerminal(): TerminalWriteTarget & { writes: Uint8Array[]; resets: number } {
  return {
    writes: [],
    resets: 0,
    write(data) {
      this.writes.push(data);
    },
    reset() {
      this.resets += 1;
      this.writes = [];
    },
  };
}

function textOf(writes: Uint8Array[]): string {
  const decoder = new TextDecoder();
  return writes.map((chunk) => decoder.decode(chunk)).join("");
}

const encode = (text: string) => new TextEncoder().encode(text);

/** Let every microtask queued so far (a promise's `.then` chain) run before assertions. */
const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

type OnChunk = (bytes: Uint8Array, resync: boolean) => void;

/** A controllable stand-in for `ptyAttach`/`ptyDetach`: the test settles the attach promise and
 * delivers frames on its own schedule, and records every detach token. */
function createStubBackend() {
  let onChunk: OnChunk = () => {};
  let resolveAttach!: (token: number) => void;
  let rejectAttach!: (err: unknown) => void;
  const attachPromise = new Promise<number>((resolve, reject) => {
    resolveAttach = resolve;
    rejectAttach = reject;
  });
  const detachTokens: number[] = [];

  const attach = (_id: number, cb: OnChunk) => {
    onChunk = cb;
    return attachPromise;
  };
  const detach = (token: number) => {
    detachTokens.push(token);
    return Promise.resolve();
  };

  return {
    attach,
    detach,
    detachTokens,
    resolveAttach,
    rejectAttach,
    deliver: (bytes: Uint8Array, resync = false) => onChunk(bytes, resync),
  };
}

let scheduled: FrameRequestCallback | null = null;

/** Runs the one pending animation frame, if any — a no-op if nothing was scheduled. */
function runFrame() {
  const callback = scheduled;
  scheduled = null;
  callback?.(0);
}

beforeEach(() => {
  scheduled = null;
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    scheduled = callback;
    return 1;
  });
  vi.stubGlobal("cancelAnimationFrame", () => {
    scheduled = null;
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("openPtyStream", () => {
  it("queues bytes without writing them while the pane is hidden", () => {
    const term = createStubTerminal();
    const backend = createStubBackend();
    openPtyStream(
      { id: 1, term, visible: () => false, onDesync: vi.fn() },
      backend.attach,
      backend.detach,
    );

    backend.deliver(encode("hidden output"));

    expect(scheduled).toBeNull();
    expect(textOf(term.writes)).toBe("");
  });

  it("desyncs and reports the overflow exactly once when the backlog exceeds the cap", () => {
    const term = createStubTerminal();
    const onDesync = vi.fn();
    const backend = createStubBackend();
    const stream = openPtyStream(
      { id: 1, term, visible: () => true, onDesync },
      backend.attach,
      backend.detach,
    );

    // Two chunks that together exceed the cap, delivered before the frame can flush — the oldest
    // is evicted and the backlog is left non-contiguous.
    const chunk = new Uint8Array(Math.ceil(PENDING_CAP_BYTES / 2) + 1024).fill(65);
    backend.deliver(chunk);
    backend.deliver(chunk);

    expect(stream.desynced()).toBe(true);
    runFrame();

    expect(onDesync).toHaveBeenCalledTimes(1);
    // The desynced backlog is discarded, never written into the pane.
    expect(term.writes).toHaveLength(0);
  });

  it("still detaches by its own token when cancelled before the attach promise resolves", async () => {
    const term = createStubTerminal();
    const backend = createStubBackend();
    const stream = openPtyStream(
      { id: 1, term, visible: () => true, onDesync: vi.fn() },
      backend.attach,
      backend.detach,
    );

    stream.cancel();
    await flush();
    expect(backend.detachTokens).toEqual([]);

    backend.resolveAttach(42);
    await flush();

    expect(backend.detachTokens).toEqual([42]);
  });

  it("discards bytes delivered after cancellation", async () => {
    const term = createStubTerminal();
    const backend = createStubBackend();
    const stream = openPtyStream(
      { id: 1, term, visible: () => true, onDesync: vi.fn() },
      backend.attach,
      backend.detach,
    );
    backend.resolveAttach(1);
    await flush();

    stream.cancel();
    backend.deliver(encode("after cancel"));
    runFrame();

    expect(textOf(term.writes)).toBe("");
  });
});
