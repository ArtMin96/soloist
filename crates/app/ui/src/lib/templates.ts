import type { TemplateKind } from "@/domain";

// Display metadata for the template kinds — the single source the manager reads for order, labels,
// and the one-line explanation of each kind. Mirrors the closed core `TemplateKind`; keeping it here
// means a component never hard-codes a kind string.

// The kinds the manager lists, in display order: prompts first (the reserved prompt-templates view),
// then the two seedable kinds.
export const TEMPLATE_KINDS: readonly TemplateKind[] = ["prompt", "scratchpad", "todo"];

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
