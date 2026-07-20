// The Agents tab presentation helpers. The agent tool registry is read-only here: list + PATH
// detection.

import type { Detection, DetectedTool } from "@/domain";

// A tool's full launch invocation: the command plus its always-appended args (shown as data).
export function toolInvocation(tool: DetectedTool["tool"]): string {
  return [tool.command, ...tool.default_args].join(" ");
}

// What each detection outcome is called, defined once so the launch picker and the settings
// registry cannot drift into describing the same state differently. Lower case is the base
// form; a surface wanting sentence case capitalises the first letter in CSS.
export const detectionLabel: Record<Detection, string> = {
  Installed: "installed",
  Missing: "not found",
  Unknown: "not checked",
};

// Why a tool is unchecked, for the surfaces that can afford the explanation. Only "Unknown"
// needs one: the other two states say all there is to say.
export const UNCHECKED_HINT =
  "Soloist could not check this tool — the probe timed out, or this provider is not auto-detected.";
