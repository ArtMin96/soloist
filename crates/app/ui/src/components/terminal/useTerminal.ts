import { useCallback, useEffect, useRef, useState } from "react";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { ptyAttach, ptyDetach, ptyResize, ptyWrite } from "@/api";
import { terminalOptions } from "@/lib/appearance";
import { isActive } from "@/lib/status";
import { useAppearance } from "@/store/appearanceContext";
import type { ProcessView } from "@/domain";

export type TerminalState = "attaching" | "live" | "not-started";

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
  const attachedRef = useRef(false);
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
    let frame = 0;
    const flush = () => {
      frame = 0;
      const batch = pending;
      pending = [];
      for (const chunk of batch) term.write(chunk);
    };

    void ptyAttach(id, (bytes) => {
      pending.push(bytes);
      if (!frame) frame = requestAnimationFrame(flush);
    })
      .then(() => setState("live"))
      .catch(() => {
        attachedRef.current = false;
        setState("not-started");
      });
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
    term.loadAddon(fit);
    term.open(host);
    termRef.current = term;
    fitRef.current = fit;
    attachedRef.current = false;

    const sync = () => {
      try {
        fit.fit();
      } catch {
        // The host has no measurable size yet; the ResizeObserver fires again once laid out.
        return;
      }
      void ptyResize(id, term.cols, term.rows).catch(() => {});
    };
    const onData = term.onData((input) => void ptyWrite(id, input).catch(() => {}));
    const observer = new ResizeObserver(() => sync());
    observer.observe(host);

    attach();
    sync();
    term.focus();

    return () => {
      observer.disconnect();
      onData.dispose();
      void ptyDetach().catch(() => {});
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
      attachedRef.current = false;
    };
  }, [id, attach]);

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
    try {
      fitRef.current?.fit();
    } catch {
      return;
    }
    void ptyResize(id, term.cols, term.rows).catch(() => {});
  }, [appearance, dark, id]);

  // A process selected before it started has no terminal to attach to; attach once it
  // goes live so its output appears without re-selecting.
  useEffect(() => {
    if (!attachedRef.current && isActive(process.status)) attach();
  }, [process.status, attach]);

  return { hostRef, state };
}
