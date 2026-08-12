#!/usr/bin/env node

import { readdir, readFile } from "node:fs/promises";
import { extname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repository = resolve(fileURLToPath(new URL("..", import.meta.url)));
const sourceRoot = resolve(repository, "crates/app/ui/src");
const sourceExtensions = new Set([".ts", ".tsx", ".css", ".html"]);

// This is a parser probe, not paint: Mermaid accepts legacy sRGB strings only, and the canvas
// sentinel detects whether assigning a candidate color changed the context. Keep this exception
// exact so it cannot grow into a general-purpose color allowlist.
const TECHNICAL_SENTINELS = new Map([
  ["crates/app/ui/src/lib/mermaid/color.ts", 'const UNSET = "#000001";'],
]);

// Bitmap branding (`public/logo.png`) and project icons supplied by users are content, not app
// styling, so this source guard intentionally does not inspect raster assets. Every authored UI
// pigment belongs in `themes/builtins/*.json`; custom themes arrive as validated data at runtime.
const COLOR_PATTERNS = [
  {
    label: "numeric color literal",
    expression: /(?<!&)#[\da-f]{3,8}\b|\brgba?\(\s*\d|\bhsla?\(\s*\d|\boklch\(\s*\d/giu,
  },
  {
    label: "raw Tailwind palette utility",
    expression:
      /\b(?:bg|text|border|ring|outline|fill|stroke|shadow|from|via|to|divide|decoration|caret|accent)-(?:white|black|slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose)(?:[-/][\w.]+)?\b/gu,
  },
  {
    label: "appearance-specific paint utility",
    expression: /\bdark:(?:bg|text|border|ring|outline|fill|stroke|shadow|from|via|to|divide|decoration|caret|accent)-/gu,
  },
  {
    label: "named CSS pigment",
    expression:
      /\b(?:color|background(?:-color)?|border(?:-\w+)?-color|outline-color|fill|stroke)\s*:\s*(?:white|black|red|green|blue|yellow|orange|purple|pink|gray|grey)\b/giu,
  },
];

// A CSS selector paints nothing: `.\!text-red-500 { color: var(--theme-error) }` overrides an
// upstream utility class rather than authoring a pigment, so only what a rule *declares* is
// inspected. Every prelude — a selector, or an at-rule's condition — is blanked to spaces of the
// same length, which keeps the reported line numbers true. This is a brace/semicolon walk rather
// than a CSS parser: a `}` inside a string would desynchronize it, and none of this tree has one.
function declarationsOnly(source) {
  let inspected = "";
  let start = 0;
  for (let index = 0; index < source.length; index += 1) {
    const delimiter = source[index];
    if (delimiter !== "{" && delimiter !== "}" && delimiter !== ";") continue;
    const segment = source.slice(start, index);
    inspected += delimiter === "{" ? segment.replace(/[^\n]/gu, " ") : segment;
    inspected += delimiter;
    start = index + 1;
  }
  return inspected + source.slice(start);
}

function violationsIn(name, source) {
  const sentinel = TECHNICAL_SENTINELS.get(name);
  let inspected = sentinel ? source.replace(sentinel, "") : source;
  if (extname(name) === ".css") inspected = declarationsOnly(inspected);

  const found = [];
  for (const { label, expression } of COLOR_PATTERNS) {
    expression.lastIndex = 0;
    for (const match of inspected.matchAll(expression)) {
      found.push(`${name}:${lineNumber(inspected, match.index)} ${label}: ${match[0]}`);
    }
  }
  return found;
}

// The guard's own proof, run before the tree is walked so the patterns cannot be loosened into
// silence unnoticed. A pigment authored in any of these forms has to be caught; a class name
// standing in a selector has to pass.
const SELF_CHECK = [
  {
    name: "fixture.css",
    source: ".w .\\!text-red-500 {\n  color: var(--theme-error);\n}\n",
    found: 0,
  },
  { name: "fixture.css", source: ".w {\n  color: red;\n}\n", found: 1 },
  { name: "fixture.css", source: ".w {\n  background: #ff0000;\n}\n", found: 1 },
  { name: "fixture.css", source: ".w {\n  @apply text-red-500;\n}\n", found: 1 },
  { name: "fixture.tsx", source: 'const badge = <b className="text-red-500" />;\n', found: 1 },
];

function selfCheck() {
  for (const { name, source, found } of SELF_CHECK) {
    const violations = violationsIn(name, source);
    if (violations.length === found) continue;
    console.error(
      `Theme color guard self-check failed: expected ${found} violation(s) in\n${source}` +
        `but found ${violations.length}:\n` +
        violations.map((item) => `  ${item}`).join("\n"),
    );
    process.exit(1);
  }
}

function isProductionSource(path) {
  const name = relative(sourceRoot, path);
  return (
    sourceExtensions.has(extname(path)) &&
    !name.includes("/test/") &&
    !name.includes(".test.") &&
    !name.includes(".spec.")
  );
}

async function sourceFiles(directory) {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await sourceFiles(path)));
    else if (isProductionSource(path)) files.push(path);
  }
  return files;
}

function lineNumber(source, offset) {
  return source.slice(0, offset).split("\n").length;
}

selfCheck();

const violations = [];
for (const path of await sourceFiles(sourceRoot)) {
  const name = relative(repository, path);
  violations.push(...violationsIn(name, await readFile(path, "utf8")));
}

if (violations.length > 0) {
  console.error("Theme color ownership violations:\n" + violations.map((item) => `  ${item}`).join("\n"));
  console.error("Move visible pigments into themes/builtins/*.json and consume them through semantic tokens.");
  process.exitCode = 1;
}
