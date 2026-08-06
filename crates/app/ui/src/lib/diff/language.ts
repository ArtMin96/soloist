// Which grammar a path is highlighted with, and how that grammar is fetched.
//
// Every grammar is its own module, so naming one here costs nothing until a file of that kind is
// actually opened. The set is deliberately a closed map rather than a computed import path: a
// template-literal import would make the bundler ship every grammar the highlighter publishes —
// hundreds of them, several megabytes — to serve the handful a repository actually contains.

import type { LanguageRegistration } from "shiki/core";

/** What one grammar module resolves to once fetched. */
type Grammar = { default: LanguageRegistration[] };

/**
 * Every grammar the diff surface can highlight with, each behind its own fetch. Adding a
 * language is one entry here and one or more extensions in {@link EXTENSIONS}.
 */
const GRAMMARS: Record<string, () => Promise<Grammar>> = {
  astro: () => import("shiki/langs/astro.mjs"),
  c: () => import("shiki/langs/c.mjs"),
  cpp: () => import("shiki/langs/cpp.mjs"),
  csharp: () => import("shiki/langs/csharp.mjs"),
  css: () => import("shiki/langs/css.mjs"),
  diff: () => import("shiki/langs/diff.mjs"),
  docker: () => import("shiki/langs/docker.mjs"),
  elixir: () => import("shiki/langs/elixir.mjs"),
  go: () => import("shiki/langs/go.mjs"),
  graphql: () => import("shiki/langs/graphql.mjs"),
  haskell: () => import("shiki/langs/haskell.mjs"),
  hcl: () => import("shiki/langs/hcl.mjs"),
  html: () => import("shiki/langs/html.mjs"),
  ini: () => import("shiki/langs/ini.mjs"),
  java: () => import("shiki/langs/java.mjs"),
  javascript: () => import("shiki/langs/javascript.mjs"),
  json: () => import("shiki/langs/json.mjs"),
  jsonc: () => import("shiki/langs/jsonc.mjs"),
  jsx: () => import("shiki/langs/jsx.mjs"),
  kotlin: () => import("shiki/langs/kotlin.mjs"),
  lua: () => import("shiki/langs/lua.mjs"),
  make: () => import("shiki/langs/make.mjs"),
  markdown: () => import("shiki/langs/markdown.mjs"),
  nix: () => import("shiki/langs/nix.mjs"),
  php: () => import("shiki/langs/php.mjs"),
  proto: () => import("shiki/langs/proto.mjs"),
  python: () => import("shiki/langs/python.mjs"),
  ruby: () => import("shiki/langs/ruby.mjs"),
  rust: () => import("shiki/langs/rust.mjs"),
  scss: () => import("shiki/langs/scss.mjs"),
  shellscript: () => import("shiki/langs/shellscript.mjs"),
  sql: () => import("shiki/langs/sql.mjs"),
  svelte: () => import("shiki/langs/svelte.mjs"),
  swift: () => import("shiki/langs/swift.mjs"),
  toml: () => import("shiki/langs/toml.mjs"),
  tsx: () => import("shiki/langs/tsx.mjs"),
  typescript: () => import("shiki/langs/typescript.mjs"),
  vue: () => import("shiki/langs/vue.mjs"),
  xml: () => import("shiki/langs/xml.mjs"),
  yaml: () => import("shiki/langs/yaml.mjs"),
  zig: () => import("shiki/langs/zig.mjs"),
};

/**
 * The languages the highlighter is built with, so the kinds of file this repository is mostly
 * made of are coloured the moment a diff opens rather than a fetch later. Everything else in
 * {@link GRAMMARS} arrives when a file of its kind is first opened.
 */
export const STARTING_LANGUAGES = [
  "typescript",
  "javascript",
  "tsx",
  "rust",
  "toml",
  "yaml",
  "json",
  "css",
  "markdown",
] as const;

/** What a path's last extension means, for the extensions that are not the language's own name. */
const EXTENSIONS: Record<string, string> = {
  bash: "shellscript",
  cc: "cpp",
  cjs: "javascript",
  cs: "csharp",
  cxx: "cpp",
  ex: "elixir",
  exs: "elixir",
  h: "c",
  hpp: "cpp",
  hs: "haskell",
  htm: "html",
  js: "javascript",
  kt: "kotlin",
  kts: "kotlin",
  md: "markdown",
  mjs: "javascript",
  mk: "make",
  patch: "diff",
  py: "python",
  rb: "ruby",
  rs: "rust",
  sh: "shellscript",
  tf: "hcl",
  ts: "typescript",
  yml: "yaml",
  zsh: "shellscript",
};

/** Files whose whole name, rather than an extension, says what they are. */
const FILENAMES: Record<string, string> = {
  dockerfile: "docker",
  gemfile: "ruby",
  makefile: "make",
  rakefile: "ruby",
};

/**
 * Which grammar `path` should be highlighted with, or `null` when none of them fits — in which
 * case the diff is shown as plain text rather than coloured as something it is not.
 */
export function languageOf(path: string): string | null {
  const name = path.slice(path.lastIndexOf("/") + 1).toLowerCase();
  const byName = FILENAMES[name];
  if (byName !== undefined) return byName;

  const dot = name.lastIndexOf(".");
  if (dot < 0) return null;
  const extension = name.slice(dot + 1);
  const language = EXTENSIONS[extension] ?? extension;
  return language in GRAMMARS ? language : null;
}

/** Fetches one grammar. Only ever called with a language {@link languageOf} named. */
export function grammarOf(language: string): Promise<Grammar> | null {
  return GRAMMARS[language]?.() ?? null;
}
