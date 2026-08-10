import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

/**
 * A compact icon control with a tooltip: the app's one shape for a toolbar, rail or split-header
 * action that carries no text of its own.
 *
 * The label names the action to a screen reader and, unless a `hint` says more, is what the tooltip
 * shows — so an icon is never left to speak for itself.
 */
export function IconButton({
  label,
  hint,
  icon,
  onClick,
  className,
}: {
  label: string;
  /** What the tooltip says when the action needs more words than its name. Defaults to the label. */
  hint?: string;
  icon: ReactNode;
  onClick: () => void;
  className?: string;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label={label}
          className={className}
          onClick={onClick}
        >
          {icon}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{hint ?? label}</TooltipContent>
    </Tooltip>
  );
}
