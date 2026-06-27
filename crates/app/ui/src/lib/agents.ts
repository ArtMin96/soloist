// The Agents tab's pre-load default and picker constants. Auto-summarization is the one new
// setting: an opt-in (summarizer tool + model) that is OFF by default — a locked decision, since
// the core must never hard-depend on an LLM. The agent tool registry itself is the Phase-7
// surface (read-only here: list + PATH detection).

import type { AgentSettings, DetectedTool } from "@/domain";

// The pre-load fallback for the Agents document — summarization off (both null). The facade's
// stored value supersedes this on load.
export const DEFAULT_AGENT_SETTINGS: AgentSettings = {
  summarizer_tool: null,
  summarizer_model: null,
};

// A tool's full launch invocation: the command plus its always-appended args (shown as data).
export function toolInvocation(tool: DetectedTool["tool"]): string {
  return [tool.command, ...tool.default_args].join(" ");
}
