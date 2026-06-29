import { useCallback, useEffect, useMemo, useState } from "react";
import {
  addLocalCommand,
  addSharedCommand,
  editLocalCommand,
  editSharedCommand,
  makeCommandLocal,
  projectSettingsPage,
  removeLocalCommand,
  removeSharedCommand,
  renameLocalCommand,
  renameSharedCommand,
  saveCommandToYaml,
  setCommandTerminalAlerts,
  setProjectAutoStartGate,
  setProjectAutoTrustCommandChanges,
  setProjectCrashExitAlerts,
  setProjectEditorOverride,
  setProjectIcon,
  setProjectTerminalAlerts,
} from "@/api";
import { CommandList } from "@/components/project-settings/CommandList";
import { NotificationsSection } from "@/components/project-settings/NotificationsSection";
import { OverviewSection } from "@/components/project-settings/OverviewSection";
import { ProjectSettingsSection } from "@/components/project-settings/ProjectSettingsSection";
import {
  PROJECT_TABS,
  projectTabButtonId,
  type ProjectTabId,
} from "@/components/project-settings/tabs";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { cn } from "@/lib/utils";
import { monogram } from "@/store/projects";
import type { CommandOps } from "@/components/project-settings/commands";
import type { ProjectSettingsPage, ProjectView } from "@/domain";

// The per-project settings page, shown in the main content pane when a project (not a process) is
// selected. It owns the page read-model — loading it for the project and reloading after every
// mutation so the view always reflects the core — and routes between the four tabs. The sections
// are presentational: they render slices of the page and raise intent through the callbacks here,
// each of which calls one core command and reloads.
export function ProjectSettingsPane({ project }: { project: ProjectView }) {
  const [page, setPage] = useState<ProjectSettingsPage | null>(null);
  const [active, setActive] = useState<ProjectTabId>("overview");
  const [error, setError] = useState<string | null>(null);

  const id = project.id;

  const reload = useCallback(
    () =>
      projectSettingsPage(id)
        .then(setPage)
        .catch((e) => setError(String(e))),
    [id],
  );

  useEffect(() => {
    void reload();
  }, [reload]);

  // Run a mutation, then refresh the page so the view reflects the core; surface any failure inline.
  const mutate = useCallback(
    (op: () => Promise<unknown>) => {
      setError(null);
      void op()
        .then(reload)
        .catch((e) => setError(String(e)));
    },
    [reload],
  );

  const ops = useMemo<CommandOps>(
    () => ({
      edit: (cmd, spec) =>
        mutate(() =>
          (cmd.visibility === "shared" ? editSharedCommand : editLocalCommand)(id, cmd.name, spec),
        ),
      rename: (cmd, to) =>
        mutate(() =>
          (cmd.visibility === "shared" ? renameSharedCommand : renameLocalCommand)(
            id,
            cmd.name,
            to,
          ),
        ),
      setTerminalAlerts: (cmd, enabled) =>
        mutate(() => setCommandTerminalAlerts(id, cmd.name, enabled)),
      toggleStorage: (cmd) =>
        mutate(() =>
          cmd.visibility === "shared"
            ? makeCommandLocal(id, cmd.name)
            : saveCommandToYaml(id, cmd.name),
        ),
      remove: (cmd) =>
        mutate(() =>
          (cmd.visibility === "shared" ? removeSharedCommand : removeLocalCommand)(id, cmd.name),
        ),
      // Add resolves once the page reloads and rejects on failure, so the dialog can stay open.
      add: (name, spec, visibility) =>
        (visibility === "shared" ? addSharedCommand : addLocalCommand)(id, name, spec).then(() => {
          void reload();
        }),
    }),
    [id, mutate, reload],
  );

  const setIcon = useCallback(
    (icon: string) =>
      setProjectIcon(id, icon).then(() => {
        void reload();
      }),
    [id, reload],
  );

  return (
    <div className="flex h-full flex-col">
      <header className="shrink-0 px-6 pt-5">
        <div className="flex items-center gap-2.5">
          <Avatar className="size-6">
            {project.icon && <AvatarImage src={project.icon} alt="" />}
            <AvatarFallback>{monogram(project.name)}</AvatarFallback>
          </Avatar>
          <h1 className="min-w-0 truncate text-[0.9375rem] font-medium tracking-[-0.005em] text-foreground">
            {project.name}
          </h1>
        </div>
        <TabRail active={active} onSelect={setActive} />
      </header>

      <div
        role="tabpanel"
        aria-labelledby={projectTabButtonId(active)}
        className="min-h-0 flex-1 overflow-y-auto"
      >
        <div className="mx-auto max-w-2xl px-6 py-6">
          {error && <p className="mb-4 text-xs text-destructive">{error}</p>}
          {page && (
            <>
              {active === "overview" && <OverviewSection page={page} onReload={reload} />}
              {active === "settings" && (
                <ProjectSettingsSection
                  name={project.name}
                  icon={project.icon}
                  settings={page.settings}
                  resolvedEditor={page.resolved_editor}
                  onAutoStartGate={(v) => mutate(() => setProjectAutoStartGate(id, v))}
                  onAutoTrustCommandChanges={(v) =>
                    mutate(() => setProjectAutoTrustCommandChanges(id, v))
                  }
                  onEditorOverride={(v) => mutate(() => setProjectEditorOverride(id, v))}
                  onSetIcon={setIcon}
                />
              )}
              {active === "notifications" && (
                <NotificationsSection
                  settings={page.settings}
                  onCrashExitAlerts={(v) => mutate(() => setProjectCrashExitAlerts(id, v))}
                  onTerminalAlerts={(v) => mutate(() => setProjectTerminalAlerts(id, v))}
                />
              )}
              {active === "commands" && <CommandList commands={page.commands} ops={ops} />}
            </>
          )}
        </div>
      </div>
    </div>
  );
}

