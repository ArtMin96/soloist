import type { DomainEvent, ProcessView } from "@/domain";

// Pure read-model projection: fold one core event into the process list. Holds no
// business logic — the core stays authoritative; this only mirrors its deltas, which
// keeps it trivially unit-testable. Events that don't change the process list (config
// sync, terminal title/bell, orphans) leave it untouched; their consumers subscribe
// elsewhere.
export function applyEvent(processes: ProcessView[], event: DomainEvent): ProcessView[] {
  switch (event.type) {
    case "ProcessSpawned":
      if (processes.some((process) => process.id === event.id)) return processes;
      return [
        ...processes,
        {
          id: event.id,
          project: event.project,
          kind: event.kind,
          label: event.label,
          status: event.status,
          exit_code: null,
        },
      ];
    case "ProcessStatusChanged":
      return processes.map((process) =>
        process.id === event.id
          ? { ...process, status: event.to, exit_code: event.exit_code }
          : process,
      );
    case "ProcessRemoved":
      return processes.filter((process) => process.id !== event.id);
    case "ConfigChanged":
    case "TerminalTitleChanged":
    case "TerminalBell":
    case "OrphansFound":
      return processes;
  }
}
