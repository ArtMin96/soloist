import { describe, expect, it } from "vitest";
import type { ScratchpadDoc } from "@/domain";
import {
  appendRow,
  docToForm,
  formToDoc,
  removeItem,
  setItem,
  type Row,
  type ScratchpadForm,
} from "@/store/scratchpadForm";

const doc: ScratchpadDoc = {
  objective: "Ship v1",
  context: "RC cut",
  plan: ["Cut RC", "Soak"],
  acceptance_criteria: ["soak green"],
  risks: ["none identified"],
  status: "in progress",
  notes: null,
};

// Build rows with predictable ids so list-op assertions can name them.
const rows = (values: string[]): Row[] => values.map((value, i) => ({ id: `r${i}`, value }));

describe("scratchpad form mapping", () => {
  it("round-trips a document through the form unchanged", () => {
    expect(formToDoc(docToForm(doc))).toEqual(doc);
  });

  it("represents a null notes as an empty string in the form", () => {
    expect(docToForm(doc).notes).toBe("");
  });

  it("gives each list row a stable id", () => {
    const form = docToForm(doc);
    const ids = form.plan.map((row) => row.id);
    expect(new Set(ids).size).toBe(form.plan.length);
    expect(form.plan.map((row) => row.value)).toEqual(["Cut RC", "Soak"]);
  });

  it("gives an empty list a blank row to type into, dropped again on save", () => {
    const empty: ScratchpadDoc = { ...doc, plan: [] };
    const form = docToForm(empty);
    expect(form.plan.map((row) => row.value)).toEqual([""]);
    expect(formToDoc(form).plan).toEqual([]);
  });

  it("trims fields and drops blank list rows and empty notes on save", () => {
    const form: ScratchpadForm = {
      objective: "  Ship v1  ",
      context: "RC cut",
      plan: rows(["Cut RC", "   ", ""]),
      acceptance_criteria: rows(["soak green"]),
      risks: rows(["none identified"]),
      status: " in progress ",
      notes: "   ",
    };
    const out = formToDoc(form);
    expect(out.objective).toBe("Ship v1");
    expect(out.status).toBe("in progress");
    expect(out.plan).toEqual(["Cut RC"]);
    expect(out.notes).toBeNull();
  });

  it("keeps non-empty notes", () => {
    expect(formToDoc({ ...docToForm(doc), notes: "see thread" }).notes).toBe("see thread");
  });

  it("edits a list immutably, preserving each row's id", () => {
    const items = rows(["a", "b", "c"]);
    expect(setItem(items, 1, "B")).toEqual([
      { id: "r0", value: "a" },
      { id: "r1", value: "B" },
      { id: "r2", value: "c" },
    ]);
    expect(removeItem(items, 0)).toEqual([
      { id: "r1", value: "b" },
      { id: "r2", value: "c" },
    ]);
    const appended = appendRow(items);
    expect(appended.slice(0, 3)).toEqual(items);
    expect(appended[3]).toEqual({ id: expect.any(String), value: "" });
    // The source array is untouched.
    expect(items).toEqual(rows(["a", "b", "c"]));
  });
});
