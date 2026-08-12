import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { ClipboardAddon } from "@xterm/addon-clipboard";
import type { IDisposable } from "@xterm/xterm";
import { activateTerminalAddons } from "@/components/terminal/terminalAddons";
import {
  copyOnSelect,
  copySelection,
  pasteClipboard,
  type TerminalClipboard,
} from "@/components/terminal/terminalClipboard";
import { oscLinkHandler, webLinksAddon } from "@/components/terminal/terminalLinks";
import { useTerminalSearch } from "@/components/terminal/terminalSearch";
import { ptyAttach, ptyDetach, ptyResize, ptyWrite } from "@/api";
import {
  TERMINAL_FIXED_OPTIONS,
  TERMINAL_SCROLLBACK_LINES,
  terminalOptions,
} from "@/lib/appearance";
import { isActive } from "@/lib/status";
import { activateTerminalRenderer, type RendererHandle } from "@/lib/terminalRenderer";
import { useAppearance } from "@/store/appearanceContext";
import { defaultAppliedTheme } from "@/theme/runtime";
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

// How long to wait before retrying an attach that was rejected while the process is active. The
// backend opens the terminal channel synchronously as a process launches, so an attach to a live
// process resolves; this backoff covers a transient rejection (a brief window, or a race with
// removal) without spinning, so a live pane is never left stranded on the "not-started" overlay
// waiting for a status change that may never arrive.
const ATTACH_RETRY_MS = 120;

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
  // The destination of the link under the pointer, or null when there is none. Fed from the link
  // machinery rather than from the cells on screen, so an OSC 8 hyperlink that displays one thing
  // and points at another is reported by where it goes.
  const [linkTarget, setLinkTarget] = useState<string | null>(null);

  // The latest visibility, read inside the attachment's byte handler so a hidden pool pane stops
  // scheduling per-frame flushes — and the VT parsing they drive — without re-creating the
  // attachment. Bytes still accumulate (bounded) and drain when the pane is shown again.
  const visibleRef = useRef(visible);
  useEffect(() => {
    visibleRef.current = visible;
  }, [visible]);

  const appearanceState = useAppearance();
  const { appearance } = appearanceState;
  const appliedTheme = appearanceState.appliedTheme ?? defaultAppliedTheme(appearanceState.dark);
  // The latest appearance, read by the creation effect to seed the emulator without depending
  // on it — a typography change restyles the live terminal (the effect below), never recreates.
  const appearanceRef = useRef({ appearance, appliedTheme });
  useEffect(() => {
    appearanceRef.current = { appearance, appliedTheme };
  }, [appearance, appliedTheme]);

  const { attach: attachSearch, search } = useTerminalSearch(appliedTheme);

  const id = process.id;

  // Hand the pane keyboard focus, but only if the user wants selecting a process to do that —
  // otherwise the pane is shown and focus stays where it was, so a click into the terminal is what
  // starts typing. Read live rather than captured at mount, so toggling the setting takes effect on
  // the next selection instead of waiting for a remount.
  const focusIfEnabled = useCallback(() => {
    if (appearanceRef.current.appearance.terminal.focus_on_click) termRef.current?.focus();
  }, []);

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
      scrollback: TERMINAL_SCROLLBACK_LINES,
      // Neither of these is part of `terminalOptions`: that projects the appearance document, and
      // every option it returns has to be re-assigned on the live restyle below. A link route and
      // the fixed set are settled when the pane opens and never move again.
      linkHandler: oscLinkHandler(setLinkTarget),
      ...TERMINAL_FIXED_OPTIONS,
      ...terminalOptions(seed.appearance, seed.appliedTheme),
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(webLinksAddon(setLinkTarget));
    // OSC 52 — what lets a program running in the pane put text on the system clipboard and take it
    // back off, which is how a remote editor or a multiplexer yanks into the desktop. The read half
    // is granted deliberately: a supervised program can ask for whatever the user last copied, and
    // the emulator offers no way to allow writes while refusing reads short of replacing its
    // clipboard provider outright. Loaded statically rather than on demand like the two addons
    // below, because it has to be parsing before the first bytes land — the scrollback a pane
    // replays as it opens can already carry the sequence, and a chunk still in flight would miss it.
    term.loadAddon(new ClipboardAddon());
    const detachSearch = attachSearch(term);
    term.open(host);
    termRef.current = term;
    fitRef.current = fit;
    attachedRef.current = false;

    // Swap in the GPU (WebGL) renderer now that the terminal is in the DOM. The load is
    // async; until it resolves — and if WebGL is unavailable — the built-in DOM renderer
    // drives the same output, so the upgrade is seamless. The promise can resolve after the
    // effect tears down, so dispose immediately in that case.
    let renderer: RendererHandle | null = null;
    let tornDown = false;
    void activateTerminalRenderer(term).then((handle) => {
      if (tornDown) {
        handle.dispose();
        return;
      }
      renderer = handle;
      // The GPU renderer re-measures the cell on activation; the first fit below ran against the
      // DOM renderer's estimate. Re-fit so cols/rows track the final cell size — a ResizeObserver
      // can't catch this because the host's size never changed, only the cell metrics did, so
      // without this the pane is left a fraction narrow (a right/bottom gap) until the next resize.
      syncSize();
    });

    // Grapheme widths and inline images arrive the same way, on their own chunks. Both resolve
    // after the pane is already usable, and either can resolve after teardown — dispose then.
    let addons: IDisposable | null = null;
    void activateTerminalAddons(term).then((handle) => {
      if (tornDown) {
        handle.dispose();
        return;
      }
      addons = handle;
    });

    // The monospace web font can resolve after the first fit, shifting the cell width; re-fit once
    // it's ready (same blind spot as the renderer swap above). Resolves immediately once loaded, so
    // later mounts just re-fit on the next microtask. Guarded so it can't touch a torn-down pane.
    if (typeof document !== "undefined" && document.fonts) {
      void document.fonts.ready.then(() => {
        if (!tornDown) syncSize();
      });
    }

    const onData = term.onData((input) => void ptyWrite(id, input).catch(() => {}));
    const onSelection = copyOnSelect(
      term,
      () => appearanceRef.current.appearance.terminal.copy_on_select,
    );
    const observer = new ResizeObserver(() => syncSize());
    observer.observe(host);

    attach();
    syncSize();
    focusIfEnabled();

    return () => {
      tornDown = true;
      observer.disconnect();
      onData.dispose();
      onSelection.dispose();
      cancelAttachRef.current?.();
      cancelAttachRef.current = null;
      detachSearch();
      // Released before the emulator they decorate: both reach back into it as they let go.
      addons?.dispose();
      renderer?.dispose();
      term.dispose();
      // The pointer never leaves a link that is being torn down, so the readout has to be cleared
      // here or it would outlive the emulator it was describing.
      setLinkTarget(null);
      termRef.current = null;
      fitRef.current = null;
      attachedRef.current = false;
    };
  }, [id, attach, syncSize, focusIfEnabled, attachSearch]);

  // Restyle the live emulator when the theme or terminal appearance changes — set on the
  // existing instance, then re-fit since the font metrics moved (so the PTY winsize tracks the
  // new cell size). One assignment per change; no recreation, no per-keystroke work. Every option
  // `terminalOptions` produces must be assigned here, or the setting applies only to panes opened
  // afterwards and silently does nothing to the one the user is looking at.
  useEffect(() => {
    const term = termRef.current;
    if (!term) return;
    const options = terminalOptions(appearance, appliedTheme);
    term.options.fontFamily = options.fontFamily;
    term.options.fontSize = options.fontSize;
    term.options.fontWeight = options.fontWeight;
    term.options.fontWeightBold = options.fontWeightBold;
    term.options.lineHeight = options.lineHeight;
    term.options.letterSpacing = options.letterSpacing;
    term.options.cursorStyle = options.cursorStyle;
    term.options.cursorInactiveStyle = options.cursorInactiveStyle;
    term.options.cursorBlink = options.cursorBlink;
    term.options.theme = options.theme;
    // Cell metrics moved with the font change, so re-fit and track the PTY winsize.
    syncSize();
  }, [appearance, appliedTheme, syncSize]);

  // A process selected before it started has no terminal to attach to; attach once it
  // goes live so its output appears without re-selecting.
  useEffect(() => {
    if (!attachedRef.current && isActive(process.status)) attach();
  }, [process.status, attach]);

  // Drive recovery off the attach *result*, not just status: if an attach was rejected while the
  // process is active, the pane is stranded on the "not-started" overlay with no status change
  // left to re-fire the effect above. Retry after a short backoff while the process stays active;
  // `attach` is a no-op once attached, so a successful attach ends the loop. A resting process
  // (rejected and not active) keeps the overlay and does not retry.
  useEffect(() => {
    if (state !== "not-started" || !isActive(process.status)) return;
    const timer = setTimeout(attach, ATTACH_RETRY_MS);
    return () => clearTimeout(timer);
  }, [state, process.status, attach]);

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
  // happened off-screen, and — when the setting allows — take keyboard focus so the user can type
  // immediately after switching.
  useEffect(() => {
    if (!visible) {
      // Switching away leaves the pointer wherever it was, so a link it was resting on never
      // reports that it was left; without this the pane would come back showing a destination the
      // pointer is no longer on.
      setLinkTarget(null);
      return;
    }
    // Drain what accrued while hidden — unless the bounded backlog overflowed, in which case the
    // drained bytes would start mid-stream (a gap): re-attach and replay the core's scrollback for a
    // coherent, current view instead.
    if (desyncedRef.current) reattach();
    else resumeRef.current?.();
    syncSize();
    focusIfEnabled();
  }, [visible, reattach, syncSize, focusIfEnabled]);

  // Stable clipboard callbacks, backed by the emulator ref for the same reason the search ones are:
  // a caller keeps one reference across remounts.
  const copy = useCallback(() => copySelection(termRef), []);
  const paste = useCallback(() => pasteClipboard(termRef), []);
  const clipboard: TerminalClipboard = { copySelection: copy, paste };

  // Put text in at the cursor as though it had been pasted, so bracketed-paste mode is honored and
  // the bytes take the same route to the PTY that typing does. Inert once the pane is torn down.
  const insert = useCallback((text: string) => termRef.current?.paste(text), []);

  return {
    hostRef,
    state,
    linkTarget,
    search,
    clipboard,
    insert,
  };
}
