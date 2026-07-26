// The pure pan-zoom math behind the diagram canvas. Kept framework-free and side-effect-free so the
// clamping and zoom-to-cursor arithmetic can be unit-tested without a DOM; the canvas component owns
// only the pointer/wheel wiring and reads its transform from here.

import { MAX_MERMAID_ZOOM, MERMAID_DEFAULT_ZOOM, MIN_MERMAID_ZOOM } from "./const";

/** A 2-D affine view: a uniform `scale` about the origin, then a `x`/`y` translation, in px. */
export interface Transform {
  scale: number;
  x: number;
  y: number;
}

/** The unzoomed, unpanned view — the fit/reset baseline. */
export const IDENTITY_TRANSFORM: Transform = { scale: MERMAID_DEFAULT_ZOOM, x: 0, y: 0 };

/** Constrain a scale to the allowed zoom range, so no gesture can drive it past the bounds. */
export function clampZoom(scale: number): number {
  return Math.min(MAX_MERMAID_ZOOM, Math.max(MIN_MERMAID_ZOOM, scale));
}

/**
 * Zoom by `factor` about the point `(px, py)` (in the canvas's own coordinates), keeping whatever
 * content sits under that point fixed on screen — the behaviour a wheel-zoom-to-cursor needs. The new
 * scale is clamped, and the translation is recomputed against the *effective* scale change so a zoom
 * that clamps at a bound does not drift. Zooming at the container centre is the same call with the
 * centre as the point, which the buttons use.
 */
export function zoomAround(
  transform: Transform,
  factor: number,
  px: number,
  py: number,
): Transform {
  const scale = clampZoom(transform.scale * factor);
  const ratio = scale / transform.scale;
  return {
    scale,
    x: px - (px - transform.x) * ratio,
    y: py - (py - transform.y) * ratio,
  };
}
