// The markup shape a process row shares wherever one renders — the sidebar's rows and the
// orchestration tree's nodes. A row lays out an optional spacer or chevron, the indicator span,
// then its plain text spans. One selector source for that shape, so a change to the row markup
// is one edit here rather than one per screen.

/** The indicator span while the row reports a plain process status. */
export const ROW_STATUS = ":scope > span[data-status]";

/**
 * The indicator span once a running tracked agent reports what it is doing — the indicator only
 * ever swaps `data-status` out for `data-activity`, never renders both.
 */
export const ROW_ACTIVITY = ":scope > span[data-activity]";

/**
 * The row's plain text spans — the label, then (where the row renders one) its kind. Structural
 * rather than styling-based: the direct-child spans carrying none of the marker attributes, so a
 * restyle does not move them and only genuinely dropping a text span breaks the read — the
 * signal we want.
 */
export const ROW_TEXT = ":scope > span:not([aria-hidden]):not([data-status]):not([data-activity])";
