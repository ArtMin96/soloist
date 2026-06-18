import { ProcessControls } from "@/components/ProcessControls";
import { StatusIndicator } from "@/components/StatusIndicator";
import { useTerminal } from "@/components/terminal/useTerminal";
import type { ProcessView } from "@/domain";

interface TerminalPaneProps {
  process: ProcessView;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
}

// The interactive PTY for the selected process: a header naming it with its status and
// controls, over the live xterm.js surface. The emulator and IPC live in `useTerminal`;
// this stays presentational.
export function TerminalPane({ process, onStart, onStop, onRestart }: TerminalPaneProps) {
  const { hostRef, state } = useTerminal(process);

  return (
    <section className="flex h-full min-w-0 flex-col bg-background">
      <header className="flex h-9 shrink-0 items-center gap-2.5 border-b bg-sidebar px-3">
        <span className="truncate text-[0.9375rem] font-[550] tracking-[-0.005em]">
          {process.label}
        </span>
        <StatusIndicator status={process.status} />
        <div className="ml-auto">
          <ProcessControls
            status={process.status}
            size="icon-sm"
            onStart={onStart}
            onStop={onStop}
            onRestart={onRestart}
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
