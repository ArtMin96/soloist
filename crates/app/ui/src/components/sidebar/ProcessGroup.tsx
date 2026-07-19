import { ChevronRight } from "lucide-react";
import { Collapsible } from "radix-ui";
import { ProcessNode } from "@/components/sidebar/ProcessNode";
import type { ProcessGroup as Group } from "@/store/grouping";
import type { ToggleSet } from "@/store/useToggleSet";

interface ProcessGroupProps {
  group: Group;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  collapsedLeads: ToggleSet;
  selectedId: number | null;
  onSelect: (id: number) => void;
  onStart: (id: number) => void;
  onStop: (id: number) => void;
  onRestart: (id: number) => void;
  onResume: (id: number) => void;
  onTrust: (id: number) => void;
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
  onStart,
  onStop,
  onRestart,
  onResume,
  onTrust,
}: ProcessGroupProps) {
  const treeColumn = group.roots.some((root) => root.children.length > 0);
  return (
    <Collapsible.Root open={open} onOpenChange={onOpenChange} className="select-none">
      <Collapsible.Trigger className="group/trigger flex w-full items-center gap-1.5 rounded-sm px-1 py-1 text-left outline-none hover:bg-sidebar-accent focus-visible:ring-2 focus-visible:ring-sidebar-ring">
        <ChevronRight
          aria-hidden
          className="size-3 text-muted-foreground transition-transform duration-[var(--dur-control)] ease-spring-settle group-data-[state=open]/trigger:rotate-90"
        />
        <span className="text-[0.6875rem] font-[550] tracking-[0.01em] text-muted-foreground">
          {group.label}
        </span>
        <span className="ml-auto pr-1 font-mono text-[0.6875rem] tabular-nums text-muted-foreground">
          {group.processes.length}
        </span>
      </Collapsible.Trigger>
      <Collapsible.Content className="overflow-hidden data-[state=open]:animate-disclose-down data-[state=closed]:animate-disclose-up">
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
              onStart={onStart}
              onStop={onStop}
              onRestart={onRestart}
              onResume={onResume}
              onTrust={onTrust}
            />
          ))}
        </div>
      </Collapsible.Content>
    </Collapsible.Root>
  );
}
