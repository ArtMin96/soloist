import { useCallback, useEffect, useRef, useState } from "react";
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

/** Stable API for in-terminal text search — backed by SearchAddon once mounted. */
export interface TerminalSearch {
  findNext: (query: string) => void;
  findPrevious: (query: string) => void;
  clear: () => void;
}

// Owns one xterm.js instance bound to the selected process: it replays the raw scrollback
// then streams live PTY bytes (coalesced per animation frame so a chatty process can't
// thrash the main thread), routes keystrokes back via `pty_write`, and keeps the PTY
// winsize in step with the pane via `pty_resize`. Theme and terminal typography follow the
// Appearance settings — applied to the live emulator on change, never recreating it.
// Detaches deterministically on unmount.
export function useTerminal(process: ProcessView) {
  const hostRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const searchRef = useRef<SearchAddon | null>(null);
  const attachedRef = useRef(false);
  // The id of the pending coalescing frame, so unmount can cancel it before disposing the terminal
  // (otherwise a frame scheduled in the last ~16 ms would write to a disposed emulator).
  const frameRef = useRef(0);
  const [state, setState] = useState<TerminalState>("attaching");

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

    let pending: Uint8Array[] = [];
    const flush = () => {
      frameRef.current = 0;
      // The effect's cleanup nulls termRef before this frame could run after a dispose; bail so a
      // late frame never writes to a disposed emulator.
      if (termRef.current !== term) return;
      const batch = pending;
      pending = [];
      for (const chunk of batch) term.write(chunk);
    };

    void ptyAttach(id, (bytes) => {
      pending.push(bytes);
      if (!frameRef.current) frameRef.current = requestAnimationFrame(flush);
    })
      .then(() => setState("live"))
      .catch(() => {
        attachedRef.current = false;
        setState("not-started");
      });
  }, [id]);

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
      if (frameRef.current) {
        cancelAnimationFrame(frameRef.current);
        frameRef.current = 0;
      }
      void ptyDetach().catch(() => {});
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

  // Stable search callbacks — backed by the SearchAddon ref so callers don't need to
  // re-subscribe when the terminal remounts (stable reference, latest addon via ref).
  const findNext = useCallback((query: string) => {
    searchRef.current?.findNext(query, { incremental: true, caseSensitive: false, regex: false });
  }, []);

  const findPrevious = useCallback((query: string) => {
    searchRef.current?.findPrevious(query, {
      incremental: true,
      caseSensitive: false,
      regex: false,
    });
  }, []);

  const clearSearch = useCallback(() => {
    searchRef.current?.clearDecorations();
  }, []);

  return { hostRef, state, search: { findNext, findPrevious, clear: clearSearch } };
}
