import { useEffect, useRef, useState } from "react";
import { onDomainEvent } from "@/api";

// How long the bell indicator lingers after a process rings the terminal bell.
const BELL_LINGER_MS = 2500;

export interface TerminalChrome {
  /** The latest OSC title the process set, or `null` to fall back to its label. */
  title: string | null;
  /** True briefly after the process rang the terminal bell — an attention cue. */
  ringing: boolean;
}

// Tracks a process's terminal chrome from the low-rate domain events the PTY emits: the OSC
// title it sets and the bell it rings. Scoped to one process id and re-subscribed when the id
// changes, so each pooled pane tracks only its own process. Kept separate from `useTerminal`,
// which owns the high-throughput byte stream and the emulator.
export function useTerminalChrome(id: number): TerminalChrome {
  const [title, setTitle] = useState<string | null>(null);
  const [ringing, setRinging] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    onDomainEvent((event) => {
      if (event.type === "TerminalTitleChanged" && event.id === id) {
        setTitle(event.title);
      } else if (event.type === "TerminalBell" && event.id === id) {
        setRinging(true);
        clearTimeout(timer.current);
        timer.current = setTimeout(() => setRinging(false), BELL_LINGER_MS);
      }
    })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
      clearTimeout(timer.current);
    };
  }, [id]);

  return { title, ringing };
}
