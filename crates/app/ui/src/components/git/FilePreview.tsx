import { useEffect, useMemo, useState } from "react";
import { ensureHighlighting, highlightedHtml } from "@/lib/diff/highlighter";
import { languageOf } from "@/lib/diff/language";
import type { AppliedTheme, FileContent } from "@/domain";
import { defaultAppliedTheme } from "@/theme/runtime";

/**
 * Past this many lines a file is shown plainly. Colouring is a per-line walk, and a file long
 * enough to cross this is one nobody reads line by line anyway.
 */
const MAX_LINES_TO_HIGHLIGHT = 5000;

/** The classes both readings share, so plain and coloured text sit on the same grid. */
const TEXT = "min-w-0 px-4 py-3 font-mono text-[0.8125rem] leading-[1.4]";

/**
 * One file as the working tree holds it, read-only.
 *
 * Presentational: the text arrives through the façade like every other repository read — nothing
 * here touches a filesystem — already bounded, so a file too large to carry arrives cut rather
 * than being fetched again in full.
 */
export function FilePreview({
  path,
  content,
  dark,
  theme,
}: {
  path: string;
  content: FileContent;
  dark: boolean;
  theme?: AppliedTheme;
}) {
  const language = useMemo(() => languageOf(path), [path]);
  const html = useHighlighted(content.text, language, theme ?? defaultAppliedTheme(dark));

  if (content.text === null) return null;
  return html === null ? (
    <pre className={TEXT}>{content.text}</pre>
  ) : (
    // The highlighter escapes the text it colours, so this is the file rendered, never the file
    // executed.
    <div className={`${TEXT} [&_pre]:!bg-transparent`} dangerouslySetInnerHTML={{ __html: html }} />
  );
}

/**
 * The file's markup once its grammar is in place — null while it is not, and null for a file
 * with no grammar or too many lines to be worth the walk, which the caller shows plainly.
 */
function useHighlighted(
  text: string | null,
  language: string | null,
  theme: AppliedTheme,
): string | null {
  // What `ensureHighlighting` last resolved, so `ready` below reads false again the instant
  // `language`/`theme` move past it, rather than staying stuck on a stale grammar's answer.
  const [loaded, setLoaded] = useState<{ language: string | null; theme: AppliedTheme } | null>(
    null,
  );

  useEffect(() => {
    let cancelled = false;
    void ensureHighlighting(language, theme).then((ok) => {
      if (!cancelled && ok) setLoaded({ language, theme });
    });
    return () => {
      cancelled = true;
    };
  }, [language, theme]);

  const ready = loaded !== null && loaded.language === language && loaded.theme === theme;

  return useMemo(() => {
    if (!ready || text === null || language === null) return null;
    if (text.split("\n", MAX_LINES_TO_HIGHLIGHT + 1).length > MAX_LINES_TO_HIGHLIGHT) return null;
    return highlightedHtml(text, language, theme);
  }, [language, ready, text, theme]);
}
