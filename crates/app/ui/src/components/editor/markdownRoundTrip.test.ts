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

// Serializes text the user typed, rather than text parsed from Markdown — the other half of the
// cycle, and the half that decides what actually lands in the store.
function save(typed: string): string {
  const editor = new Editor({
    extensions: buildEditorExtensions({ placeholder: "", slash: false }),
  });
  editor.commands.setContent({
    type: "doc",
    content: [{ type: "paragraph", content: [{ type: "text", text: typed }] }],
  });
  const out = editor.getMarkdown();
  editor.destroy();
  return out;
}

// The text a reader ends up with after stored Markdown is loaded back — what the typed characters
// have to survive as, whatever form the store holds them in.
function loadText(markdown: string): string {
  const editor = new Editor({
    extensions: buildEditorExtensions({ placeholder: "", slash: false }),
  });
  editor.commands.setContent(markdown, { contentType: "markdown", emitUpdate: false });
  const out = editor.getText();
  editor.destroy();
  return out;
}

const CASES: [label: string, markdown: string][] = [
  ["headings", "# Title\n\n## Section\n\n### Detail"],
  ["bullet list", "- one\n- two\n- three"],
  ["ordered list", "1. one\n2. two"],
  ["task list", "- [ ] pending\n- [x] done"],
  ["code block", "```ts\nconst x = 1;\n```"],
  ["mermaid diagram", "```mermaid\nflowchart TD\n  A --> B\n```"],
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

  // The serializer HTML-encodes text on the way out, which would persist `&amp;` for a typed `&`.
  // The encoding is stable, so the fixed-point check above cannot see it — only reading the stored
  // characters back does, which is what an agent gets over MCP.
  it.each([
    ["ampersand", "Current State & Context"],
    ["arrow", "Assumption -> Verification needed"],
    ["comparison", "A < B and C > D"],
    ["quotes", 'Tom & Jerry say "hi"'],
  ])("stores %s as literal characters, not entities", (_label, typed) => {
    expect(save(typed)).toBe(typed);
  });

  // Storing these literally would hand the next load real markup to parse, destroying the typed
  // text, so the encoding stays. Reading them back is what has to be right, not the stored bytes.
  it.each([
    ["an HTML tag", "Use <div> tags here"],
    ["a tag pair", "A <b>bold</b> B"],
    ["a literal entity", "Ampersand is &amp; in HTML"],
    ["a leading angle bracket", "> not a quotation"],
  ])("survives a save and reload with %s intact", (_label, typed) => {
    expect(loadText(save(typed))).toBe(typed);
  });

  // Inside a code fence the serializer passes text through untouched, so the correction must not
  // reach in and rewrite an entity the user actually typed.
  it("leaves a literal entity inside a code block alone", () => {
    expect(roundTrip("```html\n<p>&amp;</p>\n```")).toContain("<p>&amp;</p>");
  });

  // A ```mermaid fence must survive the editor's NodeView as an ordinary language-tagged code block:
  // the language tag is kept and the diagram source — arrows, ampersands, angle brackets — is stored
  // verbatim (the entity-correction layer treats code text as literal). A NodeView that swapped the
  // codeBlock node for an atom would drop the fence and fail this.
  it("round-trips a mermaid fence, keeping its language tag and diagram source verbatim", () => {
    const source = "```mermaid\nflowchart LR\n  A -->|yes & no| B\n  C[a < b]\n```";
    const out = roundTrip(source);
    expect(out).toContain("```mermaid");
    expect(out).toContain("A -->|yes & no| B");
    expect(out).toContain("C[a < b]");
    // And a second pass is a fixed point, so autosave never rewrites an unchanged diagram.
    expect(roundTrip(out)).toBe(out);
  });

  it("treats a blank body as valid and empty", () => {
    expect(roundTrip("").trim()).toBe("");
  });
});
