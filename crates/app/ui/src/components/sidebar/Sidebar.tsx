import { ProcessGroup } from "@/components/sidebar/ProcessGroup";
import { groupByKind } from "@/store/grouping";
import { useCollapseState } from "@/store/useCollapseState";
import type { ProcessView } from "@/domain";

interface SidebarProps {
  processes: ProcessView[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  onStart: (id: number) => void;
  onStop: (id: number) => void;
  onRestart: (id: number) => void;
  onTrust: (id: number) => void;
}

// The process tree: the three subtype groups, each collapsible with persisted state. It
// renders the read model and raises intent; the store owns the data and the core owns the
// behaviour.
export function Sidebar({
  processes,
  selectedId,
  onSelect,
  onStart,
  onStop,
  onRestart,
  onTrust,
}: SidebarProps) {
  const groups = groupByKind(processes);
  const [collapsed, setCollapsed] = useCollapseState();

  return (
    <nav
      aria-label="Processes"
      className="flex w-60 shrink-0 flex-col gap-1 overflow-y-auto border-r bg-sidebar p-2"
    >
      {groups.map((group) => (
        <ProcessGroup
          key={group.kind}
          group={group}
          open={!collapsed[group.kind]}
          onOpenChange={(open) => setCollapsed(group.kind, !open)}
          selectedId={selectedId}
          onSelect={onSelect}
          onStart={onStart}
          onStop={onStop}
          onRestart={onRestart}
          onTrust={onTrust}
        />
      ))}
    </nav>
  );
}
