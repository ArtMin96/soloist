import type { ProcessKind, ProcessView } from "@/domain";

// The sidebar's fixed group order: Agents first (most attention-worthy), then Terminals,
// then Commands — matching Solo's process tree.
export const GROUP_ORDER: ProcessKind[] = ["Agent", "Terminal", "Command"];

export const GROUP_LABELS: Record<ProcessKind, string> = {
  Agent: "Agents",
  Terminal: "Terminals",
  Command: "Commands",
};

// The singular noun for one process's kind — used where a single process is labelled (e.g. a
// palette row badge), as opposed to the plural section headings above. One source so no surface
// emits a bare `ProcessKind` token.
export const KIND_LABELS: Record<ProcessKind, string> = {
  Agent: "Agent",
  Terminal: "Terminal",
  Command: "Command",
};

export interface ProcessGroup {
  kind: ProcessKind;
  label: string;
  processes: ProcessView[];
}

// Buckets processes into the three subtype groups, preserving registry order within each
// group and the fixed group order. Pure — no view concerns, unit-testable. The project tier
// (which project owns which processes) is the projects module's concern; this is purely the
// process-kind grouping used within a project node.
export function groupByKind(processes: ProcessView[]): ProcessGroup[] {
  return GROUP_ORDER.map((kind) => ({
    kind,
    label: GROUP_LABELS[kind],
    processes: processes.filter((process) => process.kind === kind),
  }));
}
