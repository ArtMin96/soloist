import type { ProcessKind, ProcessView } from "@/domain";

// The sidebar's fixed group order: Agents first (most attention-worthy), then Terminals,
// then Commands — matching Solo's process tree.
export const GROUP_ORDER: ProcessKind[] = ["Agent", "Terminal", "Command"];

export const GROUP_LABELS: Record<ProcessKind, string> = {
  Agent: "Agents",
  Terminal: "Terminals",
  Command: "Commands",
};

export interface ProcessGroup {
  kind: ProcessKind;
  label: string;
  processes: ProcessView[];
}

// Buckets processes into the three subtype groups, preserving registry order within each
// group and the fixed group order. Pure — no view concerns, unit-testable.
export function groupByKind(processes: ProcessView[]): ProcessGroup[] {
  return GROUP_ORDER.map((kind) => ({
    kind,
    label: GROUP_LABELS[kind],
    processes: processes.filter((process) => process.kind === kind),
  }));
}
