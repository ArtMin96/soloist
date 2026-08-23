#!/usr/bin/env node
// Reports where the codebase repeats itself — cloned files first, then repeated blocks — across
// both halves of the tree: the Rust workspace and the TypeScript frontend and e2e sources.
//
// Reporting only. It always exits 0, it is not in `just lint`, and it is in no CI workflow. A
// duplication gate fires on code that is legitimately similar but deliberately separate, and a
// gate that cries wolf gets switched off; a switched-off gate is worse than none. Every other
// discipline rule in CLAUDE.md §15 has a signal (check-core-deps.sh, check-core-cycles.sh gate;
// check-file-size.sh warns), while "DRY — one place to change" had no signal at all, which is how
// it decayed unnoticed. This is that signal: visible on demand, blocking nothing.
//
// Whole-file similarity is measured after masking the one noun that distinguishes a candidate
// pair's file names (ScratchpadList/DiagramList -> DOC-List), because a clone that was renamed as
// it was pasted is invisible to symbol-similarity search: that finds duplicated functions, not
// duplicated files.
//
// KNOWN DELIBERATE SEPARATIONS — these are considered decisions, not debt. Do not "fix" them:
//   - the read-side latest-request guard vs the write-side single-flight queue: opposite
//     disciplines, and unifying them reintroduces the bug each one exists to prevent;
//   - `ScratchpadSummary` vs `DiagramSummary` in domain.ts: they mirror two distinct Rust core
//     types, so collapsing them would fake a single source of truth that does not exist;
//   - `coordination/diagram_repo.rs` vs `coordination/scratchpad_repo.rs`: the aggregates differ;
//   - Rust per-type trait impls in general: structurally alike by necessity, not by copy-paste.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repository = resolve(fileURLToPath(new URL("..", import.meta.url)));

// A file pair is reported when this share of its combined lines matches.
const MIN_PAIR_SIMILARITY = 0.6;
// Files shorter than this carry too little shape for a similarity number to mean anything.
const MIN_FILE_LINES = 10;
// A candidate pair needs this many verbatim lines in common before the exact diff is worth running.
const MIN_SHARED_LINES = 8;
// A line present in more than this many files is boilerplate (imports, derives), not a clone.
const BOILERPLATE_FILE_COUNT = 25;
// Exact diffs are quadratic; the widest pairs are left out rather than allowed to stall the run.
const MAX_DIFF_CELLS = 4_000_000;
// Consecutive substantial lines that must repeat verbatim to count as a cloned block.
const BLOCK_WINDOW = 18;
// Shared filename context (prefix plus suffix) a pair needs before its differing noun is masked.
const MIN_NAME_CONTEXT = 3;
// Shorter differing nouns ("a", "id") mask far too much text to be safe.
const MIN_NOUN_LENGTH = 3;
const MAX_REPORTED_PAIRS = 25;
const MAX_REPORTED_BLOCKS = 12;
const MASKED_NOUN = "DOC";

const BLOCK_COMMENT = /\/\*[\s\S]*?\*\//g;
const LINE_COMMENT = /\/\/.*$/gm;
const WHITESPACE_RUN = /\s+/g;

/**
 * Rust and TypeScript sources, repository-relative. Untracked files count too — a clone is worth
 * seeing the moment it is pasted, not once it is committed — while .gitignore still keeps target/
 * and node_modules/ out.
 */
function sourcePaths() {
  const listed = execFileSync(
    "git",
    ["ls-files", "--cached", "--others", "--exclude-standard", "*.rs", "*.ts", "*.tsx"],
    { cwd: repository, encoding: "utf8", maxBuffer: 1 << 24 },
  );
  return listed.split("\n").filter(Boolean);
}

/** Comment-free, whitespace-normalized, non-empty lines — the shape of the code, not its layout. */
function codeLines(text) {
  return text
    .replace(BLOCK_COMMENT, " ")
    .replace(LINE_COMMENT, "")
    .split("\n")
    .map((line) => line.replace(WHITESPACE_RUN, " ").trim());
}

