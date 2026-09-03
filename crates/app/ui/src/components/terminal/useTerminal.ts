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
import { openPtyStream, type PtyStream } from "@/components/terminal/terminalStream";
import { ptyResize, ptyWrite } from "@/api";
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
  // The current PTY attachment — its coalescing backlog, animation frame, and cancel/resume/desync
  // handle, all owned by one `PtyStream` (`terminalStream.ts`). Null when nothing is attached;
  // replacing it (cancel, reattach, or unmount) drops every reference to the superseded stream's
  // state in one assignment, rather than by convention across several refs.
  const streamRef = useRef<PtyStream | null>(null);
  // Re-establishes the stream on the live emulator (cancel, reset, re-attach) so the core replays
  // a coherent scrollback. Held in a ref so the attachment's flush can trigger it without a forward
  // reference. Assigned once `reattach` is defined.
  const reattachRef = useRef<(() => void) | null>(null);
  const [state, setState] = useState<TerminalState>("attaching");
  // The destination of the link under the pointer, or null when there is none. Fed from the link
  // machinery rather than from the cells on screen, so an OSC 8 hyperlink that displays one thing
  // and points at another is reported by where it goes.
  const [linkTarget, setLinkTarget] = useState<string | null>(null);
  // Switching away leaves the pointer wherever it was, so a link it was resting on never reports
  // that it was left; without this the pane would come back showing a destination the pointer is
  // no longer on. Tracked here, during render, so the clear lands the same render `visible` does.
  const [wasVisible, setWasVisible] = useState(visible);
  if (wasVisible !== visible) {
    setWasVisible(visible);
    if (!visible) setLinkTarget(null);
  }

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

  // Opens a new stream unless one is already attached. `streamRef.current === stream` guards both
  // branches below: if `cancel`/`reattach` has since replaced or cleared the ref, this stream was
  // superseded and its settle must not touch state that belongs to another one.
  const attach = useCallback(() => {
    const term = termRef.current;
    if (!term || streamRef.current) return;
    setState("attaching");

    const stream = openPtyStream({
      id,
      term,
      visible: () => visibleRef.current,
      onDesync: () => reattachRef.current?.(),
    });
    streamRef.current = stream;

    stream.ready
      .then(() => {
        if (streamRef.current === stream) setState("live");
      })
      .catch(() => {
        if (streamRef.current === stream) {
          streamRef.current = null;
          setState("not-started");
        }
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
    streamRef.current?.cancel();
    streamRef.current = null;
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
    streamRef.current = null;

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
      streamRef.current?.cancel();
      streamRef.current = null;
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
    if (!streamRef.current && isActive(process.status)) attach();
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
    if (!visible) return;
    // Drain what accrued while hidden — unless the bounded backlog overflowed, in which case the
    // drained bytes would start mid-stream (a gap): re-attach and replay the core's scrollback for a
    // coherent, current view instead.
    if (streamRef.current?.desynced()) reattach();
    else streamRef.current?.resume();
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
