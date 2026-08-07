// The Agents tab presentation helpers. The agent tool registry is read-only here: list + PATH
// detection.

import type { Assist, Detection, DetectedTool } from "@/domain";
import type { Option } from "@/lib/appearance";

// What the Assist document reads as before the stored one arrives: nothing selected, which is also
// what a fresh install holds — so the affordances that need a tool are absent rather than flickering.
export const DEFAULT_ASSIST: Assist = { tool: null };

// The label for choosing no assist tool, which turns drafting off everywhere at once.
export const NO_ASSIST_TOOL = "Off";

// The tools that can be offered for drafting: the ones Soloist knows how to ask a single question,
// and that a probe found on this machine. A provider Soloist cannot run headless is not offered
// rather than offered and then refused; one that is not installed would only fail.
export function assistToolOptions(detected: DetectedTool[]): Option<string | null>[] {
  const options: Option<string | null>[] = [{ value: null, label: NO_ASSIST_TOOL }];
  for (const tool of detected) {
    if (tool.can_draft && tool.detection === "Installed") {
      options.push({ value: tool.tool.name, label: tool.tool.name });
    }
  }
  return options;
}

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
