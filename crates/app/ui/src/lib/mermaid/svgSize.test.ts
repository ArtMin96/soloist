import { describe, expect, it } from "vitest";
import { withIntrinsicSize } from "./svgSize";

/** The shape Mermaid actually returns: a percentage width capped by an inline max-width. */
const MERMAID_OUTPUT =
  '<svg id="x" width="100%" xmlns="http://www.w3.org/2000/svg" ' +
  'viewBox="0 -49 1036.55 915" style="max-width: 1036.55px; background-color: white;">' +
  "<g></g></svg>";

/** The width and height the opening tag ends up carrying, as numbers. */
function size(svg: string): { width: number | null; height: number | null } {
  const open = svg.slice(0, svg.indexOf("<g"));
  const read = (name: string) => {
    const found = new RegExp(`\\s${name}="([^"]*)"`).exec(open);
    return found ? Number(found[1]) : null;
  };
  return { width: read("width"), height: read("height") };
}

describe("withIntrinsicSize", () => {
  it("sizes the diagram from its viewBox instead of leaving a percentage width", () => {
    expect(size(withIntrinsicSize(MERMAID_OUTPUT))).toEqual({ width: 1036.55, height: 915 });
  });

  it("drops the max-width that would otherwise cap the stamped size", () => {
    const stamped = withIntrinsicSize(MERMAID_OUTPUT);
    expect(stamped).not.toContain("max-width");
    // The rest of the inline style is left alone.
    expect(stamped).toContain("background-color: white");
  });

  it("keeps the body of the document untouched", () => {
    expect(withIntrinsicSize(MERMAID_OUTPUT)).toContain("<g></g></svg>");
  });

  it("leaves markup with no viewBox alone rather than guessing a size", () => {
    const noBox = '<svg width="100%" style="max-width: 10px;"><g></g></svg>';
    expect(withIntrinsicSize(noBox)).toBe(noBox);
  });

  it("leaves a degenerate viewBox alone", () => {
    const zero = '<svg width="100%" viewBox="0 0 0 0"><g></g></svg>';
    expect(withIntrinsicSize(zero)).toBe(zero);
  });

  it("does not mistake a '>' inside an attribute for the end of the opening tag", () => {
    const quoted =
      '<svg aria-roledescription="a > b" width="100%" viewBox="0 0 40 20"><g></g></svg>';
    const stamped = withIntrinsicSize(quoted);
    expect(size(stamped)).toEqual({ width: 40, height: 20 });
    expect(stamped).toContain('aria-roledescription="a > b"');
  });
});
