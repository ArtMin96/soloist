import { History, MoreHorizontal, Play, RotateCw, ShieldCheck, Square } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  presentProcessActions,
  runnableProcessActions,
  type ProcessActionHandlers,
  type ProcessActionKind,
  type RunnableProcessAction,
} from "@/lib/processActions";
import type { ProcessView } from "@/domain";

type ControlSize = "icon-xs" | "icon-sm";

interface ProcessControlsProps {
  process: ProcessView;
  handlers: ProcessActionHandlers;
  size?: ControlSize;
}

const ACTION_ICONS = {
  trust: ShieldCheck,
  resume: History,
  start: Play,
  stop: Square,
  restart: RotateCw,
} satisfies Record<ProcessActionKind, typeof Play>;

// A dense projection of the canonical runnable-action list. Exactly one action stays one-click;
// secondary actions move into a menu, and unavailable actions do not exist in the DOM. The same
// resolver also feeds palettes and row context menus, so this component never interprets status.
export function ProcessControls({ process, handlers, size = "icon-sm" }: ProcessControlsProps) {
  const runnable = runnableProcessActions(process, handlers);
  const { primary, secondary } = presentProcessActions(process.kind, process.status, runnable);
  if (primary == null) return null;

  return (
    <div className="flex items-center gap-0.5">
      <ActionButton action={primary} size={size} />
      {secondary.length > 0 && (
        <DropdownMenu>
          <Tooltip>
            <TooltipTrigger asChild>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="ghost"
                  size={size}
                  aria-label={`More actions for ${process.label}`}
                  onClick={(event) => event.stopPropagation()}
                >
                  <MoreHorizontal data-icon="inline-start" />
                </Button>
              </DropdownMenuTrigger>
            </TooltipTrigger>
            <TooltipContent>More actions for {process.label}</TooltipContent>
          </Tooltip>
          <DropdownMenuContent align="end" className="w-40">
            <DropdownMenuGroup>
              {secondary.map((action) => (
                <ActionMenuItem key={action.kind} action={action} />
              ))}
            </DropdownMenuGroup>
          </DropdownMenuContent>
        </DropdownMenu>
      )}
    </div>
  );
}

function ActionButton({ action, size }: { action: RunnableProcessAction; size: ControlSize }) {
  const Icon = ACTION_ICONS[action.kind];
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size={size}
          aria-label={action.label}
          onClick={(event) => {
            event.stopPropagation();
            action.run();
          }}
        >
          <Icon data-icon="inline-start" />
        </Button>
      </TooltipTrigger>
      <TooltipContent>{action.label}</TooltipContent>
    </Tooltip>
  );
}

function ActionMenuItem({ action }: { action: RunnableProcessAction }) {
  const Icon = ACTION_ICONS[action.kind];
  return (
    <DropdownMenuItem onClick={(event) => event.stopPropagation()} onSelect={action.run}>
      <Icon aria-hidden />
      {action.label}
    </DropdownMenuItem>
  );
}

export { ACTION_ICONS };
