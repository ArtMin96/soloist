// Shared constants for the Mermaid renderer. Everything tunable about diagram rendering lives here so
// the engine, the theme bridge, and the React surfaces never carry a bare literal.

/** Prefix for the unique element id every render must supply — Mermaid uses it to key its temp DOM. */
export const MERMAID_ID_PREFIX = "soloist-mermaid";

/**
 * The code-block language token that marks a fenced block as a diagram. It is the `language` attribute
 * of the editor's `codeBlock` node and the ```lang tag the Markdown serializer round-trips, so the
 * NodeView, the slash menu, and the toolbar all agree on one spelling and never carry a bare "mermaid".
 */
export const MERMAID_LANGUAGE = "mermaid";

/** The starter graph a freshly inserted diagram block holds — enough to render, small enough to replace. */
export const MERMAID_STARTER_SOURCE = "flowchart TD\n  A[Start] --> B[Done]";

/**
 * Sanitizing security level. `strict` runs Mermaid's output through DOMPurify and uses no `eval` and
 * no iframe, so it renders under the app's Content-Security-Policy unchanged; `sandbox` would need a
 * frame the CSP forbids. Diagram source is untrusted (an agent may author it), so this never relaxes.
 */
export const MERMAID_SECURITY_LEVEL = "strict";

/** Diagram font size, in px — sized to the app's dense body type rather than Mermaid's larger default. */
export const MERMAID_FONT_SIZE = "13px";

/** Debounce before re-rendering a diagram while its source is being edited (coalesces keystrokes). */
export const MERMAID_RENDER_DEBOUNCE_MS = 200;

/** The unzoomed scale — 100%, the fit/reset baseline for the pan-zoom canvas. */
export const MERMAID_DEFAULT_ZOOM = 1;

/** Lower and upper bounds a diagram may be zoomed to, so a wheel or button can never run away. */
export const MIN_MERMAID_ZOOM = 0.25;
export const MAX_MERMAID_ZOOM = 4;

/** Fraction a single zoom-in/out step (a wheel notch or a button press) changes the scale by. */
export const MERMAID_ZOOM_STEP = 0.15;

/** Supersampling factor when rasterizing an SVG to PNG, so the exported bitmap is not soft. */
export const MERMAID_PNG_SCALE = 2;

/**
 * The design-token → Mermaid `themeVariables` binding. Each entry maps a Mermaid theme variable to a
 * CSS custom property from `index.css`; the theme bridge resolves the property's OKLCH value to an rgb
 * string (Mermaid's color engine does not accept `oklch()`) and feeds it in. One binding table so the
 * light and dark diagram palettes stay the app's palette, defined once.
 */
export const MERMAID_THEME_TOKENS: Record<string, string> = {
  background: "--muted",
  mainBkg: "--accent",
  primaryColor: "--accent",
  primaryBorderColor: "--primary",
  primaryTextColor: "--foreground",
  secondaryColor: "--muted",
  tertiaryColor: "--background",
  lineColor: "--muted-foreground",
  textColor: "--foreground",
  nodeBorder: "--primary",
  clusterBkg: "--background",
  clusterBorder: "--border",
  titleColor: "--foreground",
  edgeLabelBackground: "--muted",
  actorBorder: "--primary",
  actorBkg: "--accent",
  noteBkg: "--muted",
  noteBorder: "--border",
};
