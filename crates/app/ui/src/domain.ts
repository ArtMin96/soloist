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

export interface ProcessView {
  id: number;
  project: number;
  kind: ProcessKind;
  label: string;
  status: ProcStatus;
  exit_code: number | null;
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

// Mirrors the core's `DomainEvent` (serde `tag = "type"`).
export type DomainEvent =
  | {
      type: "ProcessSpawned";
      id: number;
      project: number;
      kind: ProcessKind;
      label: string;
      status: ProcStatus;
    }
  | {
      type: "ProcessStatusChanged";
      id: number;
      from: ProcStatus;
      to: ProcStatus;
      exit_code: number | null;
    }
  | { type: "ProcessRemoved"; id: number }
  | { type: "ConfigChanged"; project: number; diff: ConfigSync; requires_trust: boolean }
  | { type: "TerminalTitleChanged"; id: number; title: string }
  | { type: "TerminalBell"; id: number }
  | { type: "OrphansFound"; orphans: OrphanInfo[] };

export interface AppInfo {
  name: string;
  version: string;
}
