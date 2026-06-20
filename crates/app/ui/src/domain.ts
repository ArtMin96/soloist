// Domain shapes mirrored from the Rust core (serde-serialized). This is the single
// source of truth for these types on the TypeScript side — components and the store
// import from here rather than re-declaring statuses or event shapes. Field names match
// the core's serde output (snake_case where the Rust field is snake_case).

export type ProcessKind = "Command" | "Agent" | "Terminal";

export type ProcStatus =
  | "Stopped"
  | "Starting"
  | "Running"
  | "Crashed"
  | "Restarting"
  | "Stopping"
  | "RestartExhausted";

// Port-readiness gate (mirrors core::Readiness): "Ungated" = no gate active, "Waiting" = a
// wait_for_port is in effect and the awaited port has not bound (Running but not Ready),
// "Ready" = the awaited port bound.
export type Readiness = "Ungated" | "Waiting" | "Ready";

export interface ProcessView {
  id: number;
  project: number;
  kind: ProcessKind;
  label: string;
  status: ProcStatus;
  exit_code: number | null;
  // True for a trust-gated command whose variant is not yet trusted; the UI blocks its
  // start and offers a trust affordance.
  requires_trust: boolean;
  // TCP ports the process is currently listening on (discovered while it runs, cleared when
  // it stops). Empty until discovery finds any.
  ports: number[];
  // Port-readiness gate; "Ungated" until a wait_for_port is in effect, and reset to
  // "Ungated" when the process stops.
  ready: Readiness;
}

// Rendered output snapshot (escape sequences applied to plain text) — the Logs source.
export interface RenderedScreen {
  lines: string[];
}

// One rendered line of terminal output.
export interface LogLine {
  text: string;
}

// A `solo.yml` sync diff, carried by ConfigChanged (consumed by the trust/sync dialog).
export interface Rename {
  from: string;
  to: string;
}

export interface ConfigSync {
  added: string[];
  updated: string[];
  removed: string[];
  renamed: Rename[];
}

// A leftover process group awaiting a Kill / Kill All / Leave decision.
export interface OrphanInfo {
  name: string;
  command: string;
  pgid: number;
}

// One command awaiting trust after a config change — enough detail to review what it
// runs (command, working dir, env) before trusting it. Carried by ConfigChanged.
export interface TrustReviewCommand {
  name: string;
  command: string;
  working_dir: string | null;
  env: Record<string, string>;
}

// The outcome of opening a project (the `project_load` command). `processes` is how many
// the folder's solo.yml declared; `created` is true when Soloist auto-created the solo.yml
// from detected commands (the folder had none). The UI turns these facts into a notice so
// opening a project is never silent.
export interface ProjectLoad {
  id: number;
  processes: number;
  created: boolean;
}

// A project's display identity (the `project_list` query): its durable id, resolved name
// (solo.yml `name:` or folder), root, and a ready-to-render icon — a `data:` URL the backend
// loaded from the project's `icon:` (null when none), so name and icon arrive as one shape
// with no separate icon request. The sidebar groups the process tree by project using this.
export interface ProjectView {
  id: number;
  name: string;
  root: string;
  icon: string | null;
}

// Mirrors the core's `DomainEvent` (serde `tag = "type"`).
export type DomainEvent =
  | {
      type: "ProcessSpawned";
      id: number;
      project: number;
      kind: ProcessKind;
      label: string;
      status: ProcStatus;
      requires_trust: boolean;
    }
  | {
      type: "ProcessStatusChanged";
      id: number;
      from: ProcStatus;
      to: ProcStatus;
      exit_code: number | null;
    }
  | { type: "ProcessRemoved"; id: number }
  // A periodic CPU/memory reading for a running process, sampled across its whole group.
  // cpu_pct is per-core (a busy multi-threaded process can exceed 100); rss is bytes.
  // Emitted ~1 Hz; consumers coalesce it (never a per-tick re-render).
  | { type: "MetricsTick"; id: number; cpu_pct: number; rss: number }
  // A process's set of bound (listening) TCP ports changed — discovered while it runs,
  // emptied when it stops. The new sorted set is carried so the read model updates without
  // a snapshot round-trip; also reflected on ProcessView.ports.
  | { type: "PortsChanged"; id: number; ports: number[] }
  // A process's readiness changed while a port wait is active: false = Running but the
  // awaited port has not bound yet, true = it bound. Reflected on ProcessView.ready.
  | { type: "ReadyStateChanged"; id: number; ready: boolean }
  // The restart policy is relaunching a crashed auto_restart command; `attempt` is its
  // position in the rate-limit window (the status also moves Crashed -> Starting).
  | { type: "RestartScheduled"; id: number; attempt: number }
  // The restart policy gave up after too many restarts in the window; the command is held
  // in RestartExhausted until the user restarts it.
  | { type: "RestartExhausted"; id: number }
  // A command was restarted because a watched file changed; the status also cycles through
  // the usual restart deltas, so this is the discrete signal for a file-watch banner.
  | { type: "FileRestart"; id: number }
  // A project was opened/changed. The UI re-reads the rendered project snapshot on this
  // (which carries each project's loaded icon); it doesn't consume the event's domain fields.
  | { type: "ProjectOpened"; id: number }
  | {
      type: "ConfigChanged";
      project: number;
      diff: ConfigSync;
      requires_trust: boolean;
      commands: TrustReviewCommand[];
    }
  | { type: "TerminalTitleChanged"; id: number; title: string }
  | { type: "TerminalBell"; id: number }
  | { type: "OrphansFound"; orphans: OrphanInfo[] };

export interface AppInfo {
  name: string;
  version: string;
}
