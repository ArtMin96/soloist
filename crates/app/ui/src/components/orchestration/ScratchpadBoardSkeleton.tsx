import { BoardSkeleton, skeletonTitleWidth } from "@/components/common/BoardSkeleton";
import { CardRowStandIn } from "@/components/common/CardRow";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

/** Card stand-ins the board draws: enough to fill the list at the minimum window height. */
const SCRATCHPAD_SKELETON_ROWS = 6;

/**
 * The scratchpad board while its first read is in flight: the toolbar strip — with stand-ins for the
 * archived and sort selects — over a column of card stand-ins carrying the two bands a real card
 * has, its title beside the meta rail and the gist beneath, so the scratchpads land into a layout
 * that is already settled rather than pushing one into place.
 */
export function ScratchpadBoardSkeleton() {
  return (
    <BoardSkeleton
      facets={
        <>
          <Skeleton className="h-7 w-28 shrink-0" />
          <Skeleton className="h-7 w-28 shrink-0" />
        </>
      }
      rows={SCRATCHPAD_SKELETON_ROWS}
      row={(index) => (
        <CardRowStandIn>
          <div className="flex items-center gap-2">
            <Skeleton className={cn("h-4", skeletonTitleWidth(index))} />
            <div className="ml-auto flex items-center gap-2">
              <Skeleton className="h-4 w-10 rounded-full" />
              <Skeleton className="h-4 w-8 rounded-full" />
              <Skeleton className="h-4 w-12 rounded-full" />
            </div>
          </div>
          <Skeleton className="h-3.5 w-4/5" />
        </CardRowStandIn>
      )}
    />
  );
}
