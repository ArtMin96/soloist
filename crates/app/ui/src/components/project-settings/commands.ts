import type { ProcessSpec, ProjectCommandView, Visibility } from "@/domain";

// The command mutations the list and editor raise. The pane binds each to a core command and
// reloads the page afterwards; the visibility-dispatched calls (edit, rename, remove, the storage
// move) pick the shared-vs-local core command from a command's `visibility` in the pane, so the
// presentational components never branch on it.
export interface CommandOps {
  // Replace a command's spec — the editor patches one field and preserves the rest.
  edit: (command: ProjectCommandView, spec: ProcessSpec) => void;
  // Rename a command in place.
  rename: (command: ProjectCommandView, to: string) => void;
  // Override the command's terminal-alert state (kept apart from its spec).
  setTerminalAlerts: (command: ProjectCommandView, enabled: boolean) => void;
  // Move a command between the shared `solo.yml` and the app-local overlay.
  toggleStorage: (command: ProjectCommandView) => void;
  // Delete a command from wherever it lives.
  remove: (command: ProjectCommandView) => void;
  // Add a new command to the shared config or the local overlay; resolves once the page reloads,
  // rejects with the reason so the add dialog can keep itself open and show it.
  add: (name: string, spec: ProcessSpec, visibility: Visibility) => Promise<void>;
}
