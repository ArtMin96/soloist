// The one syntax highlighter the diff and preview surfaces share.
//
// Built from the highlighter's fine-grained entry rather than its bundled one: the bundled entry
// carries a map of every grammar and theme it publishes, which the bundler then ships in full.
// Here the engine, the two themes, and each grammar are separate modules, so what is fetched is
// what is actually being read. The regular-expression engine is the JavaScript one rather than
// the WebAssembly one, so nothing has to load — or be allowed by the content policy — beyond the
// modules themselves.

import { processAST, type DiffAST, type DiffFileHighlighter } from "@git-diff-view/react";
import { createHighlighterCore, type HighlighterCore, type ThemeRegistrationRaw } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";
import { grammarOf, STARTING_LANGUAGES } from "@/lib/diff/language";
import type { AppliedTheme } from "@/domain";
import { contrastSafeThemeColor } from "@/theme/derive";
import { defaultAppliedTheme } from "@/theme/runtime";

// One registry slot is replaced whenever the applied palette changes. The token rules are derived
// from that same palette, so Shiki remains a grammar engine rather than a second theme authority.
const ACTIVE_THEME = "soloist-active";

// What the diff stylesheet reads each token's colour from. Both sides are emitted as custom
// properties under this prefix, and the stylesheet picks one by the theme on its wrapper.
const CSS_VARIABLE_PREFIX = "--diff-view-";

// Past this many lines a file is left uncoloured. Highlighting is a per-line walk, and a
// generated file long enough to matter is one nobody reads line by line anyway.
const MAX_LINE_TO_HIGHLIGHT = 2000;

/** The engine once it is usable, paired with the palette actually sitting in its one theme slot. */
type Ready = { core: HighlighterCore; theme: string };

let ready: Ready | null = null;
// Resolves; never rejects. A failed start is a `null` value rather than a thrown one, so every
// reader below agrees on "not ready" instead of two of them swallowing it and one throwing.
let starting: Promise<Ready | null> | null = null;
// Holds only successful grammar loads. A failed one deletes its own entry (in `ensureLanguage`)
// instead of caching the rejection, so a transient chunk failure costs one call, not the rest of
// the page's life. A failed engine start is retried the same way — `start` clears `starting`
// back to `null` on failure rather than leaving that attempt in place.
const grammars = new Map<string, Promise<boolean>>();

let maxLineToIgnoreSyntax = MAX_LINE_TO_HIGHLIGHT;
const ignoreSyntaxHighlightList: (string | RegExp)[] = [];

/**
 * The highlighter the diff viewer is handed. One object for the life of the page, so handing it
 * to a component never looks like a new highlighter and never re-renders one.
 *
 * `getAST` is synchronous by the viewer's contract, so a grammar has to be in place before a
 * file that needs it is rendered — which is what {@link ensureLanguage} is for. Until then the
 * viewer asks `hasRegisteredCurrentLang` and shows the file as plain text.
 */
export const HIGHLIGHTER: DiffFileHighlighter = {
  name: "shiki",
  type: "class",
  get maxLineToIgnoreSyntax() {
    return maxLineToIgnoreSyntax;
  },
  setMaxLineToIgnoreSyntax: (lines) => {
    maxLineToIgnoreSyntax = lines;
  },
  get ignoreSyntaxHighlightList() {
    return ignoreSyntaxHighlightList;
  },
  setIgnoreSyntaxHighlightList: (paths) => {
    ignoreSyntaxHighlightList.splice(0, ignoreSyntaxHighlightList.length, ...paths);
  },
  processAST,
  hasRegisteredCurrentLang: (language) =>
    ready?.core.getLoadedLanguages().includes(language) ?? false,
  getAST: (raw, _fileName, language) =>
    ready?.core.codeToHast(raw, {
      lang: language ?? "",
      themes: { light: ACTIVE_THEME, dark: ACTIVE_THEME },
      cssVariablePrefix: CSS_VARIABLE_PREFIX,
      // Neither theme is the default one, so every token carries both and the stylesheet
      // chooses — which is how a theme flip recolours without a second pass.
      defaultColor: false,
      mergeWhitespaces: false,
    }) as DiffAST,
};

/**
 * Makes the highlighter ready to colour `language`, fetching the engine, the themes, and the
 * grammar as needed. Resolves to whether it can. `false` now covers two cases a caller does not
 * need to tell apart: there is no grammar for `language` at all, or the engine or the grammar
 * failed to load. Either way the caller shows plain text rather than pretending — it never
 * rejects, so a failure never becomes an unhandled rejection at a fire-and-forget call site.
 *
 * Every fetch is remembered while it is in flight, so a burst of files in one language costs one
 * of them — but only a *successful* fetch stays remembered; a failed one is retried on the next
 * call instead of staying broken for the life of the page.
 */
