// A bounded most-recently-used cache of rendered diagrams.
//
// A Mermaid render costs roughly half a second for a real diagram, and the panel re-renders on every
// theme change and every debounced edit — so flipping a theme back and forth, or stepping between two
// diagrams, pays that cost again for output that has not changed. Keying on the source together with
// the palette signature makes a repeat instant while keeping a palette change a genuine miss.
//
// The cap is the point: an unbounded map of SVG strings grows with every keystroke that reaches a
// render. `Map` iterates in insertion order, so re-inserting on a hit makes the first key the least
// recently used one to evict.

import { MERMAID_RENDER_CACHE_SIZE } from "./const";

const entries = new Map<string, string>();

/**
 * Joins a key's two parts. A separator that can appear inside a palette signature would make the key
 * ambiguous — `("a", "b c")` and `("a b", "c")` would spell the same key, and one diagram would serve
 * the other's SVG. NUL cannot appear in a signature, so the pairing stays one-to-one.
 */
const SEPARATOR = "\u0000";

/** The cache key for one rendered diagram: its palette signature and its exact source. */
function key(signature: string, source: string): string {
  return `${signature}${SEPARATOR}${source}`;
}

/** The cached SVG for this source under this palette, or undefined. A hit is marked most-recent. */
export function cachedRender(signature: string, source: string): string | undefined {
  const at = key(signature, source);
  const svg = entries.get(at);
  if (svg === undefined) return undefined;
  entries.delete(at);
  entries.set(at, svg);
  return svg;
}

/** Record a rendered SVG, evicting the least recently used entry once the cap is reached. */
export function cacheRender(signature: string, source: string, svg: string): void {
  const at = key(signature, source);
  entries.delete(at);
  entries.set(at, svg);
  while (entries.size > MERMAID_RENDER_CACHE_SIZE) {
    const oldest = entries.keys().next();
    if (oldest.done) return;
    entries.delete(oldest.value);
  }
}

/** Drop every entry — the reset a test needs so one case cannot serve another's render. */
export function clearRenderCache(): void {
  entries.clear();
}
