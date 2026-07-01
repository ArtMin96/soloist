import { Badge } from "@/components/ui/badge";
import { CommandItem } from "@/components/ui/command";
import { ProcessIndicator } from "@/components/ProcessIndicator";
import { KIND_LABELS } from "@/store/grouping";
import type { ProcessView } from "@/domain";

// A command-palette row for one process: its status glyph, label, and kind badge. Reused wherever
// a palette lists processes (jump-to, focus-a-terminal), so the row never diverges between them.
// The fuzzy-search `value` carries the label, project name, and kind so any of them matches; the
// trailing id keeps the value unique when two processes share a label.
export function ProcessCommandItem({
  process,
  projectName,
  onSelect,
}: {
  process: ProcessView;
  projectName: string;
  onSelect: () => void;
}) {
  const kind = KIND_LABELS[process.kind];
  return (
    <CommandItem
      value={`${process.label} ${projectName} ${kind} ${process.id}`}
      onSelect={onSelect}
      className="gap-2"
    >
      <ProcessIndicator status={process.status} showLabel={false} />
      <span className="flex-1 truncate">{process.label}</span>
      <Badge variant="muted">{kind}</Badge>
    </CommandItem>
  );
}
