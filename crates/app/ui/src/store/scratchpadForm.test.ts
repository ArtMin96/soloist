import { describe, expect, it } from "vitest";
import type { ScratchpadDoc } from "@/domain";
import {
  appendRow,
  docToForm,
  formToDoc,
  removeItem,
  setItem,
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

describe("scratchpad form mapping", () => {
  it("round-trips a document through the form unchanged", () => {
    expect(formToDoc(docToForm(doc))).toEqual(doc);
  });

  it("represents a null notes as an empty string in the form", () => {
    expect(docToForm(doc).notes).toBe("");
  });

  it("gives an empty list a blank row to type into, dropped again on save", () => {
    const empty: ScratchpadDoc = { ...doc, plan: [] };
    const form = docToForm(empty);
    expect(form.plan).toEqual([""]);
    expect(formToDoc(form).plan).toEqual([]);
  });

  it("trims fields and drops blank list rows and empty notes on save", () => {
    const form: ScratchpadForm = {
      objective: "  Ship v1  ",
      context: "RC cut",
      plan: ["Cut RC", "   ", ""],
      acceptance_criteria: ["soak green"],
      risks: ["none identified"],
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

  it("edits a list immutably", () => {
    const items = ["a", "b", "c"];
    expect(setItem(items, 1, "B")).toEqual(["a", "B", "c"]);
    expect(removeItem(items, 0)).toEqual(["b", "c"]);
    expect(appendRow(items)).toEqual(["a", "b", "c", ""]);
    expect(items).toEqual(["a", "b", "c"]);
  });
});
