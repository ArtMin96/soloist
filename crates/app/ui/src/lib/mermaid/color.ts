// Resolving a CSS colour of any syntax to plain sRGB, because Mermaid's colour engine understands
// only the legacy notations (hex, `rgb()`, `hsl()`, named) and throws `Unsupported color format` on
// anything else. The app's design tokens are authored in `oklch()`, so every token must be converted
// before it is handed over.
//
// The conversion paints the colour into a 1x1 canvas and reads the pixel back, which makes the
// browser's own colour engine the single source of truth — no hand-rolled OKLCH maths to drift from
// the tokens the rest of the UI is drawn with. Reading the computed `color` of a probe element does
// **not** work: per CSS Color 4 a computed colour keeps its origin colour space, so an `oklch()`
// token serialises straight back as `oklch()`.

/** The alpha byte of a fully opaque pixel. */
const OPAQUE = 255;

/**
 * Rounding divisor for the alpha channel — three decimal places, which is finer than the 1/255 a byte
 * can express, so the round trip never loses a step while keeping the serialised value short.
 */
const ALPHA_PRECISION = 1000;

/**
 * A colour no design token uses. `fillStyle` ignores a value it cannot parse, so seeding this before
 * each assignment turns an unparseable input into a detectable no-op rather than silently painting
 * whichever colour was converted last.
 */
const UNSET = "#000001";

/** The canvas backing every conversion: created on first use, then reused (one context, no churn). */
let pixel: CanvasRenderingContext2D | null | undefined;

function context(): CanvasRenderingContext2D | null {
  if (pixel === undefined) {
    const canvas = document.createElement("canvas");
    canvas.width = 1;
    canvas.height = 1;
    pixel = canvas.getContext("2d", { willReadFrequently: true });
  }
  return pixel;
}

/**
 * `raw` as an `rgb()`/`rgba()` string, or `raw` unchanged when it cannot be converted: a blank value,
 * a value the canvas rejects, or a renderer with no raster canvas (jsdom, where there are no resolved
 * token values to convert in the first place). The canvas and the style engine are the same parser, so
 * a token the app is drawn with always converts.
 */
export function toRgb(raw: string): string {
  const ctx = raw ? context() : null;
  if (!ctx) return raw;
  ctx.fillStyle = UNSET;
  ctx.fillStyle = raw;
  if (ctx.fillStyle === UNSET) return raw;
  ctx.clearRect(0, 0, 1, 1);
  ctx.fillRect(0, 0, 1, 1);
  const [r, g, b, a] = ctx.getImageData(0, 0, 1, 1).data;
  if (a === OPAQUE) return `rgb(${r}, ${g}, ${b})`;
  return `rgba(${r}, ${g}, ${b}, ${Math.round((a / OPAQUE) * ALPHA_PRECISION) / ALPHA_PRECISION})`;
}
