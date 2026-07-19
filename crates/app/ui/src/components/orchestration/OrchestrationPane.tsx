import { useState } from "react";
import { OrchestrationTree } from "@/components/orchestration/OrchestrationTree";
import { ScratchpadPanel } from "@/components/orchestration/ScratchpadPanel";
import { TimersPanel } from "@/components/orchestration/TimersPanel";
import { TodoBoard } from "@/components/orchestration/TodoBoard";
import { SegmentedControl } from "@/components/SegmentedControl";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { monogram } from "@/store/projects";
import { useOrchestration } from "@/store/useOrchestration";
import type { Option } from "@/lib/appearance";
import type { ProjectView } from "@/domain";

type View = "agents" | "todos" | "scratchpads" | "timers";

const VIEW_OPTIONS: Option<View>[] = [
  { value: "agents", label: "Agents" },
  { value: "todos", label: "To-dos" },
  { value: "scratchpads", label: "Scratchpads" },
  { value: "timers", label: "Timers" },
];

// The orchestration surface for one project: a live view of the lead→worker agent tree and the
// shared coordination documents (todos, scratchpads, timers). Owns the read-model hook — the only
// place here that reaches IPC — and switches the body between views. Each view is presentational
// over the one snapshot the hook keeps live (snapshot-then-deltas).
export function OrchestrationPane({ project }: { project: ProjectView }) {
  const { tree, agents, todos, scratchpads, timers, error } = useOrchestration(project.id);
  const [view, setView] = useState<View>("agents");

  return (
    <section className="flex h-full min-w-0 flex-col bg-background">
      <header className="flex h-11 shrink-0 items-center gap-2.5 border-b bg-sidebar px-3">
        <Avatar className="size-5">
          {project.icon && <AvatarImage src={project.icon} alt="" />}
          <AvatarFallback>{monogram(project.name)}</AvatarFallback>
        </Avatar>
        <span className="min-w-0 shrink truncate text-[0.9375rem] font-[550] tracking-[var(--tracking-title)]">
          {project.name}
        </span>
        <div className="ml-auto shrink-0">
          <SegmentedControl<View>
            value={view}
            options={VIEW_OPTIONS}
            onChange={setView}
            ariaLabel="Orchestration views"
            counts={{ timers: timers.length }}
          />
        </div>
      </header>
      {error && <p className="px-3 pt-2 text-xs text-destructive">{error}</p>}
      <div className="min-h-0 flex-1 overflow-hidden">
        {view === "agents" && (
          <div className="h-full overflow-auto p-3">
            <OrchestrationTree tree={tree} />
          </div>
        )}
        {view === "todos" && (
          <TodoBoard project={project.id} todos={todos} agents={agents} scratchpads={scratchpads} />
        )}
        {view === "scratchpads" && (
          <ScratchpadPanel project={project.id} scratchpads={scratchpads} />
        )}
        {view === "timers" && <TimersPanel timers={timers} agents={agents} project={project.id} />}
      </div>
    </section>
  );
}
