import type { ScratchpadDoc } from "@/domain";

// The editable form for a scratchpad's disciplined document. It mirrors `ScratchpadDoc` but holds
// `notes` as a plain string (the optional field is empty, not null, while editing) and keeps the
// three lists as arrays the editor adds to and removes from. Mapping to and from the domain document
// is pure and single-sourced here so the component stays presentational and the round-trip is tested.
export interface ScratchpadForm {
  objective: string;
  context: string;
  plan: string[];
  acceptance_criteria: string[];
  risks: string[];
  status: string;
  notes: string;
}

// The disciplined document as an editable form. A null `notes` becomes the empty string; the lists
// gain a trailing blank row so the editor always offers an empty entry to type into.
export function docToForm(doc: ScratchpadDoc): ScratchpadForm {
  return {
    objective: doc.objective,
    context: doc.context,
    plan: withBlankRow(doc.plan),
    acceptance_criteria: withBlankRow(doc.acceptance_criteria),
    risks: withBlankRow(doc.risks),
    status: doc.status,
    notes: doc.notes ?? "",
  };
}

// The form back to a disciplined document for a write: every field trimmed, blank list rows and an
// empty `notes` dropped. It does not enforce the "no blank field / at least one entry per list"
// rule — that is the core's single source of truth, surfaced as an InvalidScratchpad on write.
export function formToDoc(form: ScratchpadForm): ScratchpadDoc {
  const notes = form.notes.trim();
  return {
    objective: form.objective.trim(),
    context: form.context.trim(),
    plan: cleanList(form.plan),
    acceptance_criteria: cleanList(form.acceptance_criteria),
    risks: cleanList(form.risks),
    status: form.status.trim(),
    notes: notes.length > 0 ? notes : null,
  };
}

// Replace the item at `index`, returning a new array (the editor's controlled-list update).
export function setItem(items: string[], index: number, value: string): string[] {
  return items.map((item, i) => (i === index ? value : item));
}

// Remove the item at `index`, returning a new array.
export function removeItem(items: string[], index: number): string[] {
  return items.filter((_, i) => i !== index);
}

// Append one blank row for the editor to type into.
export function appendRow(items: string[]): string[] {
  return [...items, ""];
}

function cleanList(items: string[]): string[] {
  return items.map((item) => item.trim()).filter((item) => item.length > 0);
}

function withBlankRow(items: string[]): string[] {
  return items.length > 0 ? [...items] : [""];
}
