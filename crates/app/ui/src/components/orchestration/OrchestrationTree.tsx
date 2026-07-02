import { OrchestrationNode } from "@/components/orchestration/OrchestrationNode";
import { useToggleSet } from "@/store/useToggleSet";
import type { OrchestrationTreeNode } from "@/store/orchestrationTree";

interface OrchestrationTreeProps {
  tree: OrchestrationTreeNode[];
}

// The agent lineage tree: lead agents with the workers they spawned nested beneath, each row a
// live status/activity glyph + name + kind. Presentational — the nested shape arrives as a prop
// (built by the read-model hook). Collapse is local view state, keyed by the ephemeral process id
// (lineage is per-run, so it is not persisted across launches). A project with no agents shows a
// quiet empty state rather than a blank panel.
export function OrchestrationTree({ tree }: OrchestrationTreeProps) {
  const collapsed = useToggleSet();

  if (tree.length === 0) {
    return (
      <p className="px-1 py-2 text-[0.8125rem] text-muted-foreground">
        No agents in this project yet. Launch an agent to see its lineage here.
      </p>
    );
  }

  return (
    <div role="tree" aria-label="Agent lineage" className="flex flex-col gap-px">
      {tree.map((node) => (
        <OrchestrationNode
          key={node.id}
          node={node}
          depth={0}
          isCollapsed={collapsed.has}
          onToggle={collapsed.toggle}
        />
      ))}
    </div>
  );
}
