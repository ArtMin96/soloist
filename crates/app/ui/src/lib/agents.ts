// The Agents tab presentation helpers. The agent tool registry is read-only here: list + PATH
// detection.

import type { DetectedTool } from "@/domain";

// A tool's full launch invocation: the command plus its always-appended args (shown as data).
export function toolInvocation(tool: DetectedTool["tool"]): string {
  return [tool.command, ...tool.default_args].join(" ");
}
