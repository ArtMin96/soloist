// Untrusted text as text and nothing more.
//
// A trust request's reason is written by whatever process asked for the command, and a grant
// records those same words, so both surfaces treat the string as hostile input rather than
// content: control characters (an escape sequence, a carriage return that overwrites the line, a
// bidirectional override that reverses it) are flattened to spaces, and runs of blank lines are
// collapsed so a reason cannot be padded until the command line is off screen.
//
// Nothing here renders as markup — React escapes the result, and no markdown or link handling is
// applied anywhere it is used — so the quotation can only ever read as the words it contains.
export function plainReason(reason: string): string {
  return reason
    .replace(/[^\P{C}\n]/gu, " ")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}
