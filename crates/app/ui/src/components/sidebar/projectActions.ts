import { Network, Play, RotateCw, Settings, Square } from "lucide-react";
import type { LucideIcon } from "lucide-react";

// The handlers a project header needs, each already scoped to its project by the Sidebar.
export interface ProjectActionHandlers {
  onStartAll: () => void;
  onRestartRunning: () => void;
  onStopAll: () => void;
  onOpenOrchestration: () => void;
  onOpenProjectSettings: () => void;
}

export interface ProjectAction {
  id: string;
  label: string;
  Icon: LucideIcon;
  run: () => void;
}

// The project-level actions, in two groups: the bulk supervisor commands (scoped to this
// project) and the project's views. One source of truth, rendered into both the header's
// ••• menu and the row's right-click menu so the two can never drift — and so the header
// row carries the project name, not five competing buttons.
export function projectActions(handlers: ProjectActionHandlers): {
  bulk: ProjectAction[];
  views: ProjectAction[];
} {
  return {
    bulk: [
      { id: "start-all", label: "Start all", Icon: Play, run: handlers.onStartAll },
      {
        id: "restart-running",
        label: "Restart running",
        Icon: RotateCw,
        run: handlers.onRestartRunning,
      },
      { id: "stop-all", label: "Stop all", Icon: Square, run: handlers.onStopAll },
    ],
    views: [
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
  };
}
