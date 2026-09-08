import { FileTextIcon, MoreHorizontalIcon, type LucideIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { humanizeName } from "@/lib/humanize";
import { TODO_STATUS_ICON } from "@/lib/todo";
import type { SessionTodo, SessionWork } from "@/domain";

/** How many items a group shows inline before the rest collapse into the overflow control. */
const INLINE_ITEM_LIMIT = 3;

interface SessionWorkBarProps {
  work: SessionWork | null;
  onOpenTodo: (todo: number) => void;
  onOpenScratchpad: (name: string) => void;
}

/** One session-work entry, normalized from either a todo or a scratchpad so both groups and the
 *  overflow menu can render either kind identically. */
interface SessionItem {
  key: string;
  label: string;
  icon: LucideIcon;
  activate: () => void;
  /** The e2e handle this item carries — a todo's id or a scratchpad's name, never both. */
  attr: Record<string, string | number>;
}

interface SessionGroupData {
  label: string;
  testId: "current" | "session";
  items: SessionItem[];
}

// The agent terminal header's live context: the coordination documents this process holds a lock
// on right now ("Current work") and everything else it touched this run ("This session"), each
// item opening the exact orchestration row it names. Presentational — the read model lives in
// `useSessionWork`; this only renders it. Renders nothing while there is nothing to show, so a
// non-agent or freshly-launched process costs the header no row.
export function SessionWorkBar({ work, onOpenTodo, onOpenScratchpad }: SessionWorkBarProps) {
  const groups = buildGroups(work, onOpenTodo, onOpenScratchpad);
  if (groups.every((group) => group.items.length === 0)) return null;

  return (
    <div data-session-work className="flex min-w-0 items-center gap-3 overflow-hidden">
      {groups.map(
        (group) => group.items.length > 0 && <SessionGroupRow key={group.testId} group={group} />,
      )}
      <OverflowMenu groups={groups} />
    </div>
  );
}

function buildGroups(
  work: SessionWork | null,
  onOpenTodo: (todo: number) => void,
  onOpenScratchpad: (name: string) => void,
): SessionGroupData[] {
  if (work == null) return [];

  const todoItem = (todo: SessionTodo): SessionItem => ({
    key: `todo-${todo.id}`,
    label: todo.title,
    icon: TODO_STATUS_ICON[todo.status],
    activate: () => onOpenTodo(todo.id),
    attr: { "data-session-todo": todo.id },
  });

  const current = work.todos.filter((todo) => todo.locked).map(todoItem);

  const session = [
    ...work.todos.filter((todo) => !todo.locked).map(todoItem),
    ...work.scratchpads.map(
      (pad): SessionItem => ({
        key: `scratchpad-${pad.name}`,
        label: humanizeName(pad.name),
        icon: FileTextIcon,
        activate: () => onOpenScratchpad(pad.name),
        attr: { "data-session-scratchpad": pad.name },
      }),
    ),
  ];

  return [
    { label: "Current work", testId: "current", items: current },
    { label: "This session", testId: "session", items: session },
  ];
}

function SessionGroupRow({ group }: { group: SessionGroupData }) {
  const inline = group.items.slice(0, INLINE_ITEM_LIMIT);
  return (
    <div
      data-session-group={group.testId}
      // `overflow-hidden` clips this group's own content once it is squeezed below its items'
      // combined width, so a shrunk item never paints over the sibling group or the controls.
      className="flex min-w-0 items-center gap-1 overflow-hidden"
    >
      <span className="type-label shrink-0 text-muted-foreground">{group.label}</span>
      {inline.map((item) => (
        <SessionItemButton key={item.key} item={item} />
      ))}
    </div>
  );
}

function SessionItemButton({ item }: { item: SessionItem }) {
  const Icon = item.icon;
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          {...item.attr}
          variant="ghost"
          size="sm"
          onClick={item.activate}
          // Overrides the shadcn `Button`'s default `shrink-0` (via `twMerge`) so the item shrinks
          // with its row instead of holding its content width and spilling past the group's clip.
          className="min-w-0 max-w-32 shrink gap-1"
        >
          <Icon aria-hidden className="size-3.5 shrink-0" />
          <span className="min-w-0 truncate">{item.label}</span>
        </Button>
      </TooltipTrigger>
      <TooltipContent>{item.label}</TooltipContent>
    </Tooltip>
  );
}

function OverflowMenu({ groups }: { groups: SessionGroupData[] }) {
  const overflow = groups.map((group) => ({
    label: group.label,
    items: group.items.slice(INLINE_ITEM_LIMIT),
  }));
  const count = overflow.reduce((total, group) => total + group.items.length, 0);
  if (count === 0) return null;

  return (
    <DropdownMenu>
      <Tooltip>
        <TooltipTrigger asChild>
          <DropdownMenuTrigger asChild>
            <Button
              data-session-overflow
              variant="ghost"
              size="icon-sm"
              aria-label={`${count} more session ${count === 1 ? "item" : "items"}`}
            >
              <MoreHorizontalIcon aria-hidden />
            </Button>
          </DropdownMenuTrigger>
        </TooltipTrigger>
        <TooltipContent>{count} more</TooltipContent>
      </Tooltip>
      <DropdownMenuContent align="end">
        {overflow.map(
          (group) =>
            group.items.length > 0 && (
              <DropdownMenuGroup key={group.label}>
                <DropdownMenuLabel>{group.label}</DropdownMenuLabel>
                {group.items.map((item) => {
                  const Icon = item.icon;
                  return (
                    <DropdownMenuItem key={item.key} {...item.attr} onSelect={item.activate}>
                      <Icon aria-hidden />
                      {item.label}
                    </DropdownMenuItem>
                  );
                })}
              </DropdownMenuGroup>
            ),
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
