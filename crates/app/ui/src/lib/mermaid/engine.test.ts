// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const initialize = vi.fn();
const render = vi.fn();

vi.mock("mermaid", () => ({ default: { initialize, render, parse: vi.fn() } }));

/** Well-formed enough for the size stamp to act on, so a result is recognisably the drawn one. */
const svgOf = (mark: string) => `<svg id="x" width="100%" viewBox="0 0 40 20">${mark}</svg>`;

async function engine() {
  return import("./engine");
}

beforeEach(async () => {
  vi.resetModules();
  initialize.mockReset().mockImplementation(() => {});
  render.mockReset().mockResolvedValue({ svg: svgOf("<g/>") });
  const { clearRenderCache } = await import("./renderCache");
  clearRenderCache();
});

afterEach(() => vi.restoreAllMocks());

describe("renderDiagram", () => {
  it("reports a configuration failure as an error instead of never settling", async () => {
    // A palette Mermaid's colour engine rejects throws out of `initialize`, before any drawing. Left
    // uncaught this rejects the promise every diagram surface awaits, so the surface sits on its
    // placeholder forever with nothing to show and nothing to explain.
    initialize.mockImplementation(() => {
      throw new Error('Unsupported color format: "oklch(0.235 0.012 255)"');
    });

    const { renderDiagram } = await engine();

    await expect(renderDiagram("flowchart TD")).resolves.toEqual({
      error: 'Unsupported color format: "oklch(0.235 0.012 255)"',
    });
  });

  it("reports a malformed diagram as an error", async () => {
    render.mockRejectedValue(new Error("Parse error on line 2"));

    const { renderDiagram } = await engine();

    await expect(renderDiagram("not a diagram")).resolves.toEqual({
      error: "Parse error on line 2",
    });
  });

  it("gives the drawn diagram a definite size so it cannot collapse to the default", async () => {
    const { renderDiagram } = await engine();

    const result = await renderDiagram("flowchart TD");

    expect(result).toEqual({ svg: expect.stringContaining('width="40" height="20"') });
  });

  it("draws the same source and palette once, then serves it back", async () => {
    const { renderDiagram } = await engine();

    const first = await renderDiagram("flowchart TD");
    const second = await renderDiagram("flowchart TD");

    expect(second).toEqual(first);
    expect(render).toHaveBeenCalledTimes(1);
  });

  it("draws again when the app palette changed", async () => {
    const { renderDiagram } = await engine();
    await renderDiagram("flowchart TD");

    document.documentElement.classList.add("dark");
    render.mockResolvedValue({ svg: svgOf("<g id='dark'/>") });
    const dark = await renderDiagram("flowchart TD");
    document.documentElement.classList.remove("dark");

    expect(dark).toEqual({ svg: expect.stringContaining("dark") });
    expect(render).toHaveBeenCalledTimes(2);
  });

  it("never lets two renders interleave, so one cannot draw in the other's palette", async () => {
    const order: string[] = [];
    let releaseFirst = () => {};
    render.mockImplementation((_id: string, source: string) => {
      order.push(`start ${source}`);
      if (source === "slow") {
        return new Promise((resolve) => {
          releaseFirst = () => {
            order.push("end slow");
            resolve({ svg: svgOf("<g/>") });
          };
        });
      }
      order.push(`end ${source}`);
      return Promise.resolve({ svg: svgOf("<g/>") });
    });

    const { renderDiagram } = await engine();
    const slow = renderDiagram("slow");
    const quick = renderDiagram("quick");
    // The library loads before the first draw begins, so wait for it rather than releasing into a
    // handler that has not been installed yet.
    await vi.waitFor(() => expect(order).toContain("start slow"));
    releaseFirst();
    await Promise.all([slow, quick]);

    // The second render must not have configured Mermaid while the first was still drawing.
    expect(order).toEqual(["start slow", "end slow", "start quick", "end quick"]);
  });
});
