import { beforeEach, describe, expect, it } from "vitest";
import { MERMAID_RENDER_CACHE_SIZE } from "./const";
import { cacheRender, cachedRender, clearRenderCache } from "./renderCache";

beforeEach(clearRenderCache);

describe("the diagram render cache", () => {
  it("serves a diagram back under the palette it was drawn in", () => {
    cacheRender("dark", "flowchart TD", "<svg>dark</svg>");
    expect(cachedRender("dark", "flowchart TD")).toBe("<svg>dark</svg>");
  });

  it("misses when the palette changed, so a theme flip really re-renders", () => {
    cacheRender("dark", "flowchart TD", "<svg>dark</svg>");
    expect(cachedRender("light", "flowchart TD")).toBeUndefined();
  });

  it("misses when the source changed", () => {
    cacheRender("dark", "flowchart TD", "<svg>a</svg>");
    expect(cachedRender("dark", "flowchart LR")).toBeUndefined();
  });

  it("cannot confuse two entries whose signature and source run together", () => {
    cacheRender("a", "b c", "<svg>first</svg>");
    cacheRender("a b", "c", "<svg>second</svg>");
    expect(cachedRender("a", "b c")).toBe("<svg>first</svg>");
    expect(cachedRender("a b", "c")).toBe("<svg>second</svg>");
  });

  it("never holds more than the cap, however many diagrams are drawn", () => {
    const overflow = MERMAID_RENDER_CACHE_SIZE + 5;
    for (let i = 0; i < overflow; i += 1) cacheRender("dark", `source ${i}`, `<svg>${i}</svg>`);

    const held = Array.from({ length: overflow }, (_, i) =>
      cachedRender("dark", `source ${i}`),
    ).filter((svg) => svg !== undefined);
    expect(held).toHaveLength(MERMAID_RENDER_CACHE_SIZE);
    // The survivors are the most recent ones; the earliest were evicted.
    expect(cachedRender("dark", "source 0")).toBeUndefined();
    expect(cachedRender("dark", `source ${overflow - 1}`)).toBe(`<svg>${overflow - 1}</svg>`);
  });

  it("evicts by least-recent use, so a diagram kept in view survives newer ones", () => {
    for (let i = 0; i < MERMAID_RENDER_CACHE_SIZE; i += 1) {
      cacheRender("dark", `source ${i}`, `<svg>${i}</svg>`);
    }
    // Touch the oldest entry, making the second-oldest the eviction candidate instead.
    expect(cachedRender("dark", "source 0")).toBe("<svg>0</svg>");

    cacheRender("dark", "newcomer", "<svg>new</svg>");

    expect(cachedRender("dark", "source 0")).toBe("<svg>0</svg>");
    expect(cachedRender("dark", "source 1")).toBeUndefined();
  });

  it("re-caching a source refreshes it rather than adding a second entry", () => {
    cacheRender("dark", "flowchart TD", "<svg>old</svg>");
    cacheRender("dark", "flowchart TD", "<svg>new</svg>");
    expect(cachedRender("dark", "flowchart TD")).toBe("<svg>new</svg>");
  });
});
