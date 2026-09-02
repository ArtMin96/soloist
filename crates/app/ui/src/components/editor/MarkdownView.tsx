import type { ReactNode } from "react";
import { LazyRichTextEditor } from "@/components/editor/LazyRichTextEditor";

interface MarkdownViewProps {
  /** The Markdown to render. Read once — the view is remounted with a fresh key to show new text. */
  markdown: string;
  /** The accessible name for the rendered region. */
  ariaLabel?: string;
  /** What stands in while the renderer's chunk loads; defaults to a document-sized frame. */
  fallback?: ReactNode;
}

// Markdown rendered for reading. It is the same editor the authoring surfaces mount, held read-only
// with its chrome off, so a document reads identically wherever it appears — one renderer, one
// Markdown dialect, and one lazily-loaded chunk rather than a second parser for display.
export function MarkdownView({ markdown, ariaLabel, fallback }: MarkdownViewProps) {
  return (
    <LazyRichTextEditor
      fallback={fallback}
      initialMarkdown={markdown}
      ariaLabel={ariaLabel}
      editable={false}
      toolbar={false}
      slash={false}
      // Read-only content emits no edits; the editor still requires somewhere to report them.
      onChange={() => {}}
    />
  );
}
