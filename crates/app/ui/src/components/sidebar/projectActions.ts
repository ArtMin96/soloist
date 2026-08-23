import {
  ArrowDown,
  ArrowUp,
  Network,
  Play,
  RotateCw,
  Settings,
  Square,
  Trash2,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

// The handlers a project header needs, each already scoped to its project by the Sidebar.
export interface ProjectActionHandlers {
  onStartAll: () => void;
  onRestartRunning: () => void;
  onStopAll: () => void;
  onOpenOrchestration: () => void;
  onOpenProjectSettings: () => void;
  onRemoveProject: () => void;
  /** Move the project one place toward the top, or `null` when it is already there. */
  onMoveUp: (() => void) | null;
  /** Move the project one place toward the bottom, or `null` when it is already there. */
  onMoveDown: (() => void) | null;
}

export interface ProjectAction {
  id: string;
  label: string;
  Icon: LucideIcon;
  run: () => void;
}

/** One group of a project menu, rendered behind its own separator. */
export interface ProjectActionSection {
  id: "bulk" | "views" | "arrange" | "danger";
  actions: ProjectAction[];
  /** Marks every action in the section for the menus' destructive treatment. */
  destructive?: boolean;
}

// The project-level actions, in canonical order: the bulk supervisor commands (scoped to this
// project), the project's views, its place in the list, and the destructive removal — rendered
// last, behind its own separator, so it can never be a slip of the pointer from a routine
// action. One source of truth, rendered into both the header's ••• menu and the row's
// right-click menu so the two can never drift — and so the header row carries the project name,
// not a row of buttons. `onRemoveProject` opens the confirm dialog; the removal itself only runs
// from there. The move actions are how the list is rearranged without a pointer, so a project at
// an end of the list simply does not offer the move it cannot make — and a project with no move
// at all skips the section entirely rather than rendering it empty.
export function projectActions(handlers: ProjectActionHandlers): ProjectActionSection[] {
  const { onMoveUp, onMoveDown } = handlers;
  const arrange: ProjectAction[] = [
    ...(onMoveUp ? [{ id: "move-up", label: "Move up", Icon: ArrowUp, run: onMoveUp }] : []),
    ...(onMoveDown
      ? [{ id: "move-down", label: "Move down", Icon: ArrowDown, run: onMoveDown }]
      : []),
  ];

  const sections: ProjectActionSection[] = [
    {
      id: "bulk",
      actions: [
        { id: "start-all", label: "Start all", Icon: Play, run: handlers.onStartAll },
        {
          id: "restart-running",
          label: "Restart running",
          Icon: RotateCw,
          run: handlers.onRestartRunning,
        },
        { id: "stop-all", label: "Stop all", Icon: Square, run: handlers.onStopAll },
      ],
    },
    {
      id: "views",
      actions: [
        {
          id: "orchestration",
          label: "Orchestration",
          Icon: Network,
          run: handlers.onOpenOrchestration,
        },
        {
          id: "project-settings",
          label: "Project settings",
          Icon: Settings,
          run: handlers.onOpenProjectSettings,
        },
      ],
    },
    { id: "arrange", actions: arrange },
    {
      id: "danger",
      actions: [
        {
          id: "remove-project",
          label: "Remove project",
          Icon: Trash2,
          run: handlers.onRemoveProject,
        },
      ],
      destructive: true,
    },
  ];

  return sections.filter((section) => section.actions.length > 0);
}
