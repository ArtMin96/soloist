import { SkeletonList } from "@/components/common/SkeletonList";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

/** Card stand-ins the board draws: enough to fill the list at the minimum window height. */
const TODO_SKELETON_ROWS = 6;

// Titles are the one line whose length actually varies from row to row, so the stand-ins cycle
// through unequal widths — a column of identical bars reads as a grid, which is not what arrives.
const TITLE_WIDTHS = ["w-3/5", "w-4/5", "w-2/5", "w-3/4", "w-1/2", "w-2/3"] as const;

/**
 * The to-do board while its first read is in flight: the toolbar strip and a column of card
 * stand-ins in the board's own boxes, at the heights and gaps the real rows use, so the todos land
 * into a layout that is already settled rather than pushing one into place.
 */
export function TodoBoardSkeleton() {
  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 flex-col gap-1.5 border-b px-3 py-2">
        <div className="flex flex-wrap items-center gap-2">
          <Skeleton className="h-7 min-w-32 flex-1 basis-40" />
          <Skeleton className="h-7 w-32 shrink-0" />
          <Skeleton className="h-7 w-36 shrink-0" />
        </div>
      </div>

      <SkeletonList
        count={TODO_SKELETON_ROWS}
        className="px-3 py-2"
        row={(index) => (
          <Card size="sm" className="w-full gap-0 rounded-lg py-0">
            <CardContent className="flex flex-col gap-1.5 px-2 py-1.5">
              <Skeleton className={cn("h-4", TITLE_WIDTHS[index % TITLE_WIDTHS.length])} />
              <div className="flex items-center gap-2">
                <Skeleton className="h-4 w-12 rounded-full" />
                <Skeleton className="h-4 w-10 rounded-full" />
                <Skeleton className="h-4 w-14 rounded-full" />
              </div>
            </CardContent>
          </Card>
        )}
      />
    </div>
  );
}
