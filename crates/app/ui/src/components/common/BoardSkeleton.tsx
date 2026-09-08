/* eslint-disable react-refresh/only-export-components -- the width cycle and the helper that reads
   it are one source; a board's own row stand-in needs the same widths this strip is drawn from. */
import type { ReactNode } from "react";
import { SkeletonList } from "@/components/common/SkeletonList";
import { Skeleton } from "@/components/ui/skeleton";

// Titles are the one line whose length actually varies from row to row, so the stand-ins cycle
// through unequal widths — a column of identical bars reads as a grid, which is not what arrives.
const TITLE_WIDTHS = ["w-3/5", "w-4/5", "w-2/5", "w-3/4", "w-1/2", "w-2/3"] as const;

/** The width a stand-in title bar takes at `index`, cycling so no two neighbours match. */
export function skeletonTitleWidth(index: number): string {
  return TITLE_WIDTHS[index % TITLE_WIDTHS.length];
}

interface BoardSkeletonProps {
  /** Stand-ins for the toolbar's facet controls, after the search stand-in; mirror the real toolbar's widths. */
  facets: ReactNode;
  rows: number;
  row: (index: number) => ReactNode;
}

/**
 * A board while its first read is in flight: the toolbar strip and a column of row stand-ins in the
 * board's own boxes, at the heights and gaps the real rows use, so the rows land into a layout that
 * is already settled rather than pushing one into place.
 */
export function BoardSkeleton({ facets, rows, row }: BoardSkeletonProps) {
  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 flex-col gap-1.5 border-b px-3 py-2">
        <div className="flex flex-wrap items-center gap-2">
          <Skeleton className="h-7 min-w-32 flex-1 basis-40" />
          {facets}
          <Skeleton className="h-3.5 w-12 shrink-0" />
          <Skeleton className="h-7 w-24 shrink-0" />
        </div>
      </div>

      <SkeletonList count={rows} className="px-3 py-2" row={row} />
    </div>
  );
}
