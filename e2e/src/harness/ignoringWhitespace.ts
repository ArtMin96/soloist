/**
 * A rendered read stripped of every space and line break, for a substring match that must not
 * depend on where the surface broke the text. Apply it to both sides of the comparison.
 *
 * A terminal read is the viewport's rows joined by newlines, and a row boundary is a wrap as often
 * as it is a line end — xterm.js's DOM keeps no record of which, and each row arrives right-trimmed,
 * so a wrap drops the space it broke on as readily as it splits a word. Two things decide where a
 * line wraps, neither of them anything a walk is about: the pane's column count, and the column the
 * line started at. On a PTY the second is not even stable — the kernel echoes injected input in
 * chunks that interleave with the child's own stdout, so a line the child printed can begin
 * anywhere on a row. Matching the raw read pins both, and fails on text that is fully present.
 */
export function ignoringWhitespace(text: string): string {
  return text.replace(/[·\s]+/gu, "");
}
