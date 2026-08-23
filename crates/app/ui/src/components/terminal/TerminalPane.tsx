import { useCallback, useRef, useState } from "react";
import { Bell } from "lucide-react";
import { ProcessControls } from "@/components/ProcessControls";
import { ProcessIndicator } from "@/components/ProcessIndicator";
import { ProcessMeta } from "@/components/sidebar/ProcessMeta";
import { FindBar } from "@/components/terminal/FindBar";
import { LinkTarget } from "@/components/terminal/LinkTarget";
import { TerminalDropTarget } from "@/components/terminal/TerminalDropTarget";
import { useTerminal } from "@/components/terminal/useTerminal";
import { useTerminalChrome } from "@/components/terminal/useTerminalChrome";
import { useTerminalFileDrop } from "@/components/terminal/useTerminalFileDrop";
import { useTerminalHotkeys } from "@/components/terminal/useTerminalHotkeys";
import type { ProcessActionHandlers } from "@/lib/processActions";
import { terminalColors } from "@/lib/terminalPalette";
import { useAppearance } from "@/store/appearanceContext";
import { useSignal } from "@/store/signalsContext";
import { cn } from "@/lib/utils";
import type { ProcessView } from "@/domain";

// A stable empty default so an unspecified `processes` keeps the same identity across renders —
// a fresh `[]` each render would defeat memoized consumers and re-run the hotkey subscription.
const NO_PROCESSES: ProcessView[] = [];

interface TerminalPaneProps {
  process: ProcessView;
  /** Whether this pane is the currently-visible one; hidden pool panes stay mounted (display:none). */
  visible?: boolean;
  /** Ordered process list for Ctrl+↑/↓ navigation. */
  processes?: ProcessView[];
  /** Called when a terminal-scope nav shortcut selects a different process. */
  onSelectProcess?: (id: number) => void;
  handlers: ProcessActionHandlers;
}

// The interactive PTY for the selected process: a header naming it with its status and
// controls, over the live xterm.js surface. The emulator and IPC live in `useTerminal`;
// this stays presentational.
export function TerminalPane({
  process,
  visible = true,
  processes = NO_PROCESSES,
  onSelectProcess,
  handlers,
}: TerminalPaneProps) {
  const sectionRef = useRef<HTMLElement>(null);
  const { hostRef, state, linkTarget, search, clipboard, insert } = useTerminal(process, visible);
  const dropping = useTerminalFileDrop(hostRef, insert, visible);
  const { title, ringing } = useTerminalChrome(process.id);
  const { metrics, restart, activity } = useSignal(process.id);
  const { appliedTheme } = useAppearance();
  // The emulator's own surface color, so the host bleeds it to every edge — the sub-cell
  // remainder and scrollbar gutter read as terminal margin, not an app-colored frame.
  const surface = terminalColors(appliedTheme).background;

  const [findOpen, setFindOpen] = useState(false);
  const [findQuery, setFindQuery] = useState("");

  const openFind = useCallback(() => setFindOpen(true), []);

  const closeFind = useCallback(() => {
    setFindOpen(false);
    setFindQuery("");
    search.clear();
  }, [search]);

  const handleFindChange = useCallback(
    (query: string) => {
      setFindQuery(query);
      if (query) search.findNext(query);
      else search.clear();
    },
    [search],
  );

  useTerminalHotkeys({
    containerRef: sectionRef,
    processes,
    processId: process.id,
    onSelectProcess,
    onOpenSearch: openFind,
    clipboard,
  });

  return (
    <section
      ref={sectionRef}
      className={cn("flex h-full min-w-0 flex-col bg-background", !visible && "hidden")}
    >
      <header className="flex h-11 shrink-0 items-center gap-2.5 border-b bg-sidebar px-3">
        <span className="truncate text-[0.9375rem] font-[550] tracking-[-0.005em]">
          {title ?? process.label}
        </span>
        <ProcessIndicator status={process.status} activity={activity} />
        <ProcessMeta
          status={process.status}
          ready={process.ready}
          ports={process.ports}
          metrics={metrics}
          restart={restart}
          verbose
        />
        {ringing && (
          <Bell
            aria-label="Terminal bell"
            className="size-3.5 shrink-0 text-primary motion-safe:animate-pulse"
          />
        )}
        <div className="ml-auto">
          <ProcessControls process={process} handlers={handlers} size="icon-sm" />
        </div>
      </header>
      <div className="relative min-h-0 flex-1">
        {findOpen && (
          <FindBar
            query={findQuery}
            matches={search.matches}
            onChange={handleFindChange}
            onFindNext={() => findQuery && search.findNext(findQuery)}
            onFindPrevious={() => findQuery && search.findPrevious(findQuery)}
            onClose={closeFind}
          />
        )}
        <div
          ref={hostRef}
          className="absolute inset-0 pt-1.5 pl-2"
          style={{ backgroundColor: surface }}
          data-testid="terminal-host"
        />
        {dropping && <TerminalDropTarget />}
        {linkTarget && <LinkTarget uri={linkTarget} />}
        {state === "not-started" && (
          <div className="pointer-events-none absolute inset-0 flex animate-in items-center justify-center px-6 text-center fade-in-0 duration-[var(--dur-sheet)]">
            <p className="max-w-sm text-sm text-pretty text-muted-foreground">
              {process.resumable ? (
                <>
                  This agent isn't running. Press{" "}
                  <span className="font-medium text-foreground">Resume last session</span> to
                  continue where it left off, or{" "}
                  <span className="font-medium text-foreground">Start</span> to begin fresh.
                </>
              ) : (
                <>
                  This process hasn't started yet. Press{" "}
                  <span className="font-medium text-foreground">Start</span> to run it.
                </>
              )}
            </p>
          </div>
        )}
      </div>
    </section>
  );
}
