import type { TemplateKind, TemplateScope } from "@/domain";

// Display metadata for the template kinds and scopes — the single source the manager reads for
// order, labels, and the one-line explanation of each. Mirrors the closed core `TemplateKind` and
// `TemplateScope`; keeping it here means a component never hard-codes a kind or scope string.

// The kinds the manager lists, in display order: prompts first (the reserved prompt-templates view),
// then the two seedable kinds.
export const TEMPLATE_KINDS: readonly TemplateKind[] = ["prompt", "scratchpad", "todo"];

// The one kind whose {{placeholders}} are substituted (mirrors the core's `RENDERABLE_KIND`). A
// prompt is applied to an agent with its fill-ins resolved; the seedable kinds start a document the
// author goes on to edit, so their markers are content and are never substituted — which is why only
// a prompt template offers a preview.
export const RENDERABLE_TEMPLATE_KIND: TemplateKind = "prompt";

export const TEMPLATE_KIND_LABEL: Record<TemplateKind, string> = {
  prompt: "Prompt",
  scratchpad: "Scratchpad",
  todo: "Todo",
};

export const TEMPLATE_KIND_DESCRIPTION: Record<TemplateKind, string> = {
  prompt: "Reusable prompts with {{placeholder}} fill-ins, applied to an agent by name.",
  scratchpad:
    "A starting shape for new scratchpads. Choose a default and empty scratchpads are seeded from it.",
  todo: "A starting shape for new todos. Choose a default and empty todos are seeded from it.",
};

// The scopes a template can live in, in display order: the library shared across every project
// first, then the open project's own.
export const TEMPLATE_SCOPES: readonly TemplateScope[] = ["global", "project"];

// The visible group label for a scope. Sentence case, never an all-caps eyebrow.
export const TEMPLATE_SCOPE_LABEL: Record<TemplateScope, string> = {
  global: "Global",
  project: "This project",
};

// What an empty scope says. Each names the scope it stands for, so the two empty groups in one kind
// are never mistaken for each other.
export const TEMPLATE_SCOPE_EMPTY: Record<TemplateScope, string> = {
  global: "No global templates yet.",
  project: "No templates in this project yet.",
};

// What the preview surface says. The prompt an agent receives is the point of a prompt template, so
// the section explains itself in those terms rather than as a developer's "render" step.
export const TEMPLATE_PREVIEW_DESCRIPTION =
  "The prompt this template produces. Fill in its placeholders to see the text an agent receives.";

export const TEMPLATE_PREVIEW_NO_PLACEHOLDERS =
  "This template declares no placeholders — it renders exactly as written.";

// A placeholder written the way it appears in a body. The delimiters mirror the core's grammar and
// are spelled once here, so no surface hard-codes them.
function marker(name: string): string {
  return `{{${name}}}`;
}

// What the preview says about placeholders left without a value. Naming them makes the gap findable
// in a long prompt; the output keeps each one literal, which is what "as written" points at.
export function unfilledNotice(unfilled: string[]): string {
  const names = unfilled.map(marker).join(", ");
  return unfilled.length === 1
    ? `No value for ${names} — it stays as written in the prompt below.`
    : `No value for ${names} — they stay as written in the prompt below.`;
}

// What the preview says about supplied values that match no placeholder — a typo, or a marker edited
// out of the body while its value was still held. Silence here is what turns a typo into a mystery.
export function unknownNotice(unknown: string[]): string {
  const names = unknown.map(marker).join(", ");
  return unknown.length === 1
    ? `${names} matches no placeholder in this template, so it was not used.`
    : `${names} match no placeholder in this template, so they were not used.`;
}

// The one phrase naming a kind in a scope, so every surface that must say which library it means
// says it the same way. Written once here rather than assembled per component.
function scopePhrase(kind: TemplateKind, scope: TemplateScope, plural: boolean): string {
  const noun = plural ? "templates" : "template";
  return scope === "global"
    ? `Global ${TEMPLATE_KIND_LABEL[kind].toLowerCase()} ${noun}`
    : `${TEMPLATE_KIND_LABEL[kind]} ${noun} in this project`;
}

// The accessible name of one kind's group in one scope — unique across the panel, so assistive
// technology (and a test) can address "the scratchpad templates in this project" exactly.
export function templateGroupLabel(kind: TemplateKind, scope: TemplateScope): string {
  return scopePhrase(kind, scope, true);
}

// How a drill-in surface (the editor's caption, the create form's heading) names the one template
// it is working on, scope included — so a name that exists in both libraries is never ambiguous.
export function templateScopeHeading(kind: TemplateKind, scope: TemplateScope): string {
  return scopePhrase(kind, scope, false);
}
