import type { TemplateKind, TemplateView } from "@domain";
import { invoke } from "../harness/tauri.js";

const PROMPT: TemplateKind = "prompt";

/**
 * Authors a prompt template in a project's library, through the same core command the Templates
 * panel's create form posts.
 *
 * An arrange step, never the behavior under test. A template's body is a ProseMirror
 * contenteditable, and WebKitGTK under WebDriver does not deliver the `beforeinput`/text events
 * ProseMirror needs to insert typed characters — the same limitation the scratchpad screen works
 * around by clicking a formatting control instead of typing. A body therefore has no clickable path,
 * so the template a walk needs is authored the way a project is opened: by calling the command the
 * surface itself calls, and asserting only on what the window renders afterwards.
 */
export async function createPromptTemplate(
  project: number,
  name: string,
  description: string,
  body: string,
): Promise<TemplateView> {
  return invoke<TemplateView>("template_create", {
    kind: PROMPT,
    project,
    name,
    description,
    body,
  });
}
