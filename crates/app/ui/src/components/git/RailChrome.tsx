import type { ReactNode } from "react";
import { FoldVerticalIcon, UnfoldVerticalIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

const EXPAND_FOLDERS_LABEL = "Expand all folders";
const COLLAPSE_FOLDERS_LABEL = "Collapse all folders";

/** What the last refused change said, stated where the change was asked for. */
export function RailError({ message }: { message: string }) {
  return (
    <p
      role="alert"
      className="shrink-0 border-t border-sidebar-border px-3 py-2 text-[0.8125rem] text-destructive"
    >
      {message}
    </p>
  );
}

/** A compact tree control for either tab; it never changes the version-control rail. */
export function TreeExpansionButton({
  expanded,
  onClick,
}: {
  expanded: boolean;
  onClick: () => void;
}) {
  const label = expanded ? COLLAPSE_FOLDERS_LABEL : EXPAND_FOLDERS_LABEL;
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-xs"
          className="ml-auto"
          aria-label={label}
          onClick={onClick}
        >
          {expanded ? <FoldVerticalIcon /> : <UnfoldVerticalIcon />}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

/** The existing rail-level control remains separate from the Files tree disclosure control. */
export function RailButton({
  label,
  icon,
  onClick,
}: {
  label: string;
  icon: ReactNode;
  onClick: () => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="ghost" size="icon-xs" aria-label={label} onClick={onClick}>
          {icon}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

export function RailMessage({ children }: { children: ReactNode }) {
  return <p className="text-[0.8125rem] text-muted-foreground">{children}</p>;
}

/** The quiet line a tab shows when it has nothing in it. */
export function RailEmpty({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full items-center justify-center px-6 text-center">
      <RailMessage>{children}</RailMessage>
    </div>
  );
}
