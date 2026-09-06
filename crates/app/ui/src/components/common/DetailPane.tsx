import { ChevronLeft, TriangleAlert } from "lucide-react";
import type { ReactNode } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
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

/**
 * The handle on the control that leaves a detail pane. The route machinery parks focus here when
 * the detail arrives, and the end-to-end walks read it, so it is stated once and imported.
 */
export const DETAIL_BACK_ATTRIBUTE = "data-detail-back";

/** The control that leaves edit mode, on whichever subject the pane is showing. */
export const DETAIL_DONE_ATTRIBUTE = "data-detail-done";

/**
 * A control's label, shown only while the pane can carry it. It stays in the accessibility tree at
 * every width — a control that loses its name when a split narrows is a control nobody can identify
 * — so it is hidden the screen-reader way rather than removed. Paired with a collapse to a square
 * button, since a hidden label still leaves its icon gap and its horizontal padding behind.
 */
export const LABEL_WIDE = "@max-[20rem]/detail-header:sr-only";

/** The square collapse `LABEL_WIDE` is paired with. */
export const SQUARE_WIDE =
  "@max-[20rem]/detail-header:size-7 @max-[20rem]/detail-header:justify-center @max-[20rem]/detail-header:gap-0 @max-[20rem]/detail-header:p-0";

/**
 * The last label to go. The pane's primary action holds its word through every regime but the
 * narrowest, because it is the one control with a running state to say.
 */
export const LABEL_FLOOR = "@max-[12rem]/detail-header:sr-only";

/** The square collapse `LABEL_FLOOR` is paired with. */
export const SQUARE_FLOOR =
  "@max-[12rem]/detail-header:size-7 @max-[12rem]/detail-header:justify-center @max-[12rem]/detail-header:gap-0 @max-[12rem]/detail-header:p-0";

/**
 * Below this the secondary actions stop being rendered inline and become menu items instead. The
 * cluster and the trigger are mutually exclusive rather than both-rendered-one-hidden, so an action
 * is never two tab stops.
 */
export const INLINE_ABOVE = "@max-[15rem]/detail-header:hidden";

/** The overflow trigger's half of the exchange `INLINE_ABOVE` describes. */
export const MENU_BELOW = "@min-[15rem]/detail-header:hidden";

interface DetailPaneHeaderProps {
  /** The control that leaves this pane, set leftmost. The caller owns it, so it can carry its own
   *  handles and wording; the header only decides where it sits. */
  back?: ReactNode;
  /** Controls for the subject, set opposite the back control. Must already fit a narrow pane —
   *  see `DETAIL_HEADER_CONTAINER`. */
  actions?: ReactNode;
  /** A string is the pane's heading; a node is a title control that renders its own heading, such
   *  as a rename-in-place field. */
  title: ReactNode;
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
        {typeof title === "string" ? (
          <h2 className="type-title font-[560] tracking-[var(--tracking-title)] text-pretty break-words text-foreground @max-[16rem]/detail-header:line-clamp-3">
            {title}
          </h2>
        ) : (
          title
        )}

        {meta}
      </div>
    </header>
  );
}

/** The control that leaves a detail pane for the list it belongs to. */
export function DetailBackButton({
  destination,
  onClick,
}: {
  /** Lowercase plural the pane returns to: "todos" gives the label "Todos" and the name
   *  "Back to todos". */
  destination: string;
  onClick: () => void;
}) {
  return (
    <Button
      {...{ [DETAIL_BACK_ATTRIBUTE]: "" }}
      variant="ghost"
      size="sm"
      onClick={onClick}
      aria-label={`Back to ${destination}`}
      className={cn(
        "-ml-2.5 min-w-0 text-muted-foreground @max-[20rem]/detail-header:ml-0",
        SQUARE_WIDE,
      )}
    >
      <ChevronLeft aria-hidden data-icon="inline-start" />
      {/* Names the destination, which is what a back control in a two-panel board should say. */}
      <span className={LABEL_WIDE}>
        {destination.charAt(0).toUpperCase() + destination.slice(1)}
      </span>
    </Button>
  );
}

/**
 * A refusal of something the header just asked for. Pinned rather than scrolled: it answers a
 * control in the header, so it may not be somewhere the reader has already scrolled past by the
 * time it appears — mount it as a direct child of the pane's root, between header and body.
 */
export function DetailNotice({ message }: { message: string }) {
  return (
    <Alert variant="destructive" className="shrink-0 rounded-none border-x-0 border-t-0">
      <TriangleAlert aria-hidden />
      <AlertDescription>{message}</AlertDescription>
    </Alert>
  );
}

/** The pane's scrolling region: everything under the pinned header, at the shared measure. */
export function DetailBody({ children }: { children: ReactNode }) {
  return (
    <div className="min-h-0 flex-1 overflow-auto">
      {/* Capped so a long body reads at a comfortable measure instead of running the pane's width,
          and held to the same column the pinned header uses so the two share a left edge. */}
      <div className={cn(DETAIL_MEASURE, "flex flex-col gap-5 py-4")}>{children}</div>
    </div>
  );
}
