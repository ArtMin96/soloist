import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
  setCommandNotificationLevel,
  setProjectAutoStartGate,
  setProjectAutoTrustCommandChanges,
  setProjectEditorOverride,
  setProjectIcon,
  setProjectNotificationLevel,
} from "@/api";
import { CommandList } from "@/components/project-settings/CommandList";
import { NotificationsSection } from "@/components/project-settings/NotificationsSection";
import { OverviewSection } from "@/components/project-settings/OverviewSection";
import { ProjectSettingsSection } from "@/components/project-settings/ProjectSettingsSection";
import { specOf } from "@/components/project-settings/spec";
import { PROJECT_TABS, type ProjectTabId } from "@/components/project-settings/tabs";
import { SegmentedControl } from "@/components/SegmentedControl";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { monogram } from "@/store/projects";
import type { CommandOps } from "@/components/project-settings/commands";
import type { Option } from "@/lib/appearance";
import type { ProcessSpec, ProjectCommandView, ProjectSettingsPage, ProjectView } from "@/domain";

// The section switch reuses the project tab list as the app's one view-switch vocabulary — the
// SegmentedControl (DESIGN.md §5), the same control the orchestration pane uses, rather than a
// second underline-tab style.
const SECTION_OPTIONS: Option<ProjectTabId>[] = PROJECT_TABS.map((tab) => ({
  value: tab.id,
  label: tab.label,
}));

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
  const pendingEdits = useRef(new Map<ProjectCommandView, ProcessSpec>());

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

  const ops = useMemo<CommandOps>(() => {
    const pending = pendingEdits.current;
    const settle = (cmd: ProjectCommandView, spec: ProcessSpec) => () => {
      if (pending.get(cmd) === spec) pending.delete(cmd);
    };
    return {
      edit: (cmd, patch) =>
        mutate(() => {
          const spec: ProcessSpec = { ...(pending.get(cmd) ?? specOf(cmd)), ...patch };
          pending.set(cmd, spec);
          return (cmd.visibility === "shared" ? editSharedCommand : editLocalCommand)(
            id,
            cmd.name,
            spec,
          ).finally(settle(cmd, spec));
        }),
      rename: (cmd, to) =>
        mutate(() =>
          (cmd.visibility === "shared" ? renameSharedCommand : renameLocalCommand)(
            id,
            cmd.name,
            to,
          ),
        ),
      setNotificationLevel: (cmd, level) =>
        mutate(() => setCommandNotificationLevel(id, cmd.name, level)),
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
    };
  }, [id, mutate, reload]);

  const setIcon = useCallback(
    (icon: string) =>
      setProjectIcon(id, icon).then(() => {
        void reload();
      }),
    [id, reload],
  );

  return (
    <section className="flex h-full min-w-0 flex-col bg-background">
      <header className="flex h-11 shrink-0 items-center gap-2.5 border-b bg-sidebar px-3">
        <Avatar className="size-5">
          {project.icon && <AvatarImage src={project.icon} alt="" />}
          <AvatarFallback>{monogram(project.name)}</AvatarFallback>
        </Avatar>
        <h1 className="min-w-0 shrink truncate text-[0.9375rem] font-[550] tracking-[-0.005em] text-foreground">
          {project.name}
        </h1>
        <div className="ml-auto shrink-0">
          <SegmentedControl<ProjectTabId>
            value={active}
            options={SECTION_OPTIONS}
            onChange={setActive}
            ariaLabel="Project settings sections"
          />
        </div>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="mx-auto max-w-2xl px-6 py-6">
          {error && <p className="mb-4 text-xs text-destructive">{error}</p>}
          {page && (
            <>
              {active === "overview" && <OverviewSection page={page} onReload={reload} />}
              {active === "settings" && (
                <ProjectSettingsSection
                  project={id}
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
                  onNotificationLevel={(level) =>
                    mutate(() => setProjectNotificationLevel(id, level))
                  }
                />
              )}
              {active === "commands" && (
                <CommandList
                  commands={page.commands}
                  projectLevel={page.settings.notification_level}
                  ops={ops}
                />
              )}
            </>
          )}
        </div>
      </div>
    </section>
  );
}
