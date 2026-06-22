// Splits the "agent with flags" input into argv tokens, respecting single and double
// quotes (the surrounding quotes are stripped); whitespace outside quotes separates tokens.
// This is only input parsing for one launch — the core re-quotes each token safely for the
// shell (`AgentTool::launch_command_line`), so this never attempts shell quoting itself.
export function tokenizeArgs(input: string): string[] {
  const tokens: string[] = [];
  let current = "";
  let quote: '"' | "'" | null = null;
  // Distinguishes an empty quoted token ("") from the gap between tokens.
  let started = false;

  for (const char of input) {
    if (quote) {
      if (char === quote) quote = null;
      else current += char;
    } else if (char === '"' || char === "'") {
      quote = char;
      started = true;
    } else if (char === " " || char === "\t" || char === "\n") {
      if (started) {
        tokens.push(current);
        current = "";
        started = false;
      }
    } else {
      current += char;
      started = true;
    }
  }
  if (started) tokens.push(current);
  return tokens;
}