// The horizontal tab rail: the same selection vocabulary as the global settings rail (active tab in
// the accent), here as a bottom-border row with full roving-tabindex keyboard navigation.
function TabRail({
  active,
  onSelect,
}: {
  active: ProjectTabId;
  onSelect: (id: ProjectTabId) => void;
}) {
  const select = (next: ProjectTabId) => {
    onSelect(next);
    document.getElementById(projectTabButtonId(next))?.focus();
  };
  const step = (delta: number) => {
    const i = PROJECT_TABS.findIndex((tab) => tab.id === active);
    select(PROJECT_TABS[(i + delta + PROJECT_TABS.length) % PROJECT_TABS.length].id);
  };

  return (
    <div
      role="tablist"
      aria-label="Project settings sections"
      className="mt-4 flex gap-4 border-b border-border"
      onKeyDown={(e) => {
        if (e.key === "ArrowRight") {
          e.preventDefault();
          step(1);
        } else if (e.key === "ArrowLeft") {
          e.preventDefault();
          step(-1);
        } else if (e.key === "Home") {
          e.preventDefault();
          select(PROJECT_TABS[0].id);
        } else if (e.key === "End") {
          e.preventDefault();
          select(PROJECT_TABS[PROJECT_TABS.length - 1].id);
        }
      }}
    >
      {PROJECT_TABS.map((tab) => {
        const isActive = tab.id === active;
        return (
          <button
            key={tab.id}
            id={projectTabButtonId(tab.id)}
            type="button"
            role="tab"
            aria-selected={isActive}
            tabIndex={isActive ? 0 : -1}
            onClick={() => onSelect(tab.id)}
            className={cn(
              "relative -mb-px border-b-2 px-1 pb-2.5 text-[0.8125rem] transition-colors",
              "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring",
              isActive
                ? "border-primary text-foreground"
                : "border-transparent text-muted-foreground hover:text-foreground",
            )}
          >
            {tab.label}
          </button>
        );
      })}
    </div>
  );
}
