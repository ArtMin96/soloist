import { describe, expect, it } from "vitest";
import { DIAGRAM_THEME_VALUES, readDiagramTheme, setDiagramTheme } from "./frontmatter";

const BODY = "flowchart TD\n  A[Start] --> B[Done]";

describe("readDiagramTheme", () => {
  it("returns null for a source with no frontmatter", () => {
    expect(readDiagramTheme(BODY)).toBeNull();
  });

  it("reads the theme from a config frontmatter block", () => {
    const source = `---\nconfig:\n  theme: forest\n---\n${BODY}`;
    expect(readDiagramTheme(source)).toBe("forest");
  });

  it("returns null when the frontmatter names a theme we do not offer", () => {
    const source = `---\nconfig:\n  theme: default\n---\n${BODY}`;
    expect(readDiagramTheme(source)).toBeNull();
  });

  it("ignores a top-level theme that is not under config", () => {
    const source = `---\ntheme: dark\n---\n${BODY}`;
    expect(readDiagramTheme(source)).toBeNull();
  });
});

describe("setDiagramTheme", () => {
  it("adds a frontmatter block to a bare source and reads back the theme", () => {
    for (const theme of DIAGRAM_THEME_VALUES) {
      const next = setDiagramTheme(BODY, theme);
      expect(readDiagramTheme(next)).toBe(theme);
      expect(next).toContain(BODY);
    }
  });

  it("round-trips: setting then clearing returns the original source", () => {
    const themed = setDiagramTheme(BODY, "dark");
    expect(readDiagramTheme(themed)).toBe("dark");
    const cleared = setDiagramTheme(themed, null);
    expect(readDiagramTheme(cleared)).toBeNull();
    expect(cleared).toBe(BODY);
  });

  it("replaces an existing override in place rather than stacking another", () => {
    const dark = setDiagramTheme(BODY, "dark");
    const forest = setDiagramTheme(dark, "forest");
    expect(readDiagramTheme(forest)).toBe("forest");
    // Exactly one theme entry — the value was replaced, not appended.
    expect(forest.match(/theme:/g)).toHaveLength(1);
  });

  it("preserves other frontmatter when adding and removing the override", () => {
    const titled = `---\ntitle: Auth flow\n---\n${BODY}`;
    const themed = setDiagramTheme(titled, "neutral");
    expect(readDiagramTheme(themed)).toBe("neutral");
    expect(themed).toContain("title: Auth flow");
    const cleared = setDiagramTheme(themed, null);
    expect(readDiagramTheme(cleared)).toBeNull();
    expect(cleared).toContain("title: Auth flow");
  });

  it("clearing a bare source is a no-op", () => {
    expect(setDiagramTheme(BODY, null)).toBe(BODY);
  });
});
