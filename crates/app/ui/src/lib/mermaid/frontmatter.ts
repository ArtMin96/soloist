// Reading and writing a diagram's per-diagram theme override, stored where Mermaid looks for it: a YAML
// frontmatter block at the top of the source.
//
//   ---
//   config:
//     theme: dark
//   ---
//   flowchart TD
//     ...
//
// Pure and source-only — it edits the text the panel autosaves, never touches the renderer. "Follow
// app" is the absence of an override (no frontmatter theme), so the diagram tracks the app's light/dark
// palette; a named theme pins the diagram to one of Mermaid's built-in palettes regardless of app theme.
// The renderer reads this same frontmatter (see the engine) to decide whether to inject the app tokens.

/** The built-in Mermaid themes offered as an override. "base" is app-tokened; the rest are self-contained. */
export const DIAGRAM_THEME_VALUES = ["base", "dark", "forest", "neutral"] as const;
export type DiagramTheme = (typeof DIAGRAM_THEME_VALUES)[number];

/** The label each override reads as in the theme control — one source for the picker. */
export const DIAGRAM_THEME_LABELS: Record<DiagramTheme, string> = {
  base: "Base",
  dark: "Dark",
  forest: "Forest",
  neutral: "Neutral",
};

/** A leading YAML frontmatter block: `---` on its own line, content, a closing `---` line. */
const FRONTMATTER = /^---[ \t]*\r?\n([\s\S]*?)\r?\n---[ \t]*(?:\r?\n|$)/;

/** The `config:` map key at the frontmatter's top level (no indentation). */
const CONFIG_KEY = /^config:[ \t]*$/;

/** A `theme:` entry indented under `config:`, capturing its value. */
const THEME_ENTRY = /^[ \t]+theme:[ \t]*(\S+)[ \t]*$/;

/** True for a token this feature can round-trip (an unknown theme reads as "follow app"). */
function isDiagramTheme(value: string): value is DiagramTheme {
  return (DIAGRAM_THEME_VALUES as readonly string[]).includes(value);
}

interface Split {
  /** The frontmatter's inner YAML lines (without the `---` fences), or null when there is none. */
  yaml: string[] | null;
  /** Everything after the frontmatter block — the diagram body, preserved verbatim. */
  body: string;
}

function split(source: string): Split {
  const match = FRONTMATTER.exec(source);
  if (!match) return { yaml: null, body: source };
  return { yaml: match[1].split(/\r?\n/), body: source.slice(match[0].length) };
}

/** Reassemble a source from its frontmatter lines and body; dropping empty frontmatter loses the fences. */
function join(yaml: string[], body: string): string {
  if (yaml.length === 0) return body;
  return `---\n${yaml.join("\n")}\n---\n${body}`;
}

/** The index range `[start, end)` of the `config:` block's indented entries, or null when absent. */
function configBlock(yaml: string[]): { key: number; start: number; end: number } | null {
  const key = yaml.findIndex((line) => CONFIG_KEY.test(line));
  if (key === -1) return null;
  let end = key + 1;
  // Indented (or blank) lines belong to the block; the first non-indented, non-blank line ends it.
  while (end < yaml.length && (yaml[end].trim() === "" || /^[ \t]/.test(yaml[end]))) end += 1;
  return { key, start: key + 1, end };
}

/** The diagram's theme override, or null when it follows the app theme (or names a theme we don't offer). */
export function readDiagramTheme(source: string): DiagramTheme | null {
  const { yaml } = split(source);
  if (!yaml) return null;
  const block = configBlock(yaml);
  if (!block) return null;
  for (let i = block.start; i < block.end; i += 1) {
    const found = THEME_ENTRY.exec(yaml[i]);
    if (found) return isDiagramTheme(found[1]) ? found[1] : null;
  }
  return null;
}

/**
 * Return `source` with its theme override set to `theme`, or removed when `theme` is null ("follow
 * app"). Any other frontmatter (a `title:`, other `config:` entries) is preserved; removing the last
 * override strips an emptied `config:` map and an emptied frontmatter block so the round-trip is clean.
 */
export function setDiagramTheme(source: string, theme: DiagramTheme | null): string {
  const { yaml, body } = split(source);

  if (theme === null) {
    if (!yaml) return source;
    const block = configBlock(yaml);
    if (!block) return source;
    const themeAt = yaml.slice(block.start, block.end).findIndex((line) => THEME_ENTRY.test(line));
    if (themeAt === -1) return source;
    const next = [...yaml];
    next.splice(block.start + themeAt, 1);
    // If that emptied the `config:` map, drop the now-childless key as well.
    if (block.end - block.start === 1) next.splice(block.key, 1);
    return join(next, body);
  }

  const entry = `  theme: ${theme}`;
  if (!yaml) return `---\nconfig:\n${entry}\n---\n${body}`;

  const block = configBlock(yaml);
  const next = [...yaml];
  if (!block) {
    next.push("config:", entry);
    return join(next, body);
  }
  const themeAt = yaml.slice(block.start, block.end).findIndex((line) => THEME_ENTRY.test(line));
  if (themeAt === -1) next.splice(block.start, 0, entry);
  else next[block.start + themeAt] = entry;
  return join(next, body);
}