export async function ensureLanguage(language: string | null): Promise<boolean> {
  const state = await (starting ??= start());
  if (state === null || language === null) return false;
  if (state.core.getLoadedLanguages().includes(language)) return true;

  const grammar = grammarOf(language);
  if (grammar === null) return false;
  await (grammars.get(language) ??
    grammars
      .set(
        language,
        grammar
          .then((module) => state.core.loadLanguage(module.default))
          .then(() => true)
          .catch(() => {
            grammars.delete(language);
            return false;
          }),
      )
      .get(language));
  return state.core.getLoadedLanguages().includes(language);
}

/** Load both the grammar and the live app palette before a highlighted surface is revealed. */
export async function ensureHighlighting(
  language: string | null,
  theme: AppliedTheme,
): Promise<boolean> {
  const state = await (starting ??= start());
  if (state === null) return false;
  if (state.theme !== theme.signature) {
    try {
      await state.core.loadTheme(syntaxTheme(theme));
      // One write, on the object the palette actually lives in — not a second variable beside it.
      state.theme = theme.signature;
    } catch {
      return false;
    }
  }
  return ensureLanguage(language);
}

/**
 * One whole file as highlighted markup, for the preview that shows a file rather than a change
 * to one. `null` when the grammar is not in place — the caller shows the text plainly instead.
 *
 * The markup is the highlighter's own: it escapes the text it colours, so what comes back is the
 * file rendered, never the file executed.
 */
export function highlightedHtml(
  code: string,
  language: string,
  theme: AppliedTheme | boolean,
): string | null {
  if (ready === null || !ready.core.getLoadedLanguages().includes(language)) return null;
  const applied = typeof theme === "boolean" ? defaultAppliedTheme(theme) : theme;
  if (ready.theme !== applied.signature) ready.core.loadThemeSync(syntaxTheme(applied));
  ready.theme = applied.signature;
  return ready.core.codeToHtml(code, { lang: language, theme: ACTIVE_THEME });
}

/**
 * Builds the engine with the default app palette and the languages worth having up front.
 * Resolves to `null` on failure rather than rejecting, and clears `starting` back to `null`
 * first, so the next call attempts a fresh start instead of replaying the same failure forever.
 */
async function start(): Promise<Ready | null> {
  const initialTheme = defaultAppliedTheme(false);
  try {
    const core = await createHighlighterCore({
      themes: [syntaxTheme(initialTheme)],
      langs: STARTING_LANGUAGES.map(grammarOf).filter((grammar) => grammar !== null),
      engine: createJavaScriptRegexEngine({ forgiving: true }),
    });
    ready = { core, theme: initialTheme.signature };
    return ready;
  } catch {
    starting = null;
    return null;
  }
}

/** TextMate scopes projected onto semantic colors from the active Soloist palette. */
function syntaxTheme(theme: AppliedTheme): ThemeRegistrationRaw {
  const { colors, extensions } = theme;
  const syntax = (color: string) => contrastSafeThemeColor(color, [colors.codeBackground]);
  const invalid = contrastSafeThemeColor(colors.error, [colors.errorSurface]);
  return {
    name: ACTIVE_THEME,
    type: theme.appearance,
    colors: {
      "editor.background": colors.codeBackground,
      "editor.foreground": colors.codeForeground,
    },
    settings: [
      { settings: { background: colors.codeBackground, foreground: colors.codeForeground } },
      {
        scope: ["comment", "punctuation.definition.comment", "string.comment"],
        settings: { foreground: syntax(colors.textMuted), fontStyle: "italic" },
      },
      {
        scope: ["keyword", "storage", "keyword.control", "keyword.operator.new"],
        settings: { foreground: syntax(colors.error) },
      },
      {
        scope: ["string", "constant.other.symbol", "markup.inline.raw.string"],
        settings: { foreground: syntax(extensions.statusRunning) },
      },
      {
        scope: ["constant.numeric", "constant.language", "constant.character"],
        settings: { foreground: syntax(colors.warning) },
      },
      {
        scope: ["entity.name.function", "support.function", "meta.function-call"],
        settings: { foreground: syntax(colors.update) },
      },
      {
        scope: ["entity.name.type", "entity.name.class", "support.type", "support.class"],
        settings: { foreground: syntax(extensions.fileLanguageViolet) },
      },
      {
        scope: ["variable", "variable.other", "meta.object-literal.key"],
        settings: { foreground: syntax(colors.codeForeground) },
      },
      {
        scope: ["entity.name.tag", "support.class.component"],
        settings: { foreground: syntax(colors.error) },
      },
      {
        scope: ["entity.other.attribute-name", "support.type.property-name"],
        settings: { foreground: syntax(colors.accent) },
      },
      {
        scope: ["markup.heading", "markup.bold"],
        settings: { foreground: syntax(colors.accent), fontStyle: "bold" },
      },
      {
        scope: ["invalid", "invalid.illegal"],
        settings: { foreground: invalid, background: colors.errorSurface },
      },
    ],
  };
}
