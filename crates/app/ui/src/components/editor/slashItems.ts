import type { Editor, Range } from "@tiptap/react";

// One entry in the "/" command menu: a label, a one-line hint, extra match keywords, and the edit it
// runs (which first deletes the typed "/query" so no trigger text is left behind). Kept as plain data
// plus a pure filter so the menu's matching is unit-testable without mounting an editor.
export interface SlashItem {
  title: string;
  hint: string;
  keywords: string[];
  run: (editor: Editor, range: Range) => void;
}

// The block structures a scratchpad body can hold — the same set the formatting toolbar exposes, so
// "/" and the toolbar never drift. Ordered by how often a note reaches for them.
export const SLASH_ITEMS: SlashItem[] = [
  {
    title: "Heading 1",
    hint: "Large section heading",
    keywords: ["h1", "title", "heading"],
    run: (editor, range) =>
      editor.chain().focus().deleteRange(range).toggleHeading({ level: 1 }).run(),
  },
  {
    title: "Heading 2",
    hint: "Medium section heading",
    keywords: ["h2", "subheading", "heading"],
    run: (editor, range) =>
      editor.chain().focus().deleteRange(range).toggleHeading({ level: 2 }).run(),
  },
  {
    title: "Heading 3",
    hint: "Small section heading",
    keywords: ["h3", "heading"],
    run: (editor, range) =>
      editor.chain().focus().deleteRange(range).toggleHeading({ level: 3 }).run(),
  },
  {
    title: "Bullet list",
    hint: "An unordered list",
    keywords: ["ul", "bullet", "unordered", "list"],
    run: (editor, range) => editor.chain().focus().deleteRange(range).toggleBulletList().run(),
  },
  {
    title: "Numbered list",
    hint: "An ordered list",
    keywords: ["ol", "ordered", "number", "list"],
    run: (editor, range) => editor.chain().focus().deleteRange(range).toggleOrderedList().run(),
  },
  {
    title: "To-do list",
    hint: "A checklist of tasks",
    keywords: ["todo", "task", "checkbox", "check", "list"],
    run: (editor, range) => editor.chain().focus().deleteRange(range).toggleTaskList().run(),
  },
  {
    title: "Quote",
    hint: "A block quotation",
    keywords: ["quote", "blockquote", "citation"],
    run: (editor, range) => editor.chain().focus().deleteRange(range).toggleBlockquote().run(),
  },
  {
    title: "Code block",
    hint: "A fenced code block",
    keywords: ["code", "fence", "pre", "snippet"],
    run: (editor, range) => editor.chain().focus().deleteRange(range).toggleCodeBlock().run(),
  },
  {
    title: "Divider",
    hint: "A horizontal rule",
    keywords: ["divider", "rule", "hr", "separator", "line"],
    run: (editor, range) => editor.chain().focus().deleteRange(range).setHorizontalRule().run(),
  },
];

/**
 * The menu items matching `query` (the text typed after "/"), case-insensitively across each item's
 * title and keywords. An empty query returns the full list — the menu opens showing everything.
 */
export function filterSlashItems(query: string): SlashItem[] {
  const q = query.trim().toLowerCase();
  if (q === "") return SLASH_ITEMS;
  return SLASH_ITEMS.filter(
    (item) =>
      item.title.toLowerCase().includes(q) || item.keywords.some((keyword) => keyword.includes(q)),
  );
}
