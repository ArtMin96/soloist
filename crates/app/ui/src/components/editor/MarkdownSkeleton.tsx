import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

type MarkdownSkeletonProps = (
  | {
      /** The Markdown the stand-in holds the place of; its length sets how many lines are drawn. */
      markdown: string;
    }
  | {
      /** For a body not yet read, whose length is therefore unknown: how many lines to hold. */
      lines: number;
    }
) & { className?: string };

/** Characters a prose line carries — the measure the rendered body is capped at. */
const CHARS_PER_LINE = 72;

/** A body stands in for at least one line, and never for more than fills the block it sits in. */
const MIN_LINES = 1;
const MAX_LINES = 8;

// A paragraph's lines do not all reach the measure, so the stand-in's do not either: near-full
// widths in an uneven cycle, and a short closing line the way prose actually ends.
const LINE_WIDTHS = ["w-full", "w-11/12", "w-full", "w-10/12"] as const;
const LAST_LINE_WIDTH = "w-2/5";

/**
 * The stand-in for a Markdown body that has not been rendered yet: bars at the prose's own line
 * pitch, as many as the text is long — or as many as a caller holding the place of a body it has
 * not read yet asks for — so the block occupies the height the words will and the reading position
 * does not move when they arrive.
 *
 * Purely visual — the wrapper around it owns the busy state and the announcement.
 */
export function MarkdownSkeleton(props: MarkdownSkeletonProps) {
  const { className } = props;
  const lines = held("markdown" in props ? estimateLineCount(props.markdown) : props.lines);

  return (
    // A bar the height of the text sitting in the gap that completes its line-height: the column's
    // rhythm is the rendered paragraph's, not a generic stack of blocks.
    <div className={cn("flex flex-col gap-2", className)}>
      {Array.from({ length: lines }, (_, index) => (
        <Skeleton key={index} className={cn("h-3", lineWidth(index, lines))} />
      ))}
    </div>
  );
}

/**
 * How many lines a body is likely to take: what it wraps to at the measure, but never fewer than the
 * lines it was written on, so a short list does not stand in as one bar.
 */
function estimateLineCount(markdown: string): number {
  const written = markdown.split("\n").length;
  const wrapped = Math.ceil(markdown.length / CHARS_PER_LINE);
  return Math.max(written, wrapped);
}

/** Held to the block's bounds, whichever side the count came from. */
function held(lines: number): number {
  return Math.min(Math.max(lines, MIN_LINES), MAX_LINES);
}

function lineWidth(index: number, lines: number): string {
  const closing = index === lines - 1 && lines > MIN_LINES;
  return closing ? LAST_LINE_WIDTH : LINE_WIDTHS[index % LINE_WIDTHS.length];
}
