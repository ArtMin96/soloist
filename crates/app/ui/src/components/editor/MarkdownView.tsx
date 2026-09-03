import { useDeferredValue, useState } from "react";
import { LoadingStandIn } from "@/components/common/LoadingStandIn";
import { LazyRichTextEditor } from "@/components/editor/LazyRichTextEditor";
import { MarkdownSkeleton } from "@/components/editor/MarkdownSkeleton";
import { cn } from "@/lib/utils";

interface MarkdownViewProps {
  /** The Markdown to render. Read once — the view is remounted with a fresh key to show new text. */
  markdown: string;
  /** The accessible name for the rendered region, and the stand-in's label while it renders. */
  ariaLabel?: string;
  /** When false the stand-in announces nothing — for a body under an already-readable author line. */
  announce?: boolean;
}

/** What the wait is called when the caller named no region: prose with no title of its own. */
const UNNAMED_LABEL = "text";

/**
 * Markdown rendered for reading. It is the same editor the authoring surfaces mount, held read-only
 * with its chrome off, so a document reads identically wherever it appears — one renderer, one
 * Markdown dialect, and one lazily-loaded chunk rather than a second parser for display.
 *
 * That renderer is expensive to start, and a click that opens a document mounts one of these per
 * body and comment. So the editor is left out of the frame the click commits and mounted on the
 * pass after it, which lets the panel it lives in move at once instead of waiting on prose. Until
 * the editor reports its content seeded, the body holds a single stand-in and the editor builds
 * itself invisibly underneath: one continuous wait rather than a blank gap between the chunk
 * landing and the text appearing. Hidden rather than unmounted, so a block that measures itself as
 * it renders — a diagram — already has the width it will be drawn at.
 */
export function MarkdownView({ markdown, ariaLabel, announce = true }: MarkdownViewProps) {
  const settled = useDeferredValue(true, false);
  const [ready, setReady] = useState(false);

  return (
    <div className="relative">
      {!ready && (
        <LoadingStandIn label={announce ? (ariaLabel ?? UNNAMED_LABEL) : undefined}>
          <MarkdownSkeleton markdown={markdown} />
        </LoadingStandIn>
      )}
      {settled && (
        <div className={cn(!ready && "invisible absolute inset-x-0 top-0")}>
          <LazyRichTextEditor
            fallback={null}
            initialMarkdown={markdown}
            ariaLabel={ariaLabel}
            editable={false}
            toolbar={false}
            slash={false}
            // Read-only content emits no edits; the editor still requires somewhere to report them.
            onChange={() => {}}
            onReady={() => setReady(true)}
          />
        </div>
      )}
    </div>
  );
}
