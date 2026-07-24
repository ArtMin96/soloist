import { describe, expect, it } from "vitest";
import { MAX_MERMAID_ZOOM, MIN_MERMAID_ZOOM } from "./const";
import { clampZoom, IDENTITY_TRANSFORM, zoomAround } from "./zoom";

describe("clampZoom", () => {
  it("holds a scale inside the range unchanged", () => {
    expect(clampZoom(1)).toBe(1);
  });

  it("clamps to the bounds past either end", () => {
    expect(clampZoom(MAX_MERMAID_ZOOM * 10)).toBe(MAX_MERMAID_ZOOM);
    expect(clampZoom(MIN_MERMAID_ZOOM / 10)).toBe(MIN_MERMAID_ZOOM);
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
