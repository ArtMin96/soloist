import type { DomainEvent, ProcessView } from "@/domain";

// Pure read-model projection: fold one core event into the process list. Holds no
// business logic — the core stays authoritative; this only mirrors its deltas, which
// keeps it trivially unit-testable.
export function applyEvent(processes: ProcessView[], event: DomainEvent): ProcessView[] {
  switch (event.type) {
    case "ProcessSpawned":
      if (processes.some((process) => process.id === event.id)) return processes;
      return [
        ...processes,
        { id: event.id, kind: event.kind, label: event.label, status: event.status },
      ];
    case "ProcessStatusChanged":
      return processes.map((process) =>
        process.id === event.id ? { ...process, status: event.to } : process,
      );
    case "ProcessRemoved":
      return processes.filter((process) => process.id !== event.id);
  }
}
