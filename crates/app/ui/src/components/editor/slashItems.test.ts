import { describe, expect, it } from "vitest";
import { SLASH_ITEMS, filterSlashItems } from "./slashItems";

describe("filterSlashItems", () => {
  it("returns every item for a blank query", () => {
    expect(filterSlashItems("")).toHaveLength(SLASH_ITEMS.length);
    expect(filterSlashItems("   ")).toHaveLength(SLASH_ITEMS.length);
  });

  it("matches the title case-insensitively", () => {
    expect(filterSlashItems("HEAD").map((item) => item.title)).toEqual([
      "Heading 1",
      "Heading 2",
      "Heading 3",
    ]);
  });

  it("matches keywords the title does not contain", () => {
    expect(filterSlashItems("todo").map((item) => item.title)).toEqual(["To-do list"]);
    expect(filterSlashItems("checkbox").map((item) => item.title)).toEqual(["To-do list"]);
    expect(filterSlashItems("divider").map((item) => item.title)).toEqual(["Divider"]);
  });

  it("returns nothing when nothing matches", () => {
    expect(filterSlashItems("zzz")).toEqual([]);
  });
});
