import { MoreHorizontal, type LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { INLINE_ABOVE, LABEL_WIDE, MENU_BELOW, SQUARE_WIDE } from "@/components/common/DetailPane";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Separator } from "@/components/ui/separator";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

/** The overflow trigger's accessible name, shared by the control and whoever addresses it. */
export const MENU_TRIGGER_LABEL = "More actions";

/** One secondary action, rendered inline while the pane is wide enough and as a menu item below. */
export interface DetailAction {
  icon: LucideIcon;
  /** Inline wording; shed to sr-only in a narrow pane. */
  label: string;
  /** Menu wording, a full phrase ("Edit todo") where the inline control says one word. */
  menuLabel: string;
  onSelect: () => void;
  /** Inline as an icon-only control named by `label`, with `label` in a tooltip. */
  iconOnly?: boolean;
  disabled?: boolean;
}

interface DetailActionsProps {
  actions: DetailAction[];
  /** The one filled control, last, after a separator; absent when the subject offers none. */
  primary?: ReactNode;
  /** The overflow trigger's tooltip, naming the subject ("More todo actions"). */
  menuTooltip: string;
}

// A detail header's action cluster, ordered secondary → separator → primary so the filled control
// is the terminal element of the row and unmistakably the one action that finishes the subject.
// Three shed steps carry it down to a 184px pane: labels go first, then the secondary set folds
// into a menu, and the primary keeps its word longest. Icon-only actions use the shared tooltip so
// their meaning remains visible to pointer and keyboard users after their labels shed.
export function DetailActions({ actions, primary, menuTooltip }: DetailActionsProps) {
  return (
    <>
      <div className={cn("flex items-center gap-1", INLINE_ABOVE)}>
        {actions.map((action) => (
          <InlineAction key={action.menuLabel} action={action} />
        ))}

        {/* Only ever drawn beside a primary action: without one it would point at nothing. `h-4`
            beats the primitive's `self-stretch`, which would otherwise run the rule the band's
            full height. */}
        {primary != null && <Separator orientation="vertical" className="mx-1 h-4" />}
      </div>

      <DropdownMenu>
        <Tooltip>
          <TooltipTrigger asChild>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={MENU_TRIGGER_LABEL}
                className={cn("text-muted-foreground", MENU_BELOW)}
              >
                <MoreHorizontal aria-hidden />
              </Button>
            </DropdownMenuTrigger>
          </TooltipTrigger>
          <TooltipContent>{menuTooltip}</TooltipContent>
        </Tooltip>
        <DropdownMenuContent align="end">
          <DropdownMenuGroup>
            {actions.map((action) => (
              <DropdownMenuItem
                key={action.menuLabel}
                onSelect={action.onSelect}
                disabled={action.disabled}
              >
                <action.icon aria-hidden /> {action.menuLabel}
              </DropdownMenuItem>
            ))}
          </DropdownMenuGroup>
        </DropdownMenuContent>
      </DropdownMenu>

      {primary}
    </>
  );
}

function InlineAction({ action }: { action: DetailAction }) {
  const Icon = action.icon;

  if (action.iconOnly) {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={action.onSelect}
            disabled={action.disabled}
            aria-label={action.label}
            className="text-muted-foreground"
          >
            <Icon aria-hidden />
          </Button>
        </TooltipTrigger>
        <TooltipContent>{action.label}</TooltipContent>
      </Tooltip>
    );
  }

  return (
    <Button
      variant="ghost"
      size="sm"
      onClick={action.onSelect}
      disabled={action.disabled}
      className={cn("text-muted-foreground", SQUARE_WIDE)}
    >
      <Icon aria-hidden data-icon="inline-start" />
      <span className={LABEL_WIDE}>{action.label}</span>
    </Button>
  );
}
