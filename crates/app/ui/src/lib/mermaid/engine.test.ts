// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const initialize = vi.fn();
const render = vi.fn();

vi.mock("mermaid", () => ({ default: { initialize, render, parse: vi.fn() } }));

/** Well-formed enough for the size stamp to act on, so a result is recognisably the drawn one. */
const svgOf = (mark: string) => `<svg id="x" width="100%" viewBox="0 0 40 20">${mark}</svg>`;

/** Let every pending continuation run, so a render that is free to start has started. */
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

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

  it("draws each diagram in its own palette when two renders overlap", async () => {
    // Mermaid holds its configuration in module-level state, so a second render that configures while
    // the first is still drawing changes the palette the first one draws in. Each render reports the
    // palette that was configured at the moment it drew, which is what the diagram would come out in.
    let dark = false;
    let started = false;
    let releaseFirst = () => {};
    initialize.mockImplementation((config: { themeVariables?: { darkMode?: boolean } }) => {
      dark = config.themeVariables?.darkMode === true;
    });
    render.mockImplementation((_id: string, source: string) => {
      if (source === "slow") {
        started = true;
        return new Promise((resolve) => {
          releaseFirst = () => resolve({ svg: svgOf(`<g data-dark="${dark}"/>`) });
        });
      }
      return Promise.resolve({ svg: svgOf(`<g data-dark="${dark}"/>`) });
    });

    const { renderDiagram } = await engine();
    const slow = renderDiagram("slow");
    // The library loads before the first draw begins, so wait for it rather than flipping the theme
    // into a render that has not been configured yet.
    await vi.waitFor(() => expect(started).toBe(true));

    document.documentElement.classList.add("dark");
    const quick = renderDiagram("quick");
    // Give the second render every chance to configure Mermaid before the first finishes drawing.
    // Serialized it cannot, because it is still waiting behind the first; left to overlap it
    // reconfigures here, and the first then draws in the palette meant for the second.
    await flush();
    releaseFirst();
    const [slowResult, quickResult] = await Promise.all([slow, quick]);
    document.documentElement.classList.remove("dark");

    expect(slowResult).toEqual({ svg: expect.stringContaining('data-dark="false"') });
    expect(quickResult).toEqual({ svg: expect.stringContaining('data-dark="true"') });
  });

  it("files a render under the palette it drew in, not the one that asked for it", async () => {
    let blocking = false;
    let releaseBlocker = () => {};
    render.mockImplementationOnce(() => {
      blocking = true;
      return new Promise((resolve) => {
        releaseBlocker = () => resolve({ svg: svgOf("<g/>") });
      });
    });
    render.mockImplementation(() => Promise.resolve({ svg: svgOf("<g id='queued'/>") }));

    const { renderDiagram } = await engine();
    const blocker = renderDiagram("blocker");
    await vi.waitFor(() => expect(blocking).toBe(true));

    // Asked for while the app is light, but it cannot draw until the blocker ahead of it finishes —
    // and the app flips before it gets there, so it draws dark.
    const queued = renderDiagram("queued");
    document.documentElement.classList.add("dark");
    releaseBlocker();
    await Promise.all([blocker, queued]);
    document.documentElement.classList.remove("dark");

    // Back in light, the diagram that drew dark must not be handed back as the light one.
    render.mockResolvedValue({ svg: svgOf("<g id='relit'/>") });
    expect(await renderDiagram("queued")).toEqual({ svg: expect.stringContaining("relit") });
  });
});
