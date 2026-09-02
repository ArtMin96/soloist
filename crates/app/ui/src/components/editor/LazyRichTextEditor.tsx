import { Suspense, lazy, type ComponentProps, type ReactNode } from "react";
import type RichTextEditor from "./RichTextEditor";

// The rich editor is loaded lazily so the whole @tiptap dependency graph lands in its own chunk and
// never touches the initial bundle. Every consumer — the scratchpad body, the todo body, later the
// template editor — mounts it through this one boundary, so they share the single lazy chunk rather
// than each declaring their own dynamic import. Opening a document is what pulls the chunk in.
const RichTextEditorLazy = lazy(() => import("./RichTextEditor"));

/**
 * What stands in while the chunk loads. The default is a document-sized frame, which is right for a
 * surface that owns a pane and wrong for a body rendered inline among others: a thread of comments
 * would otherwise flash one bordered box per comment and then jump as the real heights arrive. Such
 * a caller passes `null`.
 */
type LazyRichTextEditorProps = ComponentProps<typeof RichTextEditor> & { fallback?: ReactNode };

const DEFAULT_FALLBACK = <div className="min-h-0 flex-1 rounded-md border bg-background" />;

export function LazyRichTextEditor({
  fallback = DEFAULT_FALLBACK,
  ...props
}: LazyRichTextEditorProps) {
  return (
    <Suspense fallback={fallback}>
      <RichTextEditorLazy {...props} />
    </Suspense>
  );
}
