import { describe, expect, it } from "vitest";
import { uniqueCopyName } from "@/store/templateCopy";

describe("uniqueCopyName", () => {
  it("appends ' copy' when that name is free", () => {
    expect(uniqueCopyName("daily", [])).toBe("daily copy");
    expect(uniqueCopyName("daily", ["daily"])).toBe("daily copy");
  });

  it("numbers further copies past the first", () => {
    expect(uniqueCopyName("daily", ["daily", "daily copy"])).toBe("daily copy 2");
    expect(uniqueCopyName("daily", ["daily copy", "daily copy 2"])).toBe("daily copy 3");
  });

  it("fills the lowest free gap, not just the count", () => {
    // "daily copy 2" is free even though "daily copy 3" exists.
    expect(uniqueCopyName("daily", ["daily copy", "daily copy 3"])).toBe("daily copy 2");
  });
});
