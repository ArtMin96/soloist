import { ptyAttach, ptyDetach } from "@/api";

// Upper bound on bytes coalesced between animation frames. Flushing stops while the pane can't be
// drawn — a background pool pane, or the whole window occluded (WebKitGTK suspends rAF) — so without
// a cap a chatty process would grow the queue without limit; oldest chunks are dropped first. A drop
// leaves the remaining backlog starting mid-stream, so instead of writing that gap the pane marks
// itself to re-attach and replay the core's coherent raw-scrollback ring — an overflow never leaves
// a gap. Sized to hold a full scrollback replay (the core caps raw scrollback at 256 KiB) plus a
// burst of live output.
export const PENDING_CAP_BYTES = 512 * 1024;

// The slice of the xterm.js `Terminal` API a stream needs — narrower than the real class so the
// backpressure/desync rules below are exercised in a test with a two-method stub, never a fake
// emulator.
export interface TerminalWriteTarget {
  write(data: Uint8Array): void;
  reset(): void;
}

export interface PtyStreamOptions {
  /** The process this stream attaches to. */
  id: number;
  /** The live emulator this stream's bytes are written into. */
  term: TerminalWriteTarget;
  /** Read live so a hidden pane stops scheduling per-frame flushes without recreating the stream. */
  visible: () => boolean;
  /** Fired from a flush that found the backlog desynced, so the caller can re-attach for a
   * coherent replay. */
  onDesync: () => void;
}

// One PTY attachment: the backend forwarder, the coalescing backlog it feeds, and the animation
// frame that drains it into the emulator. All of it lives in this handle and dies with it —
// cancelling one instance can never reach into a stream opened after it, because the two never
// share state.
export interface PtyStream {
  /** Resolves once the backend has attached; rejects if it refused (a launch race or removal). */
  ready: Promise<void>;
  /**
   * Cancels the stream: drops the queued backlog and pending frame, discards any bytes that
   * arrive afterward, and detaches the backend forwarder by this stream's own token once the
   * attach settles — a token issued to a superseded stream can never cancel the one that
   * replaced it. Safe to call more than once.
   */
  cancel(): void;
  /** Drains the backlog that accrued while the pane was hidden. No-op if nothing is queued. */
  resume(): void;
  /** True once the backlog has dropped a chunk from its middle — draining it would splice a gap. */
  desynced(): boolean;
}

// Opens one PTY attachment and returns the handle that owns its lifecycle. `attach`/`detach` are
// parameters (defaulting to the real `@/api` pair) so the coalescing, cap, and desync rules below
// run against a stub promise and a stub terminal, with no React, jsdom, or xterm involved.
export function openPtyStream(
  { id, term, visible, onDesync }: PtyStreamOptions,
  attach: typeof ptyAttach = ptyAttach,
  detach: typeof ptyDetach = ptyDetach,
): PtyStream {
  // All coalescing state lives in this closure and dies with this stream. Bytes from a cancelled
  // stream are discarded on arrival — never queued, never given a frame — so they cannot swallow
  // a later stream's flush or write to its emulator. This matters most for a silent process: its
  // scrollback replay is the only content it will ever get.
  let cancelled = false;
  let frame = 0;
  let pending: Uint8Array[] = [];
  let pendingBytes = 0;
  let desynced = false;

  const flush = () => {
    frame = 0;
    if (desynced) {
      // The backlog shed a chunk from its middle (a burst outran the cap, e.g. while the window
      // was occluded and rAF suspended); writing it would splice a gap into the emulator. Discard
      // it and tell the caller to re-attach and replay the core's coherent scrollback.
      pending = [];
      pendingBytes = 0;
      onDesync();
      return;
    }
    const batch = pending;
    pending = [];
    pendingBytes = 0;
    for (const chunk of batch) term.write(chunk);
  };

  const attachment = attach(id, (bytes, resync) => {
    if (cancelled) return;
    if (resync) {
      // The forwarder re-synced from the core's scrollback (the first attach, or after it fell
      // behind): reset the emulator and drop the now-stale backlog, then start from this coherent
      // snapshot. Written on the next frame — or on resume, if hidden.
      if (frame) cancelAnimationFrame(frame);
      frame = 0;
      pending = [bytes];
      pendingBytes = bytes.length;
      desynced = false;
      term.reset();
      if (visible()) frame = requestAnimationFrame(flush);
      return;
    }
    pending.push(bytes);
    pendingBytes += bytes.length;
    while (pendingBytes > PENDING_CAP_BYTES && pending.length > 1) {
      pendingBytes -= pending[0].length;
      pending.shift();
      desynced = true;
    }
    // A hidden pool pane keeps accruing bytes (bounded above) but does not schedule a flush, so it
    // runs no VT parsing on the main thread until it is shown again.
    if (visible() && !frame) frame = requestAnimationFrame(flush);
  });

  return {
    ready: attachment.then(() => undefined),
    cancel() {
      if (cancelled) return;
      cancelled = true;
      if (frame) cancelAnimationFrame(frame);
      frame = 0;
      pending = [];
      pendingBytes = 0;
      // Detach by this stream's own token once it resolves: if a newer stream has already
      // installed its forwarder, the backend treats the stale token as a no-op — a late detach
      // can never kill the stream the user is looking at.
      void attachment.then((token) => detach(token)).catch(() => {});
    },
    resume() {
      if (cancelled || frame || pending.length === 0) return;
      frame = requestAnimationFrame(flush);
    },
    desynced() {
      return desynced;
    },
  };
}
