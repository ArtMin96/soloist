// Turning a filesystem path into a token a POSIX shell reads back as exactly those bytes.
//
// Single quotes are the only quoting form a shell leaves entirely alone: inside them a space, a
// newline, `$`, a backslash and a double quote are all literal, so none of them needs escaping and
// none of them can start a substitution. The one character that cannot appear inside a single-quoted
// run is the single quote itself, which is why it gets the close-escape-reopen treatment below.

const QUOTE = "'";
const ESCAPED_QUOTE = "'\\''";

/** Quote one path so a POSIX shell reads it back as exactly these bytes. */
export function quoteShellPath(path: string): string {
  return QUOTE + path.split(QUOTE).join(ESCAPED_QUOTE) + QUOTE;
}

/** Quote each path and join them the way a shell separates one argument from the next. */
export function quoteShellPaths(paths: string[]): string {
  return paths.map(quoteShellPath).join(" ");
}
