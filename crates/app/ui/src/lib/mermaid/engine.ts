// The Mermaid rendering engine, and the one module that imports the library. Mermaid pulls in a large
// transitive tree, so it is loaded through a single dynamic `import("mermaid")` confined here: the
// bundler splits it into its own chunk that never enters the initial payload, and it is fetched the
// first time a diagram actually renders. Every caller goes through `renderDiagram`/`parseDiagram`, so
// nothing else in the app touches Mermaid's surface.

import { MERMAID_FONT_SIZE, MERMAID_ID_PREFIX, MERMAID_SECURITY_LEVEL } from "./const";
import { cacheRender, cachedRender } from "./renderCache";
import { mermaidThemeConfig, themeSignature } from "./theme";
import { readDiagramTheme } from "./frontmatter";
import { withIntrinsicSize } from "./svgSize";

type Mermaid = typeof import("mermaid").default;

/** The in-flight (or resolved) library load, created once and shared by every render. */
let loader: Promise<Mermaid> | null = null;

/** Monotonic counter so each render supplies a DOM id Mermaid has not seen — reuse corrupts its cache. */
let renderCounter = 0;

/**
 * The tail of the render queue. Mermaid keeps its configuration in module-level state that
 * `initialize` overwrites and `render` then reads asynchronously, so two overlapping renders of
 * differently themed sources would draw each other's palette. Chaining every render onto the last one
 * keeps configure-then-draw atomic.
 *
 * Its depth is bounded by its callers, which is the only place a superseded render can be recognised:
 * a diagram surface keeps at most one render outstanding (`useDiagramRender`) and replaces its own
 * pending source rather than queueing a second, and the remaining callers are one-shot user actions
 * (copy, export). So the queue holds at most one entry per mounted surface plus the export in hand —
 * it cannot grow with the length of an editing session. Dropping work here instead would be wrong in
 * both directions: an export's render must always run, and the surface whose entry was evicted is not
 * necessarily the surface that queued the newer one.
 */
let queue: Promise<unknown> = Promise.resolve();

function loadMermaid(): Promise<Mermaid> {
  if (!loader) loader = import("mermaid").then((module) => module.default);
  return loader;
}

/** Run `work` after every render already queued, whatever their outcome. */
function enqueue<T>(work: () => Promise<T>): Promise<T> {
  const result = queue.then(work, work);
  queue = result.catch(() => undefined);
  return result;
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
 * Configure Mermaid for `source` and draw it. The app palette is injected only when the diagram
 * follows the app theme (no frontmatter override) or explicitly names the base theme those tokens
 * target. A self-contained theme (dark/forest/neutral) is left to its own palette: Mermaid folds the
 * base theme variables into the frontmatter theme's, so injecting them would bleed base colours onto
 * the chosen theme. The font size is passed either way so a diagram is sized to the app's dense body
 * type whichever palette draws it.
 */
async function draw(source: string): Promise<string> {
  const mermaid = await loadMermaid();
  const theme = mermaidThemeConfig();
  // Read beside the palette it names, never before the queue was joined. A signature taken at call
  // time can be stale by the time the render reaches the front of the queue — a theme flipped in
  // between would have this render drawn in the new palette and then filed under the old one, and the
  // cache would serve that wrong-palette SVG for as long as the entry lived.
  const signature = themeSignature();
  const declared = readDiagramTheme(source);
  const appTokened = declared === null || declared === theme.theme;
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: MERMAID_SECURITY_LEVEL,
    fontFamily: theme.fontFamily,
    ...(appTokened
      ? { theme: theme.theme, themeVariables: theme.themeVariables }
      : { themeVariables: { fontSize: MERMAID_FONT_SIZE } }),
  });
  const id = `${MERMAID_ID_PREFIX}-${(renderCounter += 1)}`;
  try {
    const { svg } = await mermaid.render(id, source);
    const sized = withIntrinsicSize(svg);
    cacheRender(signature, source, sized);
    return sized;
  } finally {
    cleanupRenderArtifacts(id);
  }
}

/**
 * Render `source` to a sanitized SVG string, or report why it could not be drawn. Runs under the
 * strict security level (DOMPurify-sanitized output, no eval, no iframe) so the SVG is safe to inject
 * and renders under the app's Content-Security-Policy.
 *
 * Never rejects: a malformed diagram, a palette Mermaid refuses, and a library chunk that fails to
 * load all resolve to an `error` a caller can show. A rejection here would leave the surfaces that
 * await it with no result and no failure — a diagram that never arrives and never explains itself.
 */
export async function renderDiagram(source: string): Promise<RenderResult> {
  const cached = cachedRender(themeSignature(), source);
  if (cached !== undefined) return { svg: cached };
  try {
    return { svg: await enqueue(() => draw(source)) };
  } catch (cause) {
    return { error: errorMessage(cause) };
  }
}

export type ParseResult = { ok: true } | { ok: false; message: string };

/**
 * Validate `source` without rendering it — the cheap check behind a live error state. Mermaid's
 * `parse` throws on invalid input, so a caught throw becomes an `ok: false` with the reported message.
 */
export async function parseDiagram(source: string): Promise<ParseResult> {
  try {
    const mermaid = await loadMermaid();
    await mermaid.parse(source);
    return { ok: true };
  } catch (cause) {
    return { ok: false, message: errorMessage(cause) };
  }
}
