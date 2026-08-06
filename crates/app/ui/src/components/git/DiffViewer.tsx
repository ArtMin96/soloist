import { useEffect, useMemo, useState } from "react";
import { DiffModeEnum, DiffView } from "@git-diff-view/react";
import "@git-diff-view/react/styles/diff-view.css";
import "@/components/git/diff-view.css";
import { ensureLanguage, HIGHLIGHTER } from "@/lib/diff/highlighter";
import { languageOf } from "@/lib/diff/language";
import type { FileDiff } from "@/domain";

/** How the two sides of a change are laid out. */
export const SIDE_BY_SIDE = "side-by-side" as const;
export const UNIFIED = "unified" as const;
export type DiffLayout = typeof SIDE_BY_SIDE | typeof UNIFIED;

/** The app's data type size, so a diff reads like the terminal beside it. */
const FONT_SIZE = 13;

/**
 * One path's change, hunk by hunk, coloured by the app's shared highlighter.
 *
 * Presentational: it renders the diff it is handed. The patch is whatever version control
 * produced — the core decided which hunks a reader gets and whether there are more.
 */
export function DiffViewer({
  diff,
  layout,
  dark,
}: {
  diff: FileDiff;
  layout: DiffLayout;
  dark: boolean;
}) {
  const language = useMemo(() => languageOf(diff.path), [diff.path]);
  const [highlight, setHighlight] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setHighlight(false);
    // A grammar is fetched, so the diff paints plain first and gains its colour a moment later
    // rather than waiting on a module before showing anything at all.
    void ensureLanguage(language).then((ready) => {
      if (!cancelled) setHighlight(ready);
    });
    return () => {
      cancelled = true;
    };
  }, [language]);

  return (
    <DiffView
      data={{
        oldFile: { fileName: diff.original_path ?? diff.path, fileLang: language },
        newFile: { fileName: diff.path, fileLang: language },
        hunks: [diff.patch],
      }}
      diffViewMode={layout === SIDE_BY_SIDE ? DiffModeEnum.Split : DiffModeEnum.Unified}
      diffViewTheme={dark ? "dark" : "light"}
      diffViewHighlight={highlight}
      diffViewFontSize={FONT_SIZE}
      diffViewWrap
      registerHighlighter={HIGHLIGHTER}
    />
  );
}
