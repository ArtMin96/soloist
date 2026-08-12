// Searching one terminal's scrollback: the addon that finds and marks matches, and the running
// count the find bar reports. Kept beside the terminal hook rather than inside it so the emulator's
// stream lifecycle and its search behavior stay separately readable.

import { useCallback, useEffect, useRef, useState } from "react";
import { SearchAddon, type ISearchOptions } from "@xterm/addon-search";
import type { Terminal } from "@xterm/xterm";
import type { AppliedTheme } from "@/domain";
import { searchDecorationColors } from "@/lib/terminalPalette";

// Ceiling on how many matches the search addon decorates at once. Each match costs a marker and a
// decoration, and a query is re-run on every keystroke against the full scrollback, so this is the
// bound that keeps a one-character query over a long buffer from allocating without limit. It is
// the addon's own default, named here so it is a decision rather than an inherited accident. Stated
// beside the module that constructs the addon, which is where the terminal's addon limits live.
const SEARCH_HIGHLIGHT_LIMIT = 1000;

/** Reported as the active index while no single match is current. */
export const NO_ACTIVE_MATCH = -1;

/** How many matches the current query has, and which one of them the view is standing on. */
export interface SearchMatches {
  /** Zero-based position of the active match, or `NO_ACTIVE_MATCH` when none is singled out. */
  index: number;
  count: number;
}

export const NO_MATCHES: SearchMatches = { index: NO_ACTIVE_MATCH, count: 0 };

/** Stable API for in-terminal text search — backed by SearchAddon once mounted. */
export interface TerminalSearch {
  findNext: (query: string) => void;
  findPrevious: (query: string) => void;
  clear: () => void;
  matches: SearchMatches;
}

// Asking for decorations is what makes every match visible at once, in the output and in the
// overview ruler — and it is also the only way to learn how many there are: the addon reports no
// result counts at all for a search that decorates nothing.
function searchOptions(theme: boolean | AppliedTheme, incremental = false): ISearchOptions {
  return {
    caseSensitive: false,
    regex: false,
    incremental,
    decorations: searchDecorationColors(theme),
  };
}

/**
 * Owns the search addon for one terminal and the match count it reports.
 *
 * `attach` is called by the terminal's creation effect once it has an emulator, and returns the
 * disposer that releases the subscription with it. The returned callbacks are stable across
 * remounts — they read the addon through a ref — so a caller keeps one reference to them.
 */
export function useTerminalSearch(theme: boolean | AppliedTheme) {
  const addonRef = useRef<SearchAddon | null>(null);
  const termRef = useRef<Terminal | null>(null);
  // The query the decorations currently on screen were painted for, or null while nothing is
  // decorated. Held so the repaint below can reissue it without the find bar asking again.
  const queryRef = useRef<string | null>(null);
  const [matches, setMatches] = useState<SearchMatches>(NO_MATCHES);
  // The theme, read at the moment a search runs rather than captured, so the callbacks stay stable
  // and a match decorated after a theme flip is drawn in the new palette.
  const themeRef = useRef(theme);
  useEffect(() => {
    themeRef.current = theme;
  }, [theme]);
  const themeSignature = typeof theme === "boolean" ? String(theme) : theme.signature;

  const attach = useCallback((term: Terminal) => {
    const addon = new SearchAddon({ highlightLimit: SEARCH_HIGHLIGHT_LIMIT });
    term.loadAddon(addon);
    addonRef.current = addon;
    termRef.current = term;
    const results = addon.onDidChangeResults((event) =>
      setMatches({ index: event.resultIndex, count: event.resultCount }),
    );
    return () => {
      results.dispose();
      addonRef.current = null;
      termRef.current = null;
      queryRef.current = null;
      setMatches(NO_MATCHES);
    };
  }, []);

  const findNext = useCallback((query: string) => {
    const addon = addonRef.current;
    if (!addon) return;
    addon.findNext(query, searchOptions(themeRef.current, true));
    queryRef.current = query;
  }, []);

  const findPrevious = useCallback((query: string) => {
    const addon = addonRef.current;
    if (!addon) return;
    // No `incremental` here: the addon expands the current selection only for `findNext`; on
    // `findPrevious` it must step to the prior match, so the flag is deliberately omitted.
    addon.findPrevious(query, searchOptions(themeRef.current));
    queryRef.current = query;
  }, []);

  // Dropping the decorations does not itself report that there are no longer any matches, so the
  // count is reset here as well — otherwise a closed or emptied find bar would reopen still
  // showing the tally from the previous query.
  const clear = useCallback(() => {
    addonRef.current?.clearDecorations();
    queryRef.current = null;
    setMatches(NO_MATCHES);
  }, []);

  // Repaint the matches already on screen in the new palette when the theme flips. The addon takes
  // its colours as an argument to a search and offers no way to restyle what it has drawn, so
  // reissuing the query is the only route to a repaint — without this, highlights keep the palette
  // of whichever theme was current when the user last typed.
  //
  // Dropping the decorations first is what makes the reissue take effect: given the same query and
  // the same matching options, the addon considers its highlights current and re-creates only the
  // active one. Discarding them clears the query it compares against, so all of them are drawn
  // again.
  //
  // `findPrevious` is what keeps the user's place. With that comparison cleared the addon resumes
  // from the *start* of the current selection — the match it put the user on — and finds that same
  // match rather than stepping past it. It also scrolls a match back into view, which a repaint
  // nobody asked for must not do, so the viewport is put back; capture and restore sit in one
  // synchronous block with nothing written in between, so the row stays the row that was captured.
  useEffect(() => {
    const addon = addonRef.current;
    const term = termRef.current;
    const query = queryRef.current;
    if (!addon || !term || !query) return;
    const viewport = term.buffer.active.viewportY;
    addon.clearDecorations();
    addon.findPrevious(query, searchOptions(themeRef.current));
    if (term.buffer.active.viewportY !== viewport) term.scrollToLine(viewport);
  }, [themeSignature]);

  const search: TerminalSearch = { findNext, findPrevious, clear, matches };
  return { attach, search };
}
