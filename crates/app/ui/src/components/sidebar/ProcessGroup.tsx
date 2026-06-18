import { ChevronRight } from "lucide-react";
import { Collapsible } from "radix-ui";
import { ProcessRow } from "@/components/sidebar/ProcessRow";
import type { ProcessGroup as Group } from "@/store/grouping";

interface ProcessGroupProps {
  group: Group;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  selectedId: number | null;
  onSelect: (id: number) => void;
  onStart: (id: number) => void;
  onStop: (id: number) => void;
  onRestart: (id: number) => void;
}

// One collapsible subtype group (Agents / Terminals / Commands). The header is a small
// sentence-case label with a count — deliberately not a tracked-uppercase eyebrow.
export function ProcessGroup({
  group,
  open,
  onOpenChange,
  selectedId,
  onSelect,
  onStart,
  onStop,
  onRestart,
}: ProcessGroupProps) {
  return (
    <Collapsible.Root open={open} onOpenChange={onOpenChange} className="select-none">
      <Collapsible.Trigger className="group/trigger flex w-full items-center gap-1.5 rounded-sm px-1 py-1 text-left outline-none hover:bg-sidebar-accent focus-visible:ring-2 focus-visible:ring-sidebar-ring">
        <ChevronRight
          aria-hidden
          className="size-3 text-muted-foreground transition-transform group-data-[state=open]/trigger:rotate-90"
        />
        <span className="text-[0.6875rem] font-[550] tracking-[0.01em] text-muted-foreground">
          {group.label}
        </span>
        <span className="ml-auto pr-1 font-mono text-[0.6875rem] text-muted-foreground/70">
          {group.processes.length}
        </span>
      </Collapsible.Trigger>
      <Collapsible.Content>
        <div className="mt-0.5 flex flex-col gap-px pl-1">
          {group.processes.length === 0 ? (
            <p className="px-2.5 py-1 text-xs text-muted-foreground/60">
              No {group.label.toLowerCase()}
            </p>
          ) : (
            group.processes.map((process) => (
              <ProcessRow
                key={process.id}
                process={process}
                selected={process.id === selectedId}
                onSelect={() => onSelect(process.id)}
                onStart={() => onStart(process.id)}
                onStop={() => onStop(process.id)}
                onRestart={() => onRestart(process.id)}
              />
            ))
          )}
        </div>
      </Collapsible.Content>
    </Collapsible.Root>
  );
}
