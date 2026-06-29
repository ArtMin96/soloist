import { useState } from "react";
import { OrchestrationTree } from "@/components/orchestration/OrchestrationTree";
import { ScratchpadPanel } from "@/components/orchestration/ScratchpadPanel";
import { TodoBoard } from "@/components/orchestration/TodoBoard";
import { useOrchestration } from "@/store/useOrchestration";
import { cn } from "@/lib/utils";
import type { ProjectView } from "@/domain";

type View = "agents" | "todos" | "scratchpads";

// The orchestration surface for one project: a live view of the lead→worker agent tree and the
// shared coordination documents (scratchpads now; todos and timers in the later panels). Owns the
// read-model hook — the only place here that reaches IPC — and switches the body between views. Each
// view is presentational over the one snapshot the hook keeps live (snapshot-then-deltas).
export function OrchestrationPane({ project }: { project: ProjectView }) {
  const { tree, agents, todos, scratchpads, error } = useOrchestration(project.id);
  const [view, setView] = useState<View>("agents");

  return (
    <section className="flex h-full min-w-0 flex-col bg-background">
      <header className="flex h-9 shrink-0 items-center gap-3 border-b bg-sidebar px-3">
        <span className="shrink-0 truncate text-[0.9375rem] font-[550] tracking-[-0.005em]">
          {project.name}
        </span>
        <nav className="flex items-center gap-2" aria-label="Orchestration views">
          <Tab active={view === "agents"} onClick={() => setView("agents")}>
            Agents
          </Tab>
          <Tab active={view === "todos"} onClick={() => setView("todos")}>
            To-dos
          </Tab>
          <Tab active={view === "scratchpads"} onClick={() => setView("scratchpads")}>
            Scratchpads
          </Tab>
        </nav>
      </header>
      {error && <p className="px-3 pt-2 text-xs text-destructive">{error}</p>}
      <div className="min-h-0 flex-1 overflow-hidden">
        {view === "agents" && (
          <div className="h-full overflow-auto p-3">
            <OrchestrationTree tree={tree} />
          </div>
        )}
        {view === "todos" && <TodoBoard project={project.id} todos={todos} agents={agents} />}
        {view === "scratchpads" && (
          <ScratchpadPanel project={project.id} scratchpads={scratchpads} />
        )}
      </div>
    </section>
  );
}

function Tab({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={active}
      className={cn(
        "relative flex h-9 items-center px-1 text-[0.8125rem] outline-none focus-visible:ring-2 focus-visible:ring-sidebar-ring",
        active ? "text-foreground" : "text-muted-foreground hover:text-foreground",
      )}
    >
      {children}
      {active && <span aria-hidden className="absolute inset-x-0 -bottom-px h-0.5 bg-primary" />}
    </button>
  );
}
