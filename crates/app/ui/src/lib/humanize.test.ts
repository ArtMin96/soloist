import { describe, expect, it } from "vitest";
import { distinctHandle, humanizeName } from "@/lib/humanize";

describe("humanizeName", () => {
  it("reads a hyphenated slug as a sentence", () => {
    expect(humanizeName("rich-editor-design")).toBe("Rich editor design");
  });

  it("reads a slug mixing both separators as a sentence", () => {
    expect(humanizeName("a-b_c")).toBe("A b c");
    expect(humanizeName("phase_2-notes")).toBe("Phase 2 notes");
  });

  it("leaves a single word alone — it has no separator, so it is not a slug", () => {
    expect(humanizeName("research")).toBe("research");
    expect(humanizeName("plan")).toBe("plan");
  });

  it("leaves a name a person already wrote alone", () => {
    expect(humanizeName("Release readiness")).toBe("Release readiness");
    expect(humanizeName("Q3 plan: risks")).toBe("Q3 plan: risks");
    expect(humanizeName("API-notes")).toBe("API-notes");
    expect(humanizeName("")).toBe("");
  });

  it("does not treat a leading, trailing, or doubled separator as a slug", () => {
    expect(humanizeName("-leading")).toBe("-leading");
    expect(humanizeName("trailing-")).toBe("trailing-");
    expect(humanizeName("double--dash")).toBe("double--dash");
  });
});

describe("distinctHandle", () => {
  it("gives the raw handle of a slug, which reads differently from its title", () => {
    expect(distinctHandle("rich-editor-design")).toBe("rich-editor-design");
  });

  it("gives nothing for a name that is already its own title", () => {
    expect(distinctHandle("research")).toBeNull();
    expect(distinctHandle("Release plan")).toBeNull();
  });
});
