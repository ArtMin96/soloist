import { runnableProcessActions, type ProcessActionHandlers } from "@/lib/processActions";
import type { ProcessView, ProjectView, Theme } from "@/domain";

/** One runnable command in the palette: a stable id (React key / search identity), a label, extra
 *  fuzzy-search keywords, and the action to run. */
export interface Command {
  id: string;
  label: string;
  keywords: string[];
  run: () => void;
}

/** A titled group of commands. */
export interface CommandGroup {
  heading: string;
  commands: Command[];
}

/** The live data and wired callbacks the registry turns into commands. Every entry is a capability
 *  the app already exposes — the registry never invents an action. */
export interface CommandContext {
  processes: ProcessView[];
  projects: ProjectView[];
  theme: Theme;
  newAgentOrTerminal: () => void;
  openProject: () => void;
  openSettings: () => void;
  setTheme: (theme: Theme) => void;
  selectProcess: (id: number) => void;
  openProjectSettings: (id: number) => void;
  openOrchestration: (id: number) => void;
  startAll: (project: number) => void;
  stopAll: (project: number) => void;
  restartRunning: (project: number) => void;
  process: ProcessActionHandlers;
}

const THEME_LABELS: Record<Theme, string> = {
  light: "Light",
  dark: "Dark",
  system: "System",
};

const THEMES: Theme[] = ["light", "dark", "system"];

// Builds the command-palette registry from the live app state and its wired callbacks. Grouped
// the way the user reaches for them: app-wide actions, appearance, then each open project (its bulk
// stack controls and navigation) and finally every process (focus + its status-aware actions, from
// the shared `runnableProcessActions` source). A new capability becomes one entry here and appears
// in the palette automatically — there is no second place to register it.
export function buildCommands(ctx: CommandContext): CommandGroup[] {
  const groups: CommandGroup[] = [
    {
      heading: "Actions",
      commands: [
        {
          id: "action:new",
          label: "New agent or terminal",
          keywords: ["launch", "agent", "terminal", "spawn"],
          run: ctx.newAgentOrTerminal,
        },
        {
          id: "action:open-project",
          label: "Open project…",
          keywords: ["folder", "add", "import"],
          run: ctx.openProject,
        },
        {
          id: "action:settings",
          label: "Open settings",
          keywords: ["preferences", "hotkeys", "appearance"],
          run: ctx.openSettings,
        },
      ],
    },
    {
      heading: "Appearance",
      commands: THEMES.map((theme) => ({
        id: `theme:${theme}`,
        label: `Theme: ${THEME_LABELS[theme]}`,
        keywords: ["theme", "appearance", "dark", "light", "system"],
        run: () => ctx.setTheme(theme),
      })),
    },
  ];

  for (const project of ctx.projects) {
    groups.push({
      heading: project.name,
      commands: [
        {
          id: `project:${project.id}:start-all`,
          label: `Start all — ${project.name}`,
          keywords: ["bulk", "stack", project.name],
          run: () => ctx.startAll(project.id),
        },
        {
          id: `project:${project.id}:stop-all`,
          label: `Stop all — ${project.name}`,
          keywords: ["bulk", "stack", project.name],
          run: () => ctx.stopAll(project.id),
        },
        {
          id: `project:${project.id}:restart-running`,
          label: `Restart running — ${project.name}`,
          keywords: ["bulk", "stack", project.name],
          run: () => ctx.restartRunning(project.id),
        },
        {
          id: `project:${project.id}:settings`,
          label: `Open settings — ${project.name}`,
          keywords: [project.name],
          run: () => ctx.openProjectSettings(project.id),
        },
        {
          id: `project:${project.id}:orchestration`,
          label: `Open orchestration — ${project.name}`,
          keywords: [project.name, "agents", "tree"],
          run: () => ctx.openOrchestration(project.id),
        },
      ],
    });
  }

  const processCommands: Command[] = [];
  for (const proc of ctx.processes) {
    const projectName = ctx.projects.find((p) => p.id === proc.project)?.name ?? "";
    processCommands.push({
      id: `process:${proc.id}:focus`,
      label: `Focus ${proc.label}`,
      keywords: [projectName, "open", "terminal", "jump"],
      run: () => ctx.selectProcess(proc.id),
    });
    for (const action of runnableProcessActions(proc, ctx.process)) {
      processCommands.push({
        id: `process:${proc.id}:${action.kind}`,
        label: `${action.label} ${proc.label}`,
        keywords: [projectName],
        run: action.run,
      });
    }
  }
  if (processCommands.length > 0) {
    groups.push({ heading: "Processes", commands: processCommands });
  }

  return groups;
}
