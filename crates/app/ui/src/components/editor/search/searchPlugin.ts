import { Plugin, PluginKey, type EditorState } from "@tiptap/pm/state";
import { Decoration, DecorationSet, type EditorView } from "@tiptap/pm/view";
import type { Node as ProseMirrorNode } from "@tiptap/pm/model";

// The decoration classes the find bar's CSS styles: a calm wash on every match, the azure accent on
// the one the caret is parked on. Named once here so the plugin and the stylesheet cannot drift.
export const SEARCH_MATCH_CLASS = "search-match";
export const SEARCH_MATCH_CURRENT_CLASS = "search-match--current";

/** A half-open range, reused for both string offsets (from the pure matcher) and document positions. */
export interface MatchRange {
  from: number;
  to: number;
}

/**
 * Every occurrence of `query` in `text`, case-insensitive and non-overlapping, as string offsets. An
 * empty query matches nothing. Pure and editor-free so the matching rule is unit-testable on its own;
 * the plugin layers document positions on top of it.
 */
export function findRanges(text: string, query: string): MatchRange[] {
  if (query.length === 0) return [];
  const haystack = text.toLowerCase();
  const needle = query.toLowerCase();
  const ranges: MatchRange[] = [];
  let cursor = 0;
  for (;;) {
    const index = haystack.indexOf(needle, cursor);
    if (index === -1) break;
    ranges.push({ from: index, to: index + needle.length });
    // Resume past this match so overlapping hits ("aa" in "aaaa") are counted once, not twice.
    cursor = index + needle.length;
  }
  return ranges;
}

/** The next match index in `direction`, wrapping around at both ends. Zero when there are none. */
export function stepIndex(current: number, total: number, direction: 1 | -1): number {
  if (total === 0) return 0;
  return (current + direction + total) % total;
}

interface SearchPluginState {
  query: string;
  currentIndex: number;
  matches: MatchRange[];
  decorations: DecorationSet;
}

type SearchMeta = { type: "setQuery"; query: string } | { type: "step"; direction: 1 | -1 };

export const searchPluginKey = new PluginKey<SearchPluginState>("editorSearch");

// Walk the document's text nodes and lift each string match up into a document-position range. A
// match is confined to a single text run — good enough for a note's find, and it keeps positions exact.
function collectMatches(doc: ProseMirrorNode, query: string): MatchRange[] {
  if (query.length === 0) return [];
  const matches: MatchRange[] = [];
  doc.descendants((node, pos) => {
    const text = node.isText ? node.text : undefined;
    if (!text) return;
    for (const range of findRanges(text, query)) {
      matches.push({ from: pos + range.from, to: pos + range.to });
    }
  });
  return matches;
}

function buildDecorations(
  doc: ProseMirrorNode,
  matches: MatchRange[],
  currentIndex: number,
): DecorationSet {
  if (matches.length === 0) return DecorationSet.empty;
  const decorations = matches.map((match, index) =>
    Decoration.inline(match.from, match.to, {
      class:
        index === currentIndex
          ? `${SEARCH_MATCH_CLASS} ${SEARCH_MATCH_CURRENT_CLASS}`
          : SEARCH_MATCH_CLASS,
    }),
  );
  return DecorationSet.create(doc, decorations);
}

// Rebuild the whole search state against a document. Used both when the query changes and when the
// note is edited underneath an open find bar — the latter shifts positions and can add or drop hits.
function recompute(doc: ProseMirrorNode, query: string, preferredIndex: number): SearchPluginState {
  const matches = collectMatches(doc, query);
  const currentIndex =
    matches.length === 0 ? 0 : Math.min(Math.max(preferredIndex, 0), matches.length - 1);
  return {
    query,
    currentIndex,
    matches,
    decorations: buildDecorations(doc, matches, currentIndex),
  };
}

// The always-present, idle-until-queried find plugin. It owns the match set and the current index,
// updated only through transaction metas, and renders them as inline decorations.
export const searchPlugin = new Plugin<SearchPluginState>({
  key: searchPluginKey,
  state: {
    init: () => ({ query: "", currentIndex: 0, matches: [], decorations: DecorationSet.empty }),
    apply(tr, value) {
      const meta = tr.getMeta(searchPluginKey) as SearchMeta | undefined;

      if (meta?.type === "setQuery") {
        // A fresh query always lands on the first match.
        return recompute(tr.doc, meta.query, 0);
      }

      if (meta?.type === "step") {
        const currentIndex = stepIndex(value.currentIndex, value.matches.length, meta.direction);
        return {
          ...value,
          currentIndex,
          decorations: buildDecorations(tr.doc, value.matches, currentIndex),
        };
      }

      if (tr.docChanged && value.query.length > 0) {
        return recompute(tr.doc, value.query, value.currentIndex);
      }

      return value;
    },
  },
  props: {
    decorations(state) {
      return searchPluginKey.getState(state)?.decorations ?? DecorationSet.empty;
    },
  },
});

// Bring the current match into view without moving the selection or touching history — the caret
// belongs to the find input while the bar is open.
function scrollCurrentMatchIntoView(view: EditorView): void {
  const state = searchPluginKey.getState(view.state);
  const match = state?.matches[state.currentIndex];
  if (!match) return;
  const { node } = view.domAtPos(match.from);
  const element = node instanceof Element ? node : node.parentElement;
  element?.scrollIntoView({ block: "nearest", inline: "nearest" });
}

/** Set the live search query; an empty string clears the highlights. */
export function setSearchQuery(view: EditorView, query: string): void {
  view.dispatch(view.state.tr.setMeta(searchPluginKey, { type: "setQuery", query }));
  scrollCurrentMatchIntoView(view);
}

/** Move the current match forward (`1`) or back (`-1`), wrapping around at both ends. */
export function stepSearch(view: EditorView, direction: 1 | -1): void {
  const state = searchPluginKey.getState(view.state);
  if (!state || state.matches.length === 0) return;
  view.dispatch(view.state.tr.setMeta(searchPluginKey, { type: "step", direction }));
  scrollCurrentMatchIntoView(view);
}

/** Clear the query so no decorations remain. */
export function clearSearch(view: EditorView): void {
  setSearchQuery(view, "");
}

/** The find bar's read model: how many matches, and which one is current (0-based). */
export function getSearchStatus(state: EditorState): { total: number; currentIndex: number } {
  const value = searchPluginKey.getState(state);
  if (!value) return { total: 0, currentIndex: 0 };
  return { total: value.matches.length, currentIndex: value.currentIndex };
}
