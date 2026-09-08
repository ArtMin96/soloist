import { ProcessNode } from "@/components/sidebar/ProcessNode";
import { SidebarGroup } from "@/components/sidebar/SidebarGroup";
import type { ProcessActionHandlers } from "@/lib/processActions";
import type { ProcessGroup as Group } from "@/store/grouping";
import type { ToggleSet } from "@/store/useToggleSet";

interface ProcessGroupProps {
  group: Group;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  collapsedLeads: ToggleSet;
  selectedId: number | null;
  onSelect: (id: number) => void;
  handlers: ProcessActionHandlers;
}

// One collapsible subtype group (Agents / Terminals / Commands). The header is a small
// sentence-case label with a count — deliberately not a tracked-uppercase eyebrow. Rows render
// as a lineage tree: a lead's spawned workers nest beneath it, and the disclosure column is
// reserved only while some row in the group actually has workers, so a group with no lineage
// keeps its flat look.
export function ProcessGroup({
  group,
  open,
  onOpenChange,
  collapsedLeads,
  selectedId,
  onSelect,
  handlers,
}: ProcessGroupProps) {
  const treeColumn = group.roots.some((root) => root.children.length > 0);
  return (
    <SidebarGroup
      label={group.label}
      count={group.processes.length}
      open={open}
      onOpenChange={onOpenChange}
    >
      <div role="tree" aria-label={group.label} className="mt-0.5 flex flex-col gap-px pl-1">
        {group.roots.map((root) => (
          <ProcessNode
            key={root.process.id}
            node={root}
            depth={0}
            treeColumn={treeColumn}
            collapsedLeads={collapsedLeads}
            selectedId={selectedId}
            onSelect={onSelect}
            handlers={handlers}
          />
        ))}
      </div>
    </SidebarGroup>
  );
}
