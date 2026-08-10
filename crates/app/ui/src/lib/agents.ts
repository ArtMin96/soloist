// The agent-tool and assist presentation helpers: what the Agents settings tab shows, and the words
// the surfaces needing a drafting tool share with it. The agent tool registry is read-only here:
// list + PATH detection.

import type { Assist, Detection, DetectedTool } from "@/domain";
import type { Option } from "@/lib/appearance";

// What the Assist document reads as before the stored one arrives: nothing selected, which is also
// what a fresh install holds — so the affordances that need a tool are absent rather than flickering.
export const DEFAULT_ASSIST: Assist = { tool: null };

// The label for choosing no assist tool, which turns drafting off everywhere at once.
export const NO_ASSIST_TOOL = "Off";

// How a control with no tool to draft with yet explains itself. Written once because every surface
// that can draft — a commit message, a pull request's description — offers the same door to the same
// setting, and a reader who meets it twice must not be told two different things. It stops at where
// drafting is set up rather than promising a tool to pick, because whether there is one to pick is
// the question that setting answers: it offers what is installed, and names what to install when
// nothing is.
export const ASSIST_SETUP_HINT =
  "No tool is picked to draft with yet. This opens Settings, where drafting is set up.";

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

// Whether any tool can be picked to draft with, i.e. whether the picker offers anything beyond
// turning drafting off. When it does not, the surface says what to install instead of presenting a
// dropdown whose only entry is "Off".
export function offersAssistTool(options: Option<string | null>[]): boolean {
  return options.some((option) => option.value !== null);
}

// The tools Soloist could draft with once they are installed: every registry entry it knows how to
// ask a single question. Read off the registry rather than listed here, so the guidance shown when
// none of them is available names whatever the core actually seeds — no second copy of the set.
export function draftCapableTools(detected: DetectedTool[]): string[] {
  return detected.flatMap((tool) => (tool.can_draft ? tool.tool.name : []));
}

// Why the detection badges may all read "not checked": the sweep itself failed. Said plainly, with
// the core's own reason, so a broken probe is never mistaken for a machine with no agent CLIs on it.
export function detectionFailure(reason: unknown): string {
  return `Could not check which agent CLIs are installed: ${String(reason)}`;
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
