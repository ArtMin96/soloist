import { useEffect, useMemo, useState, type ReactNode } from "react";
import { DiffModeEnum, DiffView } from "@git-diff-view/react";
import "@git-diff-view/react/styles/diff-view.css";
import "@/components/git/diff-view.css";
import { ensureHighlighting, HIGHLIGHTER } from "@/lib/diff/highlighter";
import { languageOf } from "@/lib/diff/language";
import type { AppliedTheme, FileDiff, HunkRange } from "@/domain";
import { defaultAppliedTheme } from "@/theme/runtime";

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
 *
 * `actions` is rendered once per hunk, attached to the line the hunk starts on. It is attached
 * by *line*, not by position in a list, so the viewer is free to mount and unmount rows as it
 * likes without an action ending up beside the wrong change.
 */
export function DiffViewer({
  diff,
  layout,
  dark,
  theme,
  actions,
}: {
  diff: FileDiff;
  layout: DiffLayout;
  dark: boolean;
  theme?: AppliedTheme;
  actions?: (hunk: HunkRange) => ReactNode;
}) {
  const language = useMemo(() => languageOf(diff.path), [diff.path]);
  const appliedTheme = theme ?? defaultAppliedTheme(dark);
  const [highlight, setHighlight] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setHighlight(false);
    // A grammar is fetched, so the diff paints plain first and gains its colour a moment later
    // rather than waiting on a module before showing anything at all.
    void ensureHighlighting(language, appliedTheme).then((ready) => {
      if (!cancelled) setHighlight(ready);
    });
    return () => {
      cancelled = true;
    };
  }, [appliedTheme, language]);

  const attached = useMemo(() => (actions ? attach(diff.hunks) : undefined), [actions, diff.hunks]);

  return (
    <DiffView<HunkRange>
      data={{
        oldFile: { fileName: diff.original_path ?? diff.path, fileLang: language },
        newFile: { fileName: diff.path, fileLang: language },
        hunks: [diff.patch],
      }}
      extendData={attached}
      renderExtendLine={actions ? ({ data }) => actions(data) : undefined}
      diffViewMode={layout === SIDE_BY_SIDE ? DiffModeEnum.Split : DiffModeEnum.Unified}
      diffViewTheme={dark ? "dark" : "light"}
      diffViewHighlight={highlight}
      diffViewFontSize={FONT_SIZE}
      diffViewWrap
      registerHighlighter={HIGHLIGHTER}
    />
  );
}

/**
 * Each hunk against the line it starts on, so its actions render there.
 *
 * A hunk that only removes lines has no line on the new side to hang from, so it hangs from the
 * old one instead — which is where a reader looking at that hunk is looking anyway.
 */
function attach(hunks: HunkRange[]): {
  oldFile: Record<string, { data: HunkRange }>;
  newFile: Record<string, { data: HunkRange }>;
} {
  const oldFile: Record<string, { data: HunkRange }> = {};
  const newFile: Record<string, { data: HunkRange }> = {};
  for (const hunk of hunks) {
    if (hunk.new_lines > 0) newFile[String(hunk.new_start)] = { data: hunk };
    else if (hunk.old_lines > 0) oldFile[String(hunk.old_start)] = { data: hunk };
  }
  return { oldFile, newFile };
}
