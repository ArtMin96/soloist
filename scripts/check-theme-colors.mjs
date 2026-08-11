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

const violations = [];
for (const path of await sourceFiles(sourceRoot)) {
  const name = relative(repository, path);
  let source = await readFile(path, "utf8");
  const sentinel = TECHNICAL_SENTINELS.get(name);
  if (sentinel) source = source.replace(sentinel, "");

  for (const { label, expression } of COLOR_PATTERNS) {
    expression.lastIndex = 0;
    for (const match of source.matchAll(expression)) {
      violations.push(`${name}:${lineNumber(source, match.index)} ${label}: ${match[0]}`);
    }
  }
}

if (violations.length > 0) {
  console.error("Theme color ownership violations:\n" + violations.map((item) => `  ${item}`).join("\n"));
  console.error("Move visible pigments into themes/builtins/*.json and consume them through semantic tokens.");
  process.exitCode = 1;
}
