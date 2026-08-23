import { Collapsible, CollapsibleContent } from "@/components/ui/collapsible";
import { ProcessRow } from "@/components/sidebar/ProcessRow";
import type { ProcessActionHandlers } from "@/lib/processActions";
import type { ProcessNode as Node } from "@/store/grouping";
import type { ToggleSet } from "@/store/useToggleSet";

interface ProcessNodeProps {
  node: Node;
  depth: number;
  treeColumn: boolean;
  collapsedLeads: ToggleSet;
  selectedId: number | null;
  onSelect: (id: number) => void;
  handlers: ProcessActionHandlers;
}

// One process in a group's lineage tree: its row, with the workers it spawned nested
// (and collapsible) beneath it. Purely presentational — the nested shape arrives as a prop
// and collapse is view state the sidebar owns, keyed by the ephemeral process id (lineage is
// per-run, so it is never persisted).
export function ProcessNode({
  node,
  depth,
  treeColumn,
  collapsedLeads,
  selectedId,
  onSelect,
  handlers,
}: ProcessNodeProps) {
  const { process, children } = node;
  const hasChildren = children.length > 0;
  const expanded = !collapsedLeads.has(process.id);

  const row = (
    <ProcessRow
      process={process}
      selected={process.id === selectedId}
      onSelect={() => onSelect(process.id)}
      handlers={handlers}
      depth={depth}
      treeColumn={treeColumn}
      hasChildren={hasChildren}
      expanded={expanded}
      onToggleExpand={() => collapsedLeads.toggle(process.id)}
    />
  );

  if (!hasChildren) return row;

  return (
    <Collapsible open={expanded}>
      {row}
      <CollapsibleContent
        role="group"
        className="flex flex-col gap-px overflow-hidden data-[state=open]:animate-disclose-down data-[state=closed]:animate-disclose-up"
      >
        {children.map((child) => (
          <ProcessNode
            key={child.process.id}
            node={child}
            depth={depth + 1}
            treeColumn={treeColumn}
            collapsedLeads={collapsedLeads}
            selectedId={selectedId}
            onSelect={onSelect}
            handlers={handlers}
          />
        ))}
      </CollapsibleContent>
    </Collapsible>
  );
}