/** Punctuation-only lines (`}`, `);`, `} else {`) repeat everywhere and carry no clone signal. */
function isSubstantial(line) {
  return line.length >= 6 && /[A-Za-z]/.test(line);
}

function load(path) {
  const raw = readFileSync(resolve(repository, path), "utf8");
  const lines = [];
  const substantial = [];
  codeLines(raw).forEach((text, index) => {
    if (text === "") return;
    lines.push(text);
    if (isSubstantial(text)) substantial.push({ text, line: index + 1 });
  });
  return { path, lines, substantial, hashes: blockHashes(substantial) };
}

function blockHashes(substantial) {
  const hashes = [];
  for (let start = 0; start + BLOCK_WINDOW <= substantial.length; start += 1) {
    const window = substantial.slice(start, start + BLOCK_WINDOW).map((entry) => entry.text);
    hashes.push(createHash("sha1").update(window.join("\n")).digest("hex").slice(0, 16));
  }
  return hashes;
}

/**
 * Lines the two arrays have in common regardless of order — an upper bound on the subsequence
 * count below, computed in linear time so hopeless pairs never reach the quadratic diff.
 */
function sharedLineCount(left, right) {
  const remaining = new Map();
  for (const line of left) remaining.set(line, (remaining.get(line) ?? 0) + 1);
  let shared = 0;
  for (const line of right) {
    const available = remaining.get(line) ?? 0;
    if (available === 0) continue;
    remaining.set(line, available - 1);
    shared += 1;
  }
  return shared;
}

/** Length of the longest common subsequence of two line arrays, over two rolling rows. */
function commonLineCount(left, right) {
  let previous = new Int32Array(right.length + 1);
  let current = new Int32Array(right.length + 1);
  for (let i = 0; i < left.length; i += 1) {
    const leftLine = left[i];
    for (let j = 0; j < right.length; j += 1) {
      current[j + 1] =
        leftLine === right[j] ? previous[j] + 1 : Math.max(previous[j + 1], current[j]);
    }
    [previous, current] = [current, previous];
  }
  return previous[right.length];
}

/** Whether a name can be cut at this offset without splitting a word. */
function isWordBoundary(name, offset) {
  if (offset === 0 || offset === name.length) return true;
  const before = name[offset - 1];
  const at = name[offset];
  if (/[_\-.]/.test(before) || /[_\-.]/.test(at)) return true;
  return /[A-Z]/.test(at) && /[a-z0-9]/.test(before);
}

/**
 * The one noun that distinguishes two file names, or null when the names are unrelated.
 * `ScratchpadList.tsx` / `DiagramList.tsx` -> `Scratchpad` / `Diagram`.
 *
 * Both cuts are pulled back to a word boundary, so the nouns are whole words. Cutting mid-word
 * pairs anything: `sampler` and `scanner` share an `s`, an `er`, and nothing else that matters.
 */
function differingNouns(left, right) {
  const a = left.replace(/\.[^./]+$/, "");
  const b = right.replace(/\.[^./]+$/, "");
  let prefix = 0;
  while (prefix < a.length && prefix < b.length && a[prefix] === b[prefix]) prefix += 1;
  let suffix = 0;
  while (
    suffix < a.length - prefix &&
    suffix < b.length - prefix &&
    a[a.length - 1 - suffix] === b[b.length - 1 - suffix]
  ) {
    suffix += 1;
  }
  while (prefix > 0 && !(isWordBoundary(a, prefix) && isWordBoundary(b, prefix))) prefix -= 1;
  while (
    suffix > 0 &&
    !(isWordBoundary(a, a.length - suffix) && isWordBoundary(b, b.length - suffix))
  ) {
    suffix -= 1;
  }
  if (prefix + suffix < MIN_NAME_CONTEXT) return null;
  const nouns = [a.slice(prefix, a.length - suffix), b.slice(prefix, b.length - suffix)];
  const usable = nouns.every((noun) => noun.length >= MIN_NOUN_LENGTH && /^[A-Za-z]+$/.test(noun));
  return usable ? nouns : null;
}

