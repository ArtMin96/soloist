import type { Extensions } from "@tiptap/react";
import { StarterKit } from "@tiptap/starter-kit";
import { Markdown } from "@tiptap/markdown";
import { Placeholder } from "@tiptap/extensions";
import { TaskList } from "@tiptap/extension-task-list";
import { TaskItem } from "@tiptap/extension-task-item";
import { TableKit } from "@tiptap/extension-table";
import { applyMarkdownEntityCorrection } from "./markdownEntities";
import { slashCommand } from "./extensions/slashCommand";
import { searchExtension } from "./search/searchExtension";

// The Markdown serializer is a module-level singleton, so its correction is applied once here rather
// than from inside the builder, which would re-run it on every editor mounted.
applyMarkdownEntityCorrection();

// The heading depth a note reaches for — three levels are enough structure for a scratchpad; more
// would invite an over-deep outline. Named so the toolbar, the outline, and the parser agree.
export const EDITOR_HEADING_LEVELS = [1, 2, 3] as const;

export interface EditorExtensionOptions {
  /** The empty-state prompt shown in the first empty block. */
  placeholder: string;
  /** Enable the "/" command menu. Off for compact variants (e.g. a single-line comment composer). */
  slash?: boolean;
}

/**
 * Assembles the editor's extension set once, so every consumer (scratchpad body, todo body, template
 * body, comment composer) shares the same Markdown-backed schema and no surface drifts. StarterKit
 * carries the marks, lists, links, and undo/redo; Markdown makes content flow in and out as GitHub-
 * flavored Markdown; TaskList/TaskItem add checkboxes; Placeholder prompts an empty note.
 */
export function buildEditorExtensions(options: EditorExtensionOptions): Extensions {
  const extensions: Extensions = [
    StarterKit.configure({
      heading: { levels: [...EDITOR_HEADING_LEVELS] },
      // A link is styled and typed but only followed deliberately — clicking one inside an editor
      // should place the caret, not navigate away.
      link: { openOnClick: false },
    }),
    Markdown.configure({ markedOptions: { gfm: true } }),
    TaskList,
    TaskItem.configure({ nested: true }),
    // GitHub-flavored tables — without a table node the Markdown serializer silently drops a note's
    // tables, so the body must carry the node to round-trip them.
    TableKit,
    Placeholder.configure({ placeholder: options.placeholder }),
    // Always present but idle until a query arrives — it powers the in-note find bar (Ctrl/Cmd+F).
    searchExtension,
  ];
  if (options.slash !== false) extensions.push(slashCommand);
  return extensions;
}
