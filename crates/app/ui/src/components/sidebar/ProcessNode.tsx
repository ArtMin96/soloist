import { Collapsible, CollapsibleContent } from "@/components/ui/collapsible";
import { ProcessRow } from "@/components/sidebar/ProcessRow";
import type { ProcessNode as Node } from "@/store/grouping";
import type { ToggleSet } from "@/store/useToggleSet";

interface ProcessNodeProps {
  node: Node;
  depth: number;
  treeColumn: boolean;
  collapsedLeads: ToggleSet;
  selectedId: number | null;
  onSelect: (id: number) => void;
  onStart: (id: number) => void;
  onStop: (id: number) => void;
  onRestart: (id: number) => void;
  onResume: (id: number) => void;
  onTrust: (id: number) => void;
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
  onStart,
  onStop,
  onRestart,
  onResume,
  onTrust,
}: ProcessNodeProps) {
  const { process, children } = node;
  const hasChildren = children.length > 0;
  const expanded = !collapsedLeads.has(process.id);

  const row = (
    <ProcessRow
      process={process}
      selected={process.id === selectedId}
      onSelect={() => onSelect(process.id)}
      onStart={() => onStart(process.id)}
      onStop={() => onStop(process.id)}
      onRestart={() => onRestart(process.id)}
      onResume={() => onResume(process.id)}
      onTrust={() => onTrust(process.id)}
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
            onStart={onStart}
            onStop={onStop}
            onRestart={onRestart}
            onResume={onResume}
            onTrust={onTrust}
          />
        ))}
      </CollapsibleContent>
    </Collapsible>
  );
}
