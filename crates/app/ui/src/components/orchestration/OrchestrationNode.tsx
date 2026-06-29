import { ChevronRight } from "lucide-react";
import { Collapsible } from "radix-ui";
import { ProcessIndicator } from "@/components/ProcessIndicator";
import { cn } from "@/lib/utils";
import type { OrchestrationTreeNode } from "@/store/orchestrationTree";

interface OrchestrationNodeProps {
  node: OrchestrationTreeNode;
  depth: number;
  isCollapsed: (id: number) => boolean;
  onToggle: (id: number) => void;
}

// One agent in the orchestration tree: its live status/activity glyph, name, and kind, with the
// workers it spawned nested (and collapsible) beneath it. Purely presentational — the lineage
// shape and live state arrive as props; collapse is view state the tree owns. A lead's disclosure
// chevron is the one interactive element; the tree observes, it does not act.
export function OrchestrationNode({ node, depth, isCollapsed, onToggle }: OrchestrationNodeProps) {
  const hasChildren = node.children.length > 0;
  const open = !isCollapsed(node.id);
  // Indent each level so lineage reads as a tree; the chevron column keeps a childless row
  // aligned with its disclosed siblings.
  const indent = { paddingLeft: `${depth * 16 + 4}px` };

  const row = (
    <div
      role="treeitem"
      aria-level={depth + 1}
      aria-expanded={hasChildren ? open : undefined}
      data-process-id={node.id}
      className="flex h-7 items-center gap-2 rounded-sm pr-2 text-[0.8125rem]"
      style={indent}
    >
      {hasChildren ? (
        <Collapsible.Trigger
          aria-label={open ? `Collapse ${node.label}'s workers` : `Expand ${node.label}'s workers`}
          className="flex size-4 shrink-0 items-center justify-center rounded-sm text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-sidebar-ring"
        >
          <ChevronRight
            aria-hidden
            className={cn("size-3 transition-transform", open && "rotate-90")}
          />
        </Collapsible.Trigger>
      ) : (
        <span aria-hidden className="size-4 shrink-0" />
      )}
      <ProcessIndicator
        status={node.status}
        activity={node.activity ?? undefined}
        showLabel={false}
      />
      <span className="min-w-0 flex-1 truncate text-foreground">{node.label}</span>
      <span className="shrink-0 text-[0.6875rem] text-muted-foreground/70">{node.kind}</span>
    </div>
  );

  if (!hasChildren) return row;

  return (
    <Collapsible.Root open={open} onOpenChange={() => onToggle(node.id)}>
      {row}
      <Collapsible.Content role="group">
        {node.children.map((child) => (
          <OrchestrationNode
            key={child.id}
            node={child}
            depth={depth + 1}
            isCollapsed={isCollapsed}
            onToggle={onToggle}
          />
        ))}
      </Collapsible.Content>
    </Collapsible.Root>
  );
}