function mask(lines, noun) {
  const pattern = new RegExp(`${noun}s?`, "gi");
  return lines.map((line) => line.replace(pattern, MASKED_NOUN));
}

function basename(path) {
  return path.slice(path.lastIndexOf("/") + 1);
}

function extension(path) {
  return path.slice(path.lastIndexOf("."));
}

/** Pairs sharing enough verbatim lines to be worth diffing, plus every same-shape name pair. */
function candidatePairs(files) {
  const owners = new Map();
  files.forEach((file, index) => {
    for (const text of new Set(file.substantial.map((entry) => entry.text))) {
      const seen = owners.get(text);
      if (seen) seen.push(index);
      else owners.set(text, [index]);
    }
  });

  const shared = new Map();
  for (const indices of owners.values()) {
    if (indices.length < 2 || indices.length > BOILERPLATE_FILE_COUNT) continue;
    for (let i = 0; i < indices.length; i += 1) {
      for (let j = i + 1; j < indices.length; j += 1) {
        const key = `${indices[i]}:${indices[j]}`;
        shared.set(key, (shared.get(key) ?? 0) + 1);
      }
    }
  }

  const candidates = new Map();
  const add = (left, right, nouns) => {
    const key = `${left}:${right}`;
    const smaller = Math.min(files[left].lines.length, files[right].lines.length);
    if (smaller < MIN_FILE_LINES) return;
    candidates.set(key, { left, right, nouns: nouns ?? candidates.get(key)?.nouns ?? null });
  };

  for (const [key, count] of shared) {
    if (count < MIN_SHARED_LINES) continue;
    const [left, right] = key.split(":").map(Number);
    add(left, right);
  }
  for (let i = 0; i < files.length; i += 1) {
    for (let j = i + 1; j < files.length; j += 1) {
      if (extension(files[i].path) !== extension(files[j].path)) continue;
      const nouns = differingNouns(basename(files[i].path), basename(files[j].path));
      if (nouns) add(i, j, nouns);
    }
  }
  return [...candidates.values()];
}

function clonedFiles(files) {
  const scored = [];
  for (const { left, right, nouns } of candidatePairs(files)) {
    const a = files[left];
    const b = files[right];
    const total = a.lines.length + b.lines.length;
    if (a.lines.length * b.lines.length > MAX_DIFF_CELLS) continue;

    // Order-insensitive counts first: the exact diff can only score lower, so a pair that cannot
    // reach the threshold even at its upper bound is dropped without paying for the diff.
    const plainBound = sharedLineCount(a.lines, b.lines);
    const maskedA = nouns ? mask(a.lines, nouns[0]) : a.lines;
    const maskedB = nouns ? mask(b.lines, nouns[1]) : b.lines;
    const maskedBound = nouns ? sharedLineCount(maskedA, maskedB) : plainBound;
    if ((2 * Math.max(plainBound, maskedBound)) / total < MIN_PAIR_SIMILARITY) continue;

    const plain = commonLineCount(a.lines, b.lines);
    const masked = nouns ? commonLineCount(maskedA, maskedB) : plain;
    const common = Math.max(plain, masked);
    const similarity = (2 * common) / total;
    if (similarity < MIN_PAIR_SIMILARITY) continue;
    scored.push({
      paths: [a.path, b.path],
      similarity,
      differing: total - 2 * common,
      total,
      nouns: masked > plain ? nouns : null,
    });
  }
  return scored.sort((x, y) => y.similarity * y.total - x.similarity * x.total);
}

