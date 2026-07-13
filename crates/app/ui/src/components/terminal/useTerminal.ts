import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { ptyAttach, ptyDetach, ptyResize, ptyWrite } from "@/api";
import { terminalOptions } from "@/lib/appearance";
import { isActive } from "@/lib/status";
import { activateTerminalRenderer, type RendererHandle } from "@/lib/terminalRenderer";
import { useAppearance } from "@/store/appearanceContext";
import type { ProcessView } from "@/domain";

export type TerminalState = "attaching" | "live" | "not-started";

// Upper bound on bytes coalesced between animation frames. Flushing stops while the pane can't be
// drawn — a background pool pane, or the whole window occluded (WebKitGTK suspends rAF) — so without
// a cap a chatty process would grow the queue without limit; oldest chunks are dropped first. A drop
// leaves the remaining backlog starting mid-stream, so instead of writing that gap the pane marks
// itself to re-attach and replay the core's coherent raw-scrollback ring — an overflow never leaves
// a gap. Sized to hold a full scrollback replay (the core caps raw scrollback at 256 KiB) plus a
// burst of live output.
const PENDING_CAP_BYTES = 512 * 1024;

/** Stable API for in-terminal text search — backed by SearchAddon once mounted. */
export interface TerminalSearch {
  findNext: (query: string) => void;
  findPrevious: (query: string) => void;
  clear: () => void;
}

