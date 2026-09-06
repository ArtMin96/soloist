import { useDeferredValue, useState } from "react";
import { LoadingStandIn } from "@/components/common/LoadingStandIn";
import { LazyRichTextEditor } from "@/components/editor/LazyRichTextEditor";
import { MarkdownSkeleton } from "@/components/editor/MarkdownSkeleton";
import { cn } from "@/lib/utils";
import { usePanelSettled } from "@/store/panelSettledContext";

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
 * That renderer is expensive to start — hundreds of milliseconds of main thread for a long
 * document — and a click that opens one mounts one of these per body and comment. So it is left out
 * of the frame the click commits, and out of the movement that brings the panel it lives in on
 * screen: it is built once that panel reports it has arrived, so the slide runs at frame rate and
 * the stand-in is on screen long enough to be seen rather than being replaced by a frozen window.
 * A body with no panel above it — a comment thread, a template preview — has nothing to wait for
 * and builds on the pass after it mounts. Until the editor reports its content seeded, the body
 * holds a single stand-in and the editor builds itself invisibly underneath: one continuous wait
 * rather than a blank gap between the chunk landing and the text appearing. Hidden rather than
 * unmounted, so a block that measures itself as it renders — a diagram — already has the width it
 * will be drawn at.
 */
export function MarkdownView({ markdown, ariaLabel, announce = true }: MarkdownViewProps) {
  const panelSettled = usePanelSettled();
  const pastFirstPass = useDeferredValue(true, false);
  const [ready, setReady] = useState(false);
  // Prose already seeded stays mounted through a panel leaving, since that panel carries what the
  // reader was looking at for the length of the movement. Prose still building is abandoned
  // instead: there is nothing on screen to keep, and finishing it would cost the movement frames.
  const build = ready || (panelSettled && pastFirstPass);

  return (
    <div className="relative">
      {!ready && (
        <LoadingStandIn label={announce ? (ariaLabel ?? UNNAMED_LABEL) : undefined}>
          <MarkdownSkeleton markdown={markdown} />
        </LoadingStandIn>
      )}
      {build && (
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
