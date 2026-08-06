// The one syntax highlighter the diff and preview surfaces share.
//
// Built from the highlighter's fine-grained entry rather than its bundled one: the bundled entry
// carries a map of every grammar and theme it publishes, which the bundler then ships in full.
// Here the engine, the two themes, and each grammar are separate modules, so what is fetched is
// what is actually being read. The regular-expression engine is the JavaScript one rather than
// the WebAssembly one, so nothing has to load — or be allowed by the content policy — beyond the
// modules themselves.

import { processAST, type DiffAST, type DiffFileHighlighter } from "@git-diff-view/react";
import { createHighlighterCore, type HighlighterCore } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";
import { grammarOf, STARTING_LANGUAGES } from "@/lib/diff/language";

// The theme names the diff stylesheet expects on each side of the light/dark flip. Every token
// carries both, so switching theme recolours the diff already on screen without re-highlighting.
const LIGHT = "github-light";
const DARK = "github-dark";

// What the diff stylesheet reads each token's colour from. Both sides are emitted as custom
// properties under this prefix, and the stylesheet picks one by the theme on its wrapper.
const CSS_VARIABLE_PREFIX = "--diff-view-";

// Past this many lines a file is left uncoloured. Highlighting is a per-line walk, and a
// generated file long enough to matter is one nobody reads line by line anyway.
const MAX_LINE_TO_HIGHLIGHT = 2000;

let engine: HighlighterCore | null = null;
let starting: Promise<HighlighterCore> | null = null;
const loaded = new Map<string, Promise<void>>();

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
  hasRegisteredCurrentLang: (language) => engine?.getLoadedLanguages().includes(language) ?? false,
  getAST: (raw, _fileName, language) =>
    engine?.codeToHast(raw, {
      lang: language ?? "",
      themes: { light: LIGHT, dark: DARK },
      cssVariablePrefix: CSS_VARIABLE_PREFIX,
      // Neither theme is the default one, so every token carries both and the stylesheet
      // chooses — which is how a theme flip recolours without a second pass.
      defaultColor: false,
      mergeWhitespaces: false,
    }) as DiffAST,
};

/**
 * Makes the highlighter ready to colour `language`, fetching the engine, the themes, and the
 * grammar as needed. Resolves to whether it can — a language with no grammar here resolves
 * `false`, and the caller shows plain text rather than pretending.
 *
 * Every fetch is remembered, so a burst of files in one language costs one of them.
 */
export async function ensureLanguage(language: string | null): Promise<boolean> {
  const core = await (starting ??= start());
  if (language === null) return false;
  if (core.getLoadedLanguages().includes(language)) return true;

  const grammar = grammarOf(language);
  if (grammar === null) return false;
  await (loaded.get(language) ??
    loaded
      .set(
        language,
        grammar.then((module) => core.loadLanguage(module.default)),
      )
      .get(language));
  return core.getLoadedLanguages().includes(language);
}

/**
 * One whole file as highlighted markup, for the preview that shows a file rather than a change
 * to one. `null` when the grammar is not in place — the caller shows the text plainly instead.
 *
 * The markup is the highlighter's own: it escapes the text it colours, so what comes back is the
 * file rendered, never the file executed.
 */
export function highlightedHtml(code: string, language: string, dark: boolean): string | null {
  if (engine?.getLoadedLanguages().includes(language) !== true) return null;
  return engine.codeToHtml(code, { lang: language, theme: dark ? DARK : LIGHT });
}

/** Builds the engine with the two themes and the languages worth having up front. */
async function start(): Promise<HighlighterCore> {
  const core = await createHighlighterCore({
    themes: [import("shiki/themes/github-light.mjs"), import("shiki/themes/github-dark.mjs")],
    langs: STARTING_LANGUAGES.map(grammarOf).filter((grammar) => grammar !== null),
    engine: createJavaScriptRegexEngine({ forgiving: true }),
  });
  engine = core;
  return core;
}
