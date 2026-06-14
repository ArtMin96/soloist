// Domain shapes mirrored from the Rust core (serde-serialized). This is the single
// source of truth for these types on the TypeScript side — components and the store
// import from here rather than re-declaring statuses or event shapes.

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
  kind: ProcessKind;
  label: string;
  status: ProcStatus;
}

// Mirrors the core's `DomainEvent` (serde `tag = "type"`).
export type DomainEvent =
  | { type: "ProcessSpawned"; id: number; kind: ProcessKind; label: string; status: ProcStatus }
  | { type: "ProcessStatusChanged"; id: number; from: ProcStatus; to: ProcStatus }
  | { type: "ProcessRemoved"; id: number };

export interface AppInfo {
  name: string;
  version: string;
}
