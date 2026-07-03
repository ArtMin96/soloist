import type { ScratchpadDoc } from "@/domain";

// A single editable list row: a stable id paired with its text. The id is editor-only — it is the
// React key and focus identity while typing, so removing a middle row never reindexes the surviving
// inputs onto each other's DOM nodes. It never reaches the persisted document, which stays
// `string[]` (the core's `Vec<String>` and the MCP schema); `formToDoc` strips it back out.
export interface Row {
  id: string;
  value: string;
}

// The editable form for a scratchpad's disciplined document. It mirrors `ScratchpadDoc` but holds
// `notes` as a plain string (the optional field is empty, not null, while editing) and keeps the
// three lists as `Row[]` the editor adds to and removes from. Mapping to and from the domain
// document is pure and single-sourced here so the component stays presentational and the round-trip
// is tested.
export interface ScratchpadForm {
  objective: string;
  context: string;
  plan: Row[];
  acceptance_criteria: Row[];
  risks: Row[];
  status: string;
  notes: string;
}

let nextRowId = 0;

// A fresh, unique row id. Uniqueness among siblings (not cross-session stability) is all a React key
// needs, so a monotonic counter is enough and stays dependency-free.
function rowId(): string {
  nextRowId += 1;
  return `row-${nextRowId}`;
}

// Wrap raw strings as rows, always leaving at least one blank row for the editor to type into.
function toRows(items: string[]): Row[] {
  const rows = items.map((value) => ({ id: rowId(), value }));
  return rows.length > 0 ? rows : [{ id: rowId(), value: "" }];
}

// The disciplined document as an editable form. A null `notes` becomes the empty string; the lists
// gain a trailing blank row so the editor always offers an empty entry to type into.
export function docToForm(doc: ScratchpadDoc): ScratchpadForm {
  return {
    objective: doc.objective,
    context: doc.context,
    plan: toRows(doc.plan),
    acceptance_criteria: toRows(doc.acceptance_criteria),
    risks: toRows(doc.risks),
    status: doc.status,
    notes: doc.notes ?? "",
  };
}

// The form back to a disciplined document for a write: every field trimmed, blank list rows and an
// empty `notes` dropped, and the editor-only row ids discarded. It does not enforce the "no blank
// field / at least one entry per list" rule — that is the core's single source of truth, surfaced
// as an InvalidScratchpad on write.
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

// Replace the value of the row at `index`, keeping its id, returning a new array (the editor's
// controlled-list update).
export function setItem(items: Row[], index: number, value: string): Row[] {
  return items.map((item, i) => (i === index ? { ...item, value } : item));
}

// Remove the row at `index`, returning a new array.
export function removeItem(items: Row[], index: number): Row[] {
  return items.filter((_, i) => i !== index);
}

// Append one blank row for the editor to type into.
export function appendRow(items: Row[]): Row[] {
  return [...items, { id: rowId(), value: "" }];
}

function cleanList(items: Row[]): string[] {
  return items.flatMap((item) => {
    const trimmed = item.value.trim();
    return trimmed.length > 0 ? [trimmed] : [];
  });
}
