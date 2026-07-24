// The Mermaid rendering engine, and the one module that imports the library. Mermaid pulls in a large
// transitive tree, so it is loaded through a single dynamic `import("mermaid")` confined here: the
// bundler splits it into its own chunk that never enters the initial payload, and it is fetched the
// first time a diagram actually renders. Every caller goes through `renderDiagram`/`parseDiagram`, so
// nothing else in the app touches Mermaid's surface.

import { MERMAID_ID_PREFIX, MERMAID_SECURITY_LEVEL } from "./const";
import { mermaidThemeConfig } from "./theme";
import { readDiagramTheme } from "./frontmatter";

type Mermaid = typeof import("mermaid").default;

/** The in-flight (or resolved) library load, created once and shared by every render. */
let loader: Promise<Mermaid> | null = null;

/** Monotonic counter so each render supplies a DOM id Mermaid has not seen — reuse corrupts its cache. */
let renderCounter = 0;

function loadMermaid(): Promise<Mermaid> {
  if (!loader) loader = import("mermaid").then((module) => module.default);
  return loader;
}

/** The message from a thrown value, whether it is an `Error` or a bare string Mermaid rejected with. */
function errorMessage(cause: unknown): string {
  if (cause instanceof Error) return cause.message;
  if (typeof cause === "string") return cause;
  return "Could not render diagram.";
}

/**
 * Mermaid renders into a temporary element keyed by the id it is handed, and on a parse failure it can
 * leave that element (and a `d`-prefixed sibling) orphaned in the document. Removing both after every
 * attempt keeps a stream of failed renders from leaking DOM nodes.
 */
function cleanupRenderArtifacts(id: string): void {
  document.getElementById(id)?.remove();
  document.getElementById(`d${id}`)?.remove();
}

export type RenderResult = { svg: string } | { error: string };

/**
 * Render `source` to a sanitized SVG string, or report the parse error. Initializes Mermaid with the
 * app's current theme on every call so a light/dark flip is reflected, and always runs under the strict
 * security level (DOMPurify-sanitized output, no eval, no iframe) so the SVG is safe to inject and
 * renders under the app's Content-Security-Policy.
 */
export async function renderDiagram(source: string): Promise<RenderResult> {
  const mermaid = await loadMermaid();
  const id = `${MERMAID_ID_PREFIX}-${(renderCounter += 1)}`;
  const theme = mermaidThemeConfig();
  // The app palette is injected only when the diagram follows the app theme (no frontmatter override)
  // or explicitly names the base theme those tokens target. A self-contained theme (dark/forest/
  // neutral) is left to its own palette: mermaid folds these base themeVariables into the frontmatter
  // theme's, so injecting them would bleed base colors onto the chosen theme. The font size rides in
  // `themeVariables` (a CSS string), so mermaid's numeric top-level `fontSize` slot stays unused.
  const declared = readDiagramTheme(source);
  const appTokened = declared === null || declared === theme.theme;
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: MERMAID_SECURITY_LEVEL,
    fontFamily: theme.fontFamily,
    ...(appTokened
      ? { theme: theme.theme, darkMode: theme.darkMode, themeVariables: theme.themeVariables }
      : {}),
  });
  try {
    const { svg } = await mermaid.render(id, source);
    return { svg };
  } catch (cause) {
    return { error: errorMessage(cause) };
  } finally {
    cleanupRenderArtifacts(id);
  }
}

export type ParseResult = { ok: true } | { ok: false; message: string };

/**
 * Validate `source` without rendering it — the cheap check behind a live error state. Mermaid's
 * `parse` throws on invalid input, so a caught throw becomes an `ok: false` with the reported message.
 */
export async function parseDiagram(source: string): Promise<ParseResult> {
  const mermaid = await loadMermaid();
  try {
    await mermaid.parse(source);
    return { ok: true };
  } catch (cause) {
    return { ok: false, message: errorMessage(cause) };
  }
}
