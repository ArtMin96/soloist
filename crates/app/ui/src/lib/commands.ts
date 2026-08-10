import { HOTKEY_ACTION_LABELS } from "@/lib/hotkeys";
import { runnableProcessActions, type ProcessActionHandlers } from "@/lib/processActions";
import { requestBranchSwitcher, type BranchClusterView } from "@/store/git/branchCluster";
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
  /**
   * What the window chrome is showing about the checked-out branch, or null when no repository is in
   * view. Its controls sit in the far corner of the title bar; the palette offers the same actions,
   * routed to the same callbacks rather than to a second implementation of them.
   */
  git: BranchClusterView | null;
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

/**
 * The version-control group's own wording. The chrome's controls are icons in the title bar's far
 * corner, where there is room for a glyph and a tooltip; a palette row has room to say what the
 * action does, and being found by typing is the whole point of these entries.
 */
const GIT_HEADING = "Version control";
const SWITCH_BRANCH = "Switch branch…";
const FETCH = "Fetch from the remote";
const PULL = "Pull from the upstream";
const PUSH = "Push to the upstream";
const PUBLISH = "Publish this branch";
const STOP_EXCHANGE = "Stop waiting on the remote";
const SHOW_PULL_REQUEST = "Show this branch's pull request";
const GIT_KEYWORDS = ["git", "version control", "branch"];

/**
 * The actions the window chrome offers for what is checked out, as palette entries.
 *
 * Each runs the callback its control runs, and each is offered exactly when its control is: nothing
 * here decides what version control allows. While an exchange with the remote is under way the three
 * that reach it give way to the one that ends it, the same way the strip's buttons do.
 */
function gitCommands(git: BranchClusterView): Command[] {
  const commands: Command[] = [];
  if (git.branchActions !== null) {
    commands.push({
      id: "git:switch-branch",
      label: SWITCH_BRANCH,
      keywords: [...GIT_KEYWORDS, "checkout", "create", "stash"],
      run: requestBranchSwitcher,
    });
  }
  const { exchange } = git;
  if (exchange !== null) {
    if (git.exchanging) {
      commands.push({
        id: "git:stop-exchange",
        label: STOP_EXCHANGE,
        keywords: [...GIT_KEYWORDS, "cancel", "abort"],
        run: exchange.stop,
      });
    } else {
      const unpublished = git.branch.upstream === null;
      commands.push({
        id: "git:fetch",
        label: FETCH,
        keywords: [...GIT_KEYWORDS, "remote", "refresh", "origin"],
        run: exchange.fetch,
      });
      if (git.capabilities.pull) {
        commands.push({
          id: "git:pull",
          label: PULL,
          keywords: [...GIT_KEYWORDS, "remote", "merge", "rebase", "incoming"],
          run: exchange.pull,
        });
      }
      if (git.capabilities.push) {
        commands.push({
          id: "git:push",
          label: unpublished ? PUBLISH : PUSH,
          keywords: [...GIT_KEYWORDS, "remote", "upstream", "outgoing"],
          run: exchange.push,
        });
      }
    }
  }
  if (git.openPullRequest !== null) {
    commands.push({
      id: "git:pull-request",
      label: SHOW_PULL_REQUEST,
      keywords: [...GIT_KEYWORDS, "pr", "gh", "github", "propose", "review", "forge"],
      run: git.openPullRequest,
    });
  }
  return commands;
}

// Builds the command-palette registry from the live app state and its wired callbacks. Grouped
// the way the user reaches for them: app-wide actions, what is checked out, appearance, then each
// open project (its bulk stack controls and navigation) and finally every process (focus + its
// status-aware actions, from the shared `runnableProcessActions` source). A new capability becomes
// one entry here and appears in the palette automatically — there is no second place to register it.
export function buildCommands(ctx: CommandContext): CommandGroup[] {
  const groups: CommandGroup[] = [
    {
      heading: "Actions",
      commands: [
        {
          id: "action:new",
          label: HOTKEY_ACTION_LABELS.new_agent_or_terminal,
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
          // Settings is where several features are switched on at all, so the words for those
          // features belong here: the palette scores a search against label plus keywords, and a
          // term absent from both scores zero and hides the entry entirely. Anyone hunting for the
          // tool that drafts commit messages reaches it by the name of the thing they want.
          label: "Open settings",
          keywords: [
            "preferences",
            "hotkeys",
            "appearance",
            "agent",
            "assist",
            "draft",
            "ai commit message",
          ],
          run: ctx.openSettings,
        },
      ],
    },
  ];

  const git = ctx.git === null ? [] : gitCommands(ctx.git);
  if (git.length > 0) groups.push({ heading: GIT_HEADING, commands: git });

  groups.push({
    heading: "Appearance",
    commands: THEMES.map((theme) => ({
      id: `theme:${theme}`,
      label: `Theme: ${THEME_LABELS[theme]}`,
      keywords: ["theme", "appearance", "dark", "light", "system"],
      run: () => ctx.setTheme(theme),
    })),
  });

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

  const projectNameById = new Map(ctx.projects.map((p) => [p.id, p.name] as const));
  const processCommands: Command[] = [];
  for (const proc of ctx.processes) {
    const projectName = projectNameById.get(proc.project) ?? "";
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
