// The orchestration pane's inbound cross-surface navigation target: a session-work item's opener
// resolves it, the pane switches to its view and expands/selects it. A fresh `nonce` on every
// activation lets a repeat of the same target refocus rather than being a no-op state change.
export type OrchestrationFocus =
  | { view: "todos"; id: number; nonce: number }
  | { view: "scratchpads"; name: string; nonce: number };
