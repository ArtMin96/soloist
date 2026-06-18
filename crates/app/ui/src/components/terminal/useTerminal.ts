import { useCallback, useEffect, useRef, useState } from "react";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { ptyAttach, ptyDetach, ptyResize, ptyWrite } from "@/api";
import { isActive } from "@/lib/status";
import type { ProcessView } from "@/domain";

export type TerminalState = "attaching" | "live" | "not-started";

// Terminal colors that match DESIGN.md's cool-slate surface, kept as plain values so the
// emulator never has to parse the OKLCH design tokens. Program output keeps its own ANSI.
function terminalTheme(dark: boolean) {
  return dark
    ? {
        background: "#1b1e25",
        foreground: "#e6e8ec",
        cursor: "#8ab4f8",
        cursorAccent: "#1b1e25",
        selectionBackground: "#33405a",
      }
    : {
        background: "#fbfbfd",
        foreground: "#23262c",
        cursor: "#3b6fd4",
        cursorAccent: "#fbfbfd",
        selectionBackground: "#cfdcf5",
      };
}

// Owns one xterm.js instance bound to the selected process: it replays the raw scrollback
// then streams live PTY bytes (coalesced per animation frame so a chatty process can't
// thrash the main thread), routes keystrokes back via `pty_write`, and keeps the PTY
// winsize in step with the pane via `pty_resize`. Detaches deterministically on unmount.
export function useTerminal(process: ProcessView) {
  const hostRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const attachedRef = useRef(false);
  const [state, setState] = useState<TerminalState>("attaching");

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

    const term = new Terminal({
      fontFamily: '"Geist Mono Variable", ui-monospace, monospace',
      fontSize: 13,
      lineHeight: 1.15,
      cursorBlink: true,
      scrollback: 5000,
      theme: terminalTheme(document.documentElement.classList.contains("dark")),
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(host);
    termRef.current = term;
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
      attachedRef.current = false;
    };
  }, [id, attach]);

  // A process selected before it started has no terminal to attach to; attach once it
  // goes live so its output appears without re-selecting.
  useEffect(() => {
    if (!attachedRef.current && isActive(process.status)) attach();
  }, [process.status, attach]);

  return { hostRef, state };
}
