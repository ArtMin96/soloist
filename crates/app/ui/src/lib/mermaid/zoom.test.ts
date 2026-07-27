import { describe, expect, it } from "vitest";
import { MAX_FIT_ZOOM, MAX_MERMAID_ZOOM, MERMAID_DEFAULT_ZOOM, MIN_MERMAID_ZOOM } from "./const";
import { clampZoom, fitScale, IDENTITY_TRANSFORM, zoomAround } from "./zoom";

describe("clampZoom", () => {
  it("holds a scale inside the range unchanged", () => {
    expect(clampZoom(1)).toBe(1);
  });

  it("clamps to the bounds past either end", () => {
    expect(clampZoom(MAX_MERMAID_ZOOM * 10)).toBe(MAX_MERMAID_ZOOM);
    expect(clampZoom(MIN_MERMAID_ZOOM / 10)).toBe(MIN_MERMAID_ZOOM);
  });
});

describe("fitScale", () => {
  it("shrinks a diagram larger than the pane until it fits", () => {
    // Height is the tighter axis here, so it is the one that decides.
    expect(fitScale(1206, 1949, 661, 832)).toBeCloseTo(832 / 1949);
  });

  it("enlarges a diagram smaller than the pane instead of leaving it marooned", () => {
    expect(fitScale(150, 180, 661, 832)).toBeGreaterThan(1);
  });

  it("never enlarges past the ceiling, however much room there is", () => {
    expect(fitScale(10, 10, 4000, 4000)).toBe(MAX_FIT_ZOOM);
  });

  it("fills the axis that runs out first, never overflowing the other", () => {
    const scale = fitScale(400, 100, 800, 800);
    expect(400 * scale).toBeLessThanOrEqual(800);
    expect(100 * scale).toBeLessThanOrEqual(800);
    expect(scale).toBeCloseTo(2);
  });

  it("stays at the unzoomed scale when a dimension has not been measured", () => {
    expect(fitScale(0, 0, 800, 600)).toBe(MERMAID_DEFAULT_ZOOM);
    expect(fitScale(400, 300, 0, 0)).toBe(MERMAID_DEFAULT_ZOOM);
  });
});

describe("zoomAround", () => {
  it("keeps the content under the cursor fixed on screen", () => {
    const px = 100;
    const py = 50;
    const before = { scale: 1, x: 20, y: 10 };
    // The content coordinate currently under the cursor.
    const content = { x: (px - before.x) / before.scale, y: (py - before.y) / before.scale };

    const after = zoomAround(before, 1.5, px, py);

    expect(after.scale).toBeCloseTo(1.5);
    // That same content coordinate must still project to the cursor after the zoom.
    expect(after.x + after.scale * content.x).toBeCloseTo(px);
    expect(after.y + after.scale * content.y).toBeCloseTo(py);
  });

  it("never zooms past the maximum, even at a huge factor", () => {
    const after = zoomAround(IDENTITY_TRANSFORM, 1000, 0, 0);
    expect(after.scale).toBe(MAX_MERMAID_ZOOM);
  });

  it("never zooms below the minimum, even at a tiny factor", () => {
    const after = zoomAround(IDENTITY_TRANSFORM, 0.0001, 0, 0);
    expect(after.scale).toBe(MIN_MERMAID_ZOOM);
  });
});
