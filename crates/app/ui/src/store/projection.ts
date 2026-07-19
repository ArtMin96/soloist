import type { DomainEvent, ProcessView } from "@/domain";

// Pure read-model projection: fold one core event into the process list. Holds no
// business logic — the core stays authoritative; this only mirrors its deltas, which
// keeps it trivially unit-testable. Events that don't change the process list (config
// sync, terminal title/bell, orphans, restart notices, metrics ticks, agent activity, and
// the coordination deltas — todo/timer/lease/scratchpad/kv/template changed) leave it untouched;
// their consumers subscribe elsewhere (the coordination deltas drive an orchestration_snapshot
// re-query once the orchestration view exists; a template change drives the settings manager to
// re-read the (kind, scope) list it names). A restart's status move arrives as ProcessStatusChanged;
// RestartScheduled/RestartExhausted are the discrete signals notifications subscribe to;
// MetricsTick feeds a coalesced CPU/memory view, not the list.
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
          requires_trust: event.requires_trust,
          resumable: event.resumable,
          ports: [],
          ready: "Ungated",
        },
      ];
    case "ProcessStatusChanged":
      return processes.map((process) =>
        process.id === event.id
          ? { ...process, status: event.to, exit_code: event.exit_code }
          : process,
      );
    case "PortsChanged":
      return processes.map((process) =>
        process.id === event.id ? { ...process, ports: event.ports } : process,
      );
    case "ReadyStateChanged":
      return processes.map((process) =>
        process.id === event.id
          ? { ...process, ready: event.ready ? "Ready" : "Waiting" }
          : process,
      );
    case "ProcessRemoved":
      return processes.filter((process) => process.id !== event.id);
    // A removed project's processes each announce ProcessRemoved first; this sweep only
    // covers a consumer that missed those deltas (bus lag), so the list never strands rows
    // keyed to a project that no longer exists.
    case "ProjectRemoved":
      return processes.filter((process) => process.project !== event.id);
    case "ProcessRenamed":
      return processes.map((process) =>
        process.id === event.id ? { ...process, label: event.label } : process,
      );
    case "MetricsTick":
    case "RestartScheduled":
    case "RestartExhausted":
    case "FileRestart":
    case "ProjectOpened":
    case "ConfigChanged":
    case "TerminalTitleChanged":
    case "TerminalBell":
    case "AgentActivityChanged":
    case "OrphansFound":
    case "TodoChanged":
    case "TimerArmed":
    case "TimerFired":
    case "TimerCleared":
    case "TimerPaused":
    case "TimerResumed":
    case "LeaseChanged":
    case "ScratchpadChanged":
    case "KvChanged":
    case "TemplateChanged":
      return processes;
  }
}
