import { BoardSkeleton, skeletonTitleWidth } from "@/components/common/BoardSkeleton";
import { CardRowStandIn } from "@/components/common/CardRow";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

/** Card stand-ins the board draws: enough to fill the list at the minimum window height. */
const TODO_SKELETON_ROWS = 6;

/**
 * The to-do board while its first read is in flight: the toolbar strip — with stand-ins for the
 * status select and the grouping toggle — over a column of card stand-ins at the heights and gaps
 * the real rows use, so the todos land into a layout that is already settled rather than pushing
 * one into place.
 */
export function TodoBoardSkeleton() {
  return (
    <BoardSkeleton
      facets={
        <>
          <Skeleton className="h-7 w-32 shrink-0" />
          <Skeleton className="h-7 w-36 shrink-0" />
        </>
      }
      rows={TODO_SKELETON_ROWS}
      row={(index) => (
        <CardRowStandIn>
          <Skeleton className={cn("h-4", skeletonTitleWidth(index))} />
          <div className="flex items-center gap-2">
            <Skeleton className="h-4 w-12 rounded-full" />
            <Skeleton className="h-4 w-10 rounded-full" />
            <Skeleton className="h-4 w-14 rounded-full" />
          </div>
        </CardRowStandIn>
      )}
    />
  );
}
