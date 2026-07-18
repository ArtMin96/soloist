import { Suspense, lazy, type ComponentProps } from "react";
import type RichTextEditor from "./RichTextEditor";

// The rich editor is loaded lazily so the whole @tiptap dependency graph lands in its own chunk and
// never touches the initial bundle. Every consumer — the scratchpad body, the todo body, later the
// template editor — mounts it through this one boundary, so they share the single lazy chunk rather
// than each declaring their own dynamic import. Opening a document is what pulls the chunk in.
const RichTextEditorLazy = lazy(() => import("./RichTextEditor"));

export function LazyRichTextEditor(props: ComponentProps<typeof RichTextEditor>) {
  return (
    <Suspense fallback={<div className="min-h-0 flex-1 rounded-md border bg-background" />}>
      <RichTextEditorLazy {...props} />
    </Suspense>
  );
}
