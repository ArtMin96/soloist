import type { ReactNode } from "react";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

interface SkeletonListProps {
  /** How many stand-in rows to draw. */
  count: number;
  /** One row. Defaults to a single full-width line. */
  row?: (index: number) => ReactNode;
  /** Layout of the list box (gap, padding) — mirror the real list's classes. */
  className?: string;
}

/**
 * The stand-in for a list whose first read has not landed: the real list's own box, filled with
 * `count` rows, so the rhythm the rows arrive into is already on screen and nothing shifts when
 * they do.
 *
 * Purely visual and hidden from assistive tech — the region wrapping this owns the loading
 * semantics and the announcement, so a reader hears one message rather than a column of nothing.
 */
export function SkeletonList({ count, row, className }: SkeletonListProps) {
  return (
    <ul aria-hidden className={cn("flex flex-col gap-2", className)}>
      {Array.from({ length: count }, (_, index) => (
        <li key={index}>{row ? row(index) : <Skeleton className="h-8 w-full" />}</li>
      ))}
    </ul>
  );
}