function clonedBlocks(files, skipPairs) {
  const index = new Map();
  files.forEach((file, fileIndex) => {
    file.hashes.forEach((hash, start) => {
      const seen = index.get(hash);
      if (seen) seen.push([fileIndex, start]);
      else index.set(hash, [[fileIndex, start]]);
    });
  });

  const covered = files.map((file) => new Uint8Array(file.substantial.length));
  const blocks = [];
  files.forEach((file, fileIndex) => {
    file.hashes.forEach((hash, start) => {
      if (covered[fileIndex][start]) return;
      const partner = (index.get(hash) ?? []).find(
        ([otherFile, otherStart]) =>
          !covered[otherFile][otherStart] &&
          !(otherFile === fileIndex && Math.abs(otherStart - start) < BLOCK_WINDOW) &&
          !(otherFile === fileIndex && otherStart === start) &&
          !skipPairs.has(pairKey(file.path, files[otherFile].path)),
      );
      if (!partner) return;
      const [otherFile, otherStart] = partner;
      const other = files[otherFile];
      let length = BLOCK_WINDOW;
      while (
        start + length < file.substantial.length &&
        otherStart + length < other.substantial.length &&
        file.substantial[start + length].text === other.substantial[otherStart + length].text
      ) {
        length += 1;
      }
      for (let offset = 0; offset < length; offset += 1) {
        covered[fileIndex][start + offset] = 1;
        covered[otherFile][otherStart + offset] = 1;
      }
      blocks.push({
        length,
        locations: [span(file, start, length), span(other, otherStart, length)],
      });
    });
  });
  return blocks.sort((x, y) => y.length - x.length);
}

function span(file, start, length) {
  const first = file.substantial[start].line;
  const last = file.substantial[start + length - 1].line;
  return `${file.path}:${first}-${last}`;
}

function pairKey(left, right) {
  return left < right ? `${left}|${right}` : `${right}|${left}`;
}

function report() {
  const files = sourcePaths().map(load);
  const pairs = clonedFiles(files);
  const reported = new Set(pairs.map((pair) => pairKey(pair.paths[0], pair.paths[1])));
  const blocks = clonedBlocks(files, reported);

  console.log(`duplication report — ${files.length} Rust and TypeScript sources\n`);

  const percent = Math.round(MIN_PAIR_SIMILARITY * 100);
  console.log(`== Cloned files (>= ${percent}% of lines shared, worst first) ==`);
  if (pairs.length === 0) console.log("  (none)");
  for (const pair of pairs.slice(0, MAX_REPORTED_PAIRS)) {
    const note = pair.nouns ? `  [${pair.nouns[0]} <-> ${pair.nouns[1]} masked]` : "";
    console.log(
      `  ${String(Math.round(pair.similarity * 100)).padStart(3)}%  ` +
        `${String(pair.differing).padStart(4)} differing of ${pair.total} lines${note}`,
    );
    for (const path of pair.paths) console.log(`         ${path}`);
  }
  if (pairs.length > MAX_REPORTED_PAIRS) {
    console.log(`  ... and ${pairs.length - MAX_REPORTED_PAIRS} more above the threshold`);
  }

  console.log(`\n== Cloned blocks (>= ${BLOCK_WINDOW} lines repeated verbatim elsewhere) ==`);
  if (blocks.length === 0) console.log("  (none)");
  for (const block of blocks.slice(0, MAX_REPORTED_BLOCKS)) {
    console.log(`  ${String(block.length).padStart(4)} lines`);
    for (const location of block.locations) console.log(`         ${location}`);
  }
  if (blocks.length > MAX_REPORTED_BLOCKS) {
    console.log(`  ... and ${blocks.length - MAX_REPORTED_BLOCKS} more`);
  }

  console.log("\n  Reporting only — nothing here fails a build. Some pairs are deliberate;");
  console.log("  scripts/report-duplication.mjs lists the separations that must not be merged.");
}

try {
  report();
} catch (error) {
  console.log(`duplication report unavailable: ${error.message}`);
}
process.exitCode = 0;
