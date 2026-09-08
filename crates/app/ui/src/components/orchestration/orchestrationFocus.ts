// The orchestration pane's inbound cross-surface navigation target: a document row's opener
// resolves it, the pane switches to its view and expands/selects it. A fresh `nonce` on every
// activation lets a repeat of the same target refocus rather than being a no-op state change.
export type OrchestrationFocus = OrchestrationTarget & { nonce: number };

/** The document a surface asks the pane to show — a todo or a scratchpad, both addressed by id —
 *  before activation stamps the nonce on it. */
export type OrchestrationTarget = { view: "todos" | "scratchpads"; id: number };
