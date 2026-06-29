import { OrchestrationTree } from "@/components/orchestration/OrchestrationTree";
import { useOrchestration } from "@/store/useOrchestration";
import type { ProjectView } from "@/domain";

// The orchestration surface for one project: the live lead→worker agent tree. Owns the read-model
// hook (the only place here that reaches the IPC layer) and hands the built tree to the
// presentational OrchestrationTree. orch-02/03 extend this surface with the todo, scratchpad, and
// timer panels.
export function OrchestrationPane({ project }: { project: ProjectView }) {
  const { tree, error } = useOrchestration(project.id);

  return (
    <section className="flex h-full min-w-0 flex-col bg-background">
      <header className="flex h-9 shrink-0 items-center gap-2.5 border-b bg-sidebar px-3">
        <span className="truncate text-[0.9375rem] font-[550] tracking-[-0.005em]">
          {project.name}
        </span>
        <span className="text-[0.6875rem] text-muted-foreground">Orchestration</span>
      </header>
      {error && <p className="px-3 pt-2 text-xs text-destructive">{error}</p>}
      <div className="min-h-0 flex-1 overflow-auto p-3">
        <OrchestrationTree tree={tree} />
      </div>
    </section>
  );
}
