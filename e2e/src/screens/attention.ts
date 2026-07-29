// What every unread surface is called. The process row's dot, the project header's dot and the
// title-bar count all carry this one accessible name — the app names them alike on purpose, so
// they read as one state rather than three — and every screen that reaches for one starts here.

export const ATTENTION_NAME = "Needs attention";

/** Any rendered unread dot, wherever it sits. Callers scope it to the surface they mean. */
export const ATTENTION_MARKER = `[role="img"][aria-label="${ATTENTION_NAME}"]`;