// Owns one xterm.js instance bound to the selected process: it replays the raw scrollback
// then streams live PTY bytes (coalesced per animation frame so a chatty process can't
// thrash the main thread), routes keystrokes back via `pty_write`, and keeps the PTY
// winsize in step with the pane via `pty_resize`. While its pane is hidden in the keep-alive
// pool the emulator stays mounted but pauses flushing, so a background process does no
// per-frame parsing on the main thread; the backlog drains when the pane is shown — or, if it
// overflowed the cap while hidden, the pane re-attaches and replays the core's scrollback so the
// view stays gap-free. Theme and terminal typography follow the Appearance settings — applied to
// the live emulator on change,
// never recreating it. Detaches deterministically on unmount.
export function useTerminal(process: ProcessView, visible = true) {
  const hostRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const searchRef = useRef<SearchAddon | null>(null);
  const attachedRef = useRef(false);
  // Cancels the current attachment: drops its queued chunks and pending frame, discards its
  // late-arriving bytes, and detaches its backend forwarder by token. Unmount calls it before
  // disposing the emulator, so a superseded attachment can never write to the new terminal
  // or claim its animation frame.
  const cancelAttachRef = useRef<(() => void) | null>(null);
  // Drains the current attachment's flush when its pane becomes visible again, writing the bytes
  // that accumulated (bounded) while it was hidden. Null between attachments.
  const resumeRef = useRef<(() => void) | null>(null);
  // Re-establishes the stream on the live emulator (cancel, reset, re-attach) so the core replays
  // a coherent scrollback. Held in a ref so the attachment's flush can trigger it without a forward
  // reference. Assigned once `reattach` is defined.
  const reattachRef = useRef<(() => void) | null>(null);
  // Set when the bounded backlog overflowed and dropped bytes — while hidden, or while visible but
  // with rAF suspended (an occluded window) — so the remaining backlog is non-contiguous and
  // draining it would splice a gap. The pane then re-attaches and replays the core's coherent
  // scrollback instead. Reset on each (re)attach and on a backend resync.
  const desyncedRef = useRef(false);
  const [state, setState] = useState<TerminalState>("attaching");

  // The latest visibility, read inside the attachment's byte handler so a hidden pool pane stops
  // scheduling per-frame flushes — and the VT parsing they drive — without re-creating the
  // attachment. Bytes still accumulate (bounded) and drain when the pane is shown again.
  const visibleRef = useRef(visible);
  visibleRef.current = visible;

  const { appearance, dark } = useAppearance();
  // The latest appearance, read by the creation effect to seed the emulator without depending
  // on it — a typography change restyles the live terminal (the effect below), never recreates.
  const appearanceRef = useRef({ appearance, dark });
  appearanceRef.current = { appearance, dark };

  const id = process.id;

  const attach = useCallback(() => {
    const term = termRef.current;
    if (!term || attachedRef.current) return;
    attachedRef.current = true;
    setState("attaching");
    desyncedRef.current = false;

    // All coalescing state lives in this closure and dies with this attachment. Bytes from a
    // cancelled attachment are discarded on arrival — never queued, never given a frame — so
    // they cannot swallow the live attachment's flush or write to its emulator. This matters
    // most for a silent process: its scrollback replay is the only content it will ever get.
    let cancelled = false;
    let frame = 0;
    let pending: Uint8Array[] = [];
    let pendingBytes = 0;

    const flush = () => {
      frame = 0;
      if (desyncedRef.current) {
        // The backlog shed a chunk from its middle (a burst outran the cap, e.g. while the
        // window was occluded and rAF suspended); writing it would splice a gap into the
        // emulator. Discard it and re-attach to replay the core's coherent scrollback.
        pending = [];
        pendingBytes = 0;
        reattachRef.current?.();
        return;
      }
      const batch = pending;
      pending = [];
      pendingBytes = 0;
      for (const chunk of batch) term.write(chunk);
    };

    // Called when this attachment's pane is shown: parse the backlog it accrued while hidden.
    resumeRef.current = () => {
      if (cancelled || frame || pending.length === 0) return;
      frame = requestAnimationFrame(flush);
    };

    const attachment = ptyAttach(id, (bytes, resync) => {
      if (cancelled) return;
      if (resync) {
        // The forwarder re-synced from the core's scrollback (the first attach, or after it
        // fell behind): reset the emulator and drop the now-stale backlog, then start from
        // this coherent snapshot. Written on the next frame — or on show, if hidden.
        if (frame) cancelAnimationFrame(frame);
        frame = 0;
        pending = [bytes];
        pendingBytes = bytes.length;
        desyncedRef.current = false;
        term.reset();
        if (visibleRef.current) frame = requestAnimationFrame(flush);
        return;
      }
      pending.push(bytes);
      pendingBytes += bytes.length;
      while (pendingBytes > PENDING_CAP_BYTES && pending.length > 1) {
        pendingBytes -= pending[0].length;
        pending.shift();
        // A drop leaves the backlog non-contiguous; draining it would splice a gap. Mark the
        // pane to re-attach and replay the core's coherent scrollback instead of showing junk.
        desyncedRef.current = true;
      }
      // A hidden pool pane keeps accruing bytes (bounded above) but does not schedule a flush, so it
      // runs no VT parsing on the main thread until it is shown again.
      if (visibleRef.current && !frame) frame = requestAnimationFrame(flush);
    });

    cancelAttachRef.current = () => {
      cancelled = true;
      if (frame) cancelAnimationFrame(frame);
      frame = 0;
      pending = [];
      pendingBytes = 0;
      // Detach by this attachment's own token once it resolves: if a newer attachment has
      // already installed its forwarder, the backend treats the stale token as a no-op — a
      // late detach can never kill the stream the user is looking at.
      void attachment.then((token) => ptyDetach(token)).catch(() => {});
    };

    attachment
      .then(() => {
        if (!cancelled) setState("live");
      })
      .catch(() => {
        if (cancelled) return;
        attachedRef.current = false;
        setState("not-started");
      });
  }, [id]);

  // Re-establishes the PTY stream on the live emulator: cancels the current attachment, clears the
  // stale (gappy) screen, and attaches afresh so the core replays its coherent raw scrollback. Used
  // when a hidden pane's backlog overflowed — draining it would splice in a gap, so the pane instead
  // shows the same current, gap-free view a fresh mount would. Reuses the emulator; only the stream
  // is re-established.
  const reattach = useCallback(() => {
    const term = termRef.current;
    if (!term) return;
    cancelAttachRef.current?.();
    attachedRef.current = false;
    term.reset();
    attach();
  }, [attach]);
  // Expose the latest `reattach` to the attachment's flush without a forward reference. Assigned
  // in a layout effect rather than during render, so a discarded concurrent render cannot leave
  // the ref pointing at an uncommitted callback; the rAF flush only reads it after commit.
  useLayoutEffect(() => {
    reattachRef.current = reattach;
  }, [reattach]);

  // Fit the emulator to its host, then push the resulting winsize to the PTY. Reads the live
  // refs so it can run from any effect — initial layout, a host resize, an appearance change,
  // or a relaunch (a new PTY is spawned at a default winsize and must be re-synced to the pane).
  const syncSize = useCallback(() => {
    const term = termRef.current;
    const fit = fitRef.current;
    if (!term || !fit) return;
    try {
      fit.fit();
    } catch {
      // The host has no measurable size yet; the ResizeObserver fires again once laid out.
      return;
    }
    void ptyResize(id, term.cols, term.rows).catch(() => {});
  }, [id]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    const seed = appearanceRef.current;
    const term = new Terminal({
      cursorBlink: true,
      scrollback: 5000,
      ...terminalOptions(seed.appearance, seed.dark),
    });
    const fit = new FitAddon();
    const search = new SearchAddon();
    term.loadAddon(fit);
    term.loadAddon(search);
    term.open(host);
    termRef.current = term;
    fitRef.current = fit;
    searchRef.current = search;
    attachedRef.current = false;

    // Swap in the GPU (WebGL) renderer now that the terminal is in the DOM. The load is
    // async; until it resolves — and if WebGL is unavailable — the built-in DOM renderer
    // drives the same output, so the upgrade is seamless. The promise can resolve after the
    // effect tears down, so dispose immediately in that case.
    let renderer: RendererHandle | null = null;
    let tornDown = false;
    void activateTerminalRenderer(term).then((handle) => {
      if (tornDown) handle.dispose();
      else renderer = handle;
    });

    const onData = term.onData((input) => void ptyWrite(id, input).catch(() => {}));
    const observer = new ResizeObserver(() => syncSize());
    observer.observe(host);

    attach();
    syncSize();
    term.focus();

    return () => {
      tornDown = true;
      observer.disconnect();
      onData.dispose();
      cancelAttachRef.current?.();
      cancelAttachRef.current = null;
      renderer?.dispose();
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
      searchRef.current = null;
      attachedRef.current = false;
    };
  }, [id, attach, syncSize]);

  // Restyle the live emulator when the theme or terminal typography changes — set on the
  // existing instance, then re-fit since the font metrics moved (so the PTY winsize tracks the
  // new cell size). One assignment per change; no recreation, no per-keystroke work.
  useEffect(() => {
    const term = termRef.current;
    if (!term) return;
    const options = terminalOptions(appearance, dark);
    term.options.fontFamily = options.fontFamily;
    term.options.fontSize = options.fontSize;
    term.options.fontWeight = options.fontWeight;
    term.options.fontWeightBold = options.fontWeightBold;
    term.options.lineHeight = options.lineHeight;
    term.options.letterSpacing = options.letterSpacing;
    term.options.theme = options.theme;
    // Cell metrics moved with the font change, so re-fit and track the PTY winsize.
    syncSize();
  }, [appearance, dark, syncSize]);

  // A process selected before it started has no terminal to attach to; attach once it
  // goes live so its output appears without re-selecting.
  useEffect(() => {
    if (!attachedRef.current && isActive(process.status)) attach();
  }, [process.status, attach]);

  // A relaunch (resume, restart, or start-after-stop) spawns a *new* PTY at a default winsize
  // while the existing emulator and its live stream are reused; re-sync the pane's size to the
  // new PTY once the process is active again, so the agent re-renders to the full pane instead
  // of the spawn default — otherwise its output leaves gaps on the right and bottom.
  useEffect(() => {
    if (isActive(process.status)) syncSize();
  }, [process.status, syncSize]);

  // Refit, drain, and focus when this pane becomes visible again. In the keep-alive pool a hidden
  // terminal stays mounted (display:none) with its stream live but its parsing paused and its host
  // unmeasurable; on show, parse the bytes it accrued while hidden, reconcile any size change that
  // happened off-screen, and take keyboard focus so the user can type immediately after switching.
  useEffect(() => {
    if (!visible) return;
    // Drain what accrued while hidden — unless the bounded backlog overflowed, in which case the
    // drained bytes would start mid-stream (a gap): re-attach and replay the core's scrollback for a
    // coherent, current view instead.
    if (desyncedRef.current) reattach();
    else resumeRef.current?.();
    syncSize();
    termRef.current?.focus();
  }, [visible, reattach, syncSize]);

  // Stable search callbacks — backed by the SearchAddon ref so callers don't need to
  // re-subscribe when the terminal remounts (stable reference, latest addon via ref).
  const findNext = useCallback((query: string) => {
    searchRef.current?.findNext(query, { incremental: true, caseSensitive: false, regex: false });
  }, []);

  const findPrevious = useCallback((query: string) => {
    // No `incremental` here: the addon expands the current selection only for `findNext`; on
    // `findPrevious` it must step to the prior match, so the flag is deliberately omitted.
    searchRef.current?.findPrevious(query, { caseSensitive: false, regex: false });
  }, []);

  const clearSearch = useCallback(() => {
    searchRef.current?.clearDecorations();
  }, []);

  return { hostRef, state, search: { findNext, findPrevious, clear: clearSearch } };
}
