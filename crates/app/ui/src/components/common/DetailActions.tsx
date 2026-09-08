import { MoreHorizontal, type LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { INLINE_ABOVE, MENU_BELOW } from "@/components/common/DetailPane";
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

/** One secondary action, rendered inline while the pane is wide enough and as a menu item below. */
export interface DetailAction {
  icon: LucideIcon;
  /** Inline wording; shed to sr-only in a narrow pane. */
  label: string;
  /** Menu wording, a full phrase ("Edit todo") where the inline control says one word. */
  menuLabel: string;
  onSelect: () => void;
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
// Secondary actions are equally compact icon controls with visible tooltips, then fold into one
// overflow menu when the pane narrows. The primary keeps its label longest.
export function DetailActions({ actions, primary, menuTooltip }: DetailActionsProps) {
  return (
    <div className="flex h-7 items-center gap-1">
      <div className={cn("items-center gap-1", INLINE_ABOVE)}>
        {actions.map((action) => (
          <InlineAction key={action.menuLabel} action={action} />
        ))}
      </div>

      <DropdownMenu>
        <Tooltip>
          <TooltipTrigger asChild>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={menuTooltip}
                className={cn("text-icon-muted", MENU_BELOW)}
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
                className="data-disabled:text-text-muted data-disabled:opacity-100"
              >
                <action.icon aria-hidden /> {action.menuLabel}
              </DropdownMenuItem>
            ))}
          </DropdownMenuGroup>
        </DropdownMenuContent>
      </DropdownMenu>

      {primary != null && (
        <Separator orientation="vertical" className="mx-1 h-4 data-vertical:self-center" />
      )}

      {primary}
    </div>
  );
}

function InlineAction({ action }: { action: DetailAction }) {
  const Icon = action.icon;

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={action.onSelect}
          disabled={action.disabled}
          aria-label={action.label}
          className="text-icon-muted"
        >
          <Icon aria-hidden />
        </Button>
      </TooltipTrigger>
      <TooltipContent>{action.label}</TooltipContent>
    </Tooltip>
  );
}
