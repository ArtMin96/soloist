// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { Editor } from "@tiptap/react";
import { buildEditorExtensions } from "./editorExtensions";

// Loads Markdown into a real editor and reads it back out — the exact path a scratchpad body travels
// on every open and save. The slash extension is off because it plays no part in serialization and
// its floating popup has nothing to attach to in jsdom.
function roundTrip(markdown: string): string {
  const editor = new Editor({
    extensions: buildEditorExtensions({ placeholder: "", slash: false }),
  });
  editor.commands.setContent(markdown, { contentType: "markdown", emitUpdate: false });
  const out = editor.getMarkdown();
  editor.destroy();
  return out;
}

const CASES: [label: string, markdown: string][] = [
  ["headings", "# Title\n\n## Section\n\n### Detail"],
  ["bullet list", "- one\n- two\n- three"],
  ["ordered list", "1. one\n2. two"],
  ["task list", "- [ ] pending\n- [x] done"],
  ["code block", "```ts\nconst x = 1;\n```"],
  ["inline marks", "A **bold** and *italic* and `code` word."],
  ["link", "See [the docs](https://example.com/page)."],
  ["blockquote", "> a quotation"],
  ["table", "| A | B |\n| --- | --- |\n| 1 | 2 |"],
];

describe("markdown round-trip", () => {
  it.each(CASES)("keeps %s stable across a load→save cycle", (_label, markdown) => {
    // The serializer may normalize outer whitespace and column padding on the first pass; a second
    // pass must then be a fixed point, so autosave never rewrites a note it did not actually change.
    const once = roundTrip(markdown);
    expect(roundTrip(once)).toBe(once);
  });

  it("preserves each construct's content, not just its shape", () => {
    expect(roundTrip("# Title")).toContain("# Title");
    expect(roundTrip("- one\n- two")).toContain("- one");
    expect(roundTrip("1. one\n2. two")).toContain("1. one");
    expect(roundTrip("- [ ] pending\n- [x] done")).toContain("- [ ] pending");
    expect(roundTrip("- [x] done")).toContain("[x] done");
    expect(roundTrip("```ts\nconst x = 1;\n```")).toContain("const x = 1;");
    expect(roundTrip("[docs](https://example.com)")).toContain("[docs](https://example.com)");
    const table = roundTrip("| A | B |\n| --- | --- |\n| 1 | 2 |");
    expect(table).toContain("| A");
    expect(table).toContain("| 1");
  });

  // The serializer HTML-encodes text on the way out to Markdown, which would persist `&amp;` for a
  // typed `&`. The encoding is stable, so the fixed-point check above cannot see it — only reading
  // the characters back does.
  it("keeps HTML-significant characters literal rather than encoding them as entities", () => {
    expect(roundTrip("Current State & Context")).toContain("State & Context");
    expect(roundTrip("Assumption -> Verification")).toContain("-> Verification");
    expect(roundTrip("A < B and C > D")).toContain("A < B and C > D");
    expect(roundTrip('Tom & Jerry say "hi"')).toContain("Tom & Jerry");
  });

  // Inside a code fence the serializer already passes text through untouched, so the entity
  // correction must not reach in and rewrite a literal entity a user actually typed.
  it("leaves a literal entity inside a code block alone", () => {
    expect(roundTrip("```html\n<p>&amp;</p>\n```")).toContain("<p>&amp;</p>");
  });

  it("treats a blank body as valid and empty", () => {
    expect(roundTrip("").trim()).toBe("");
  });
});
