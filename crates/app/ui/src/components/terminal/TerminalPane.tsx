import { Bell } from "lucide-react";
import { ProcessControls } from "@/components/ProcessControls";
import { ProcessMeta } from "@/components/sidebar/ProcessMeta";
import { StatusIndicator } from "@/components/StatusIndicator";
import { useTerminal } from "@/components/terminal/useTerminal";
import { useTerminalChrome } from "@/components/terminal/useTerminalChrome";
import { useSignal } from "@/store/signalsContext";
import type { ProcessView } from "@/domain";

interface TerminalPaneProps {
  process: ProcessView;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
  onTrust: () => void;
}

// The interactive PTY for the selected process: a header naming it with its status and
// controls, over the live xterm.js surface. The emulator and IPC live in `useTerminal`;
// this stays presentational.
export function TerminalPane({ process, onStart, onStop, onRestart, onTrust }: TerminalPaneProps) {
  const { hostRef, state } = useTerminal(process);
  const { title, ringing } = useTerminalChrome(process.id);
  const { metrics, attempt } = useSignal(process.id);

  return (
    <section className="flex h-full min-w-0 flex-col bg-background">
      <header className="flex h-9 shrink-0 items-center gap-2.5 border-b bg-sidebar px-3">
        <span className="truncate text-[0.9375rem] font-[550] tracking-[-0.005em]">
          {title ?? process.label}
        </span>
        <StatusIndicator status={process.status} />
        <ProcessMeta
          status={process.status}
          ready={process.ready}
          ports={process.ports}
          metrics={metrics}
          attempt={attempt}
          verbose
        />
        {ringing && (
          <Bell
            aria-label="Terminal bell"
            className="size-3.5 shrink-0 text-primary motion-safe:animate-pulse"
          />
        )}
        <div className="ml-auto">
          <ProcessControls
            status={process.status}
            size="icon-sm"
            onStart={onStart}
            onStop={onStop}
            onRestart={onRestart}
            requiresTrust={process.requires_trust}
            onTrust={onTrust}
          />
        </div>
      </header>
      <div className="relative min-h-0 flex-1">
        <div ref={hostRef} className="absolute inset-2" data-testid="terminal-host" />
        {state === "not-started" && (
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center px-6 text-center">
            <p className="text-sm text-muted-foreground">
              This process hasn’t started yet. Press{" "}
              <span className="font-medium text-foreground">Start</span> to run it.
            </p>
          </div>
        )}
      </div>
    </section>
  );
}
