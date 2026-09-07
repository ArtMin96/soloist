import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";

/** The DOM handle a row's card carries. */
export const CARD_ROW_ATTRIBUTE = "data-card-row";
/** The DOM handle the row's own button carries — the control that opens the row. */
export const CARD_TRIGGER_ATTRIBUTE = "data-card-trigger";

interface CardRowProps {
  /** Hands the pane over to this row's detail. */
  onOpen: () => void;
  children: ReactNode;
  /** A sibling control beside the trigger, never inside it (its own tab stop). */
  aside?: ReactNode;
}

/**
 * One row of a board: a full-width card whose whole face is the control that opens it, with room
 * for a single control beside it. A card rather than a line because a single baseline gives the
 * title no room against everything competing with it, and because the whole card is then one
 * target worth aiming at.
 */
export function CardRow({ onOpen, children, aside }: CardRowProps) {
  return (
    <Card
      {...{ [CARD_ROW_ATTRIBUTE]: "" }}
      size="sm"
      className="w-full min-w-0 gap-0 rounded-lg border border-border py-0 ring-0"
    >
      {/* The card's own button and the aside are siblings, not nested — a row never buries an
          interactive control inside another one, so each is its own tab stop. */}
      <CardContent className="flex items-center gap-0 p-0">
        <Button
          type="button"
          {...{ [CARD_TRIGGER_ATTRIBUTE]: "" }}
          variant="ghost"
          onClick={onOpen}
          className="h-auto min-w-0 flex-1 cursor-pointer flex-col items-stretch gap-1.5 overflow-hidden rounded-none p-3 text-left whitespace-normal hover:border-transparent hover:bg-sidebar-row-hover hover:[box-shadow:none] active:bg-sidebar-row-active active:not-aria-[haspopup]:scale-100 focus-visible:bg-sidebar-row-hover focus-visible:ring-inset supports-backdrop-filter:hover:bg-sidebar-row-hover supports-backdrop-filter:hover:backdrop-blur-none supports-backdrop-filter:hover:backdrop-saturate-100 motion-reduce:transition-none [&>*]:min-w-0"
        >
          {children}
        </Button>
        {aside}
      </CardContent>
    </Card>
  );
}

/**
 * A row's box with no control in it, for a list whose first read is still in flight: the same card
 * at the same padding, so the rows land into a rhythm that is already on screen.
 */
export function CardRowStandIn({ children }: { children: ReactNode }) {
  return (
    <Card size="sm" className="w-full min-w-0 gap-0 rounded-lg border border-border py-0 ring-0">
      <CardContent className="flex min-w-0 flex-col gap-1.5 p-3">{children}</CardContent>
    </Card>
  );
}
