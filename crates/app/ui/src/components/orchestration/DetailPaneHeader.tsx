import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

/**
 * The container this header names, so slot content can answer the pane's width rather than the
 * window's. A detail pane is a split of an already narrow window — with the git rail at its default
 * it is 184px while the viewport is comfortable — so a slot that needs to shed its labels reaches
 * for `@max-[Nrem]/detail-header:` and not a viewport breakpoint, which would lie about the space.
 */
export const DETAIL_HEADER_CONTAINER = "detail-header";

/**
 * The column a detail pane holds its content to. Both the pinned header and the scrolling document
 * under it wear this, so they resolve to the same left edge at every width — on a wide pane a
 * full-bleed header beside a centred column reads as two unrelated layouts. Defined once because
 * the alignment only holds while both sides agree on the cap *and* the padding inside it.
 */
export const DETAIL_MEASURE = "mx-auto w-full max-w-3xl px-4";

interface DetailPaneHeaderProps {
  /** The control that leaves this pane, set leftmost. The caller owns it, so it can carry its own
   *  handles and wording; the header only decides where it sits. */
  back?: ReactNode;
  /** Controls for the subject, set opposite the back control. Must already fit a narrow pane —
   *  see `DETAIL_HEADER_CONTAINER`. */
  actions?: ReactNode;
  title: string;
  /** The subject's identifiers and state, set below the title as a rail of uniform-height chips. */
  meta?: ReactNode;
}

// The pinned header of a detail pane, in three bands with one job each: controls, then the title,
// then the subject's metadata. Controls never share a line with content and content never shares a
// line with a control, which is what keeps the bands alignable — band 1 is two clusters at one
// control height, and band 3 is chips centred against each other. Nothing here is baseline-aligned:
// a chip is a flex box and contributes its content's baseline from the middle of its own height, so
// a row mixing chips with text can never agree on one, and the fix is not to ask it to.
//
// It is the pane's only fixed row, so nothing may grow without bound except the title, which is
// allowed to wrap because it is alone on its line and is the one thing a reader came for.
export function DetailPaneHeader({ back, actions, title, meta }: DetailPaneHeaderProps) {
  return (
    <header className="@container/detail-header flex shrink-0 flex-col border-b pt-3 pb-2.5">
      {/* The rule spans the pane, but everything above it is held to the same measure the body
          below it uses, so the header and the document share one left edge instead of the header
          running full-bleed past a centred column. */}
      <div className={cn(DETAIL_MEASURE, "flex flex-col gap-2")}>
        {(back != null || actions != null) && (
          // `justify-between` rather than a flex spacer: a zero-width spacer is still a child and is
          // still charged its gaps, which is what overflowed this row at a 184px pane.
          <div className="flex h-8 items-center justify-between gap-2 overflow-hidden">
            <div className="flex min-w-0 items-center">{back}</div>
            <div className="flex shrink-0 items-center gap-1">{actions}</div>
          </div>
        )}

        {/* Wraps, and is never truncated: the list row truncates, so this is the one place a long
            title is readable in full. It is clamped only in the narrow regime, where an unbounded
            title would take most of a 480px-minimum window's height for a header. */}
        <h2 className="type-title font-[560] tracking-[var(--tracking-title)] text-pretty break-words text-foreground @max-[16rem]/detail-header:line-clamp-3">
          {title}
        </h2>

        {meta}
      </div>
    </header>
  );
}
