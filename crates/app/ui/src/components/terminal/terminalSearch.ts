// Searching one terminal's scrollback: the addon that finds and marks matches, and the running
// count the find bar reports. Kept beside the terminal hook rather than inside it so the emulator's
// stream lifecycle and its search behavior stay separately readable.

import { useCallback, useRef, useState } from "react";
import { SearchAddon, type ISearchOptions } from "@xterm/addon-search";
import type { Terminal } from "@xterm/xterm";
import { TERMINAL_SEARCH_HIGHLIGHT_LIMIT } from "@/lib/appearance";
import { searchDecorationColors } from "@/lib/terminalPalette";

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
function searchOptions(dark: boolean, incremental = false): ISearchOptions {
  return {
    caseSensitive: false,
    regex: false,
    incremental,
    decorations: searchDecorationColors(dark),
  };
}

/**
 * Owns the search addon for one terminal and the match count it reports.
 *
 * `attach` is called by the terminal's creation effect once it has an emulator, and returns the
 * disposer that releases the subscription with it. The returned callbacks are stable across
 * remounts — they read the addon through a ref — so a caller keeps one reference to them.
 */
export function useTerminalSearch(isDark: () => boolean) {
  const addonRef = useRef<SearchAddon | null>(null);
  const [matches, setMatches] = useState<SearchMatches>(NO_MATCHES);
  // The theme, read at the moment a search runs rather than captured, so the callbacks stay stable
  // and a match decorated after a theme flip is drawn in the new palette.
  const darkRef = useRef(isDark);
  darkRef.current = isDark;

  const attach = useCallback((term: Terminal) => {
    const addon = new SearchAddon({ highlightLimit: TERMINAL_SEARCH_HIGHLIGHT_LIMIT });
    term.loadAddon(addon);
    addonRef.current = addon;
    const results = addon.onDidChangeResults((event) =>
      setMatches({ index: event.resultIndex, count: event.resultCount }),
    );
    return () => {
      results.dispose();
      addonRef.current = null;
      setMatches(NO_MATCHES);
    };
  }, []);

  const findNext = useCallback((query: string) => {
    addonRef.current?.findNext(query, searchOptions(darkRef.current(), true));
  }, []);

  const findPrevious = useCallback((query: string) => {
    // No `incremental` here: the addon expands the current selection only for `findNext`; on
    // `findPrevious` it must step to the prior match, so the flag is deliberately omitted.
    addonRef.current?.findPrevious(query, searchOptions(darkRef.current()));
  }, []);

  // Dropping the decorations does not itself report that there are no longer any matches, so the
  // count is reset here as well — otherwise a closed or emptied find bar would reopen still
  // showing the tally from the previous query.
  const clear = useCallback(() => {
    addonRef.current?.clearDecorations();
    setMatches(NO_MATCHES);
  }, []);

  const search: TerminalSearch = { findNext, findPrevious, clear, matches };
  return { attach, search };
}
