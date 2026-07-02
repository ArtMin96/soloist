import { useCallback, useMemo, useRef, useState } from "react";
import { AgentPicker } from "@/components/AgentPicker";
import { CommandPalette } from "@/components/CommandPalette";
import { EmptyState } from "@/components/EmptyState";
import { ErrorBanner } from "@/components/ErrorBanner";
import { OrchestrationPane } from "@/components/orchestration/OrchestrationPane";
import { OrphanDialog } from "@/components/OrphanDialog";
import { ProjectSettingsPane } from "@/components/project-settings/ProjectSettingsPane";
import { QuickActionsPalette } from "@/components/QuickActionsPalette";
import { QuickJumpPalette } from "@/components/QuickJumpPalette";
import { SettingsOverlay } from "@/components/settings/SettingsOverlay";
import { Sidebar } from "@/components/sidebar/Sidebar";
import { TerminalPane } from "@/components/terminal/TerminalPane";
import { Titlebar } from "@/components/titlebar/Titlebar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { TrustDialog } from "@/components/TrustDialog";
import { AppearanceProvider } from "@/store/AppearanceProvider";
import { HotkeysProvider } from "@/store/HotkeysProvider";
import { SidebarSettingsProvider } from "@/store/SidebarSettingsProvider";
import { useAgents } from "@/store/useAgents";
import { useAppInfo } from "@/store/useAppInfo";
import { useGlobalHotkeys } from "@/store/useGlobalHotkeys";
import { useOrphans } from "@/store/useOrphans";
import { useLineage } from "@/store/useLineage";
import { useProcesses } from "@/store/useProcesses";
import { useProjects } from "@/store/projects";
import { SignalsProvider } from "@/store/SignalsProvider";
import { useTrust } from "@/store/useTrust";
import { useWindowActive } from "@/store/useWindowActive";
import type { HotkeyAction } from "@/domain";

// Binds the live keymap to the app's actions; rendered inside HotkeysProvider so it reads the
// keymap the settings panel edits. Returns nothing — it only installs the global key listener.
function GlobalHotkeys({ handlers }: { handlers: Partial<Record<HotkeyAction, () => void>> }) {
  useGlobalHotkeys(handlers);
  return null;
}

// The dashboard shell: a top bar of stack controls, the process tree, and the selected
// process's terminal. All state is a projection of the core read model; this composes the
// pieces and tracks only which process is selected.
export default function App() {
  useWindowActive();
  const info = useAppInfo();
  const store = useProcesses();
  const lineage = useLineage();
  const projects = useProjects(store.reportError);
  const trust = useTrust(store.refresh, store.reportError);
  const orphans = useOrphans();
  const agents = useAgents(store.reportError);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(null);
  const [orchestrationProjectId, setOrchestrationProjectId] = useState<number | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [quickJumpOpen, setQuickJumpOpen] = useState(false);
  const [quickActionsOpen, setQuickActionsOpen] = useState(false);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);

  const selected = store.processes.find((process) => process.id === selectedId) ?? null;
  const selectedProject = projects.projects.find((p) => p.id === selectedProjectId) ?? null;
  const orchestrationProject =
    projects.projects.find((p) => p.id === orchestrationProjectId) ?? null;

  // The project whose processes the Quick Actions palette shows: whichever project currently
  // has a terminal open, or the settings / orchestration pane open.
  const activeProjectId = selected?.project ?? selectedProjectId ?? orchestrationProjectId ?? null;

  // The main pane shows one of: a process terminal, a project's settings, its orchestration tree,
  // or the empty state. The three selections are mutually exclusive, so opening one clears the
  // others and exactly one view is active.
  const selectProcess = useCallback((id: number) => {
    setSelectedId(id);
    setSelectedProjectId(null);
    setOrchestrationProjectId(null);
  }, []);
  const openProjectSettings = useCallback((projectId: number) => {
    setSelectedProjectId(projectId);
    setSelectedId(null);
    setOrchestrationProjectId(null);
  }, []);
  const openOrchestration = useCallback((projectId: number) => {
    setOrchestrationProjectId(projectId);
    setSelectedId(null);
    setSelectedProjectId(null);
  }, []);

  // Trust a command by id: the row/header carries the project and name the gate needs.
  const trustById = useCallback(
    (id: number) => {
      const process = store.processes.find((candidate) => candidate.id === id);
      if (process) trust.trust(process.project, process.label);
    },
    [store.processes, trust],
  );

  // Open the launch picker, refreshing the tool list each time so detection is current.
  const { reload: reloadAgents, launch: launchAgent } = agents;
  const openPicker = useCallback(() => {
    reloadAgents();
    setPickerOpen(true);
  }, [reloadAgents]);

  // Stable ref so the hotkey closure always sees the latest selection without re-creating
  // the handlers object (and re-binding the global listener) on every process click.
  const selectedIdRef = useRef(selectedId);
  selectedIdRef.current = selectedId;

  // The keyboard-first paths run through the remappable keymap (the Hotkeys settings tab): a
  // pressed General chord dispatches its action's handler here. Wiring a new action is one
  // more entry; an action with no handler yet is simply inert.
  const { stop } = store;
  const hotkeyHandlers = useMemo<Partial<Record<HotkeyAction, () => void>>>(
    () => ({
      open_command_palette: () => setCommandPaletteOpen(true),
      new_agent_or_terminal: openPicker,
      open_settings: () => setSettingsOpen(true),
      close_agent_or_terminal: () => {
        if (selectedIdRef.current !== null) stop(selectedIdRef.current);
      },
      quick_jump: () => setQuickJumpOpen(true),
      quick_actions: () => setQuickActionsOpen(true),
    }),
    [openPicker, stop],
  );

  // Launch an agent and focus its new terminal, so the user lands on the running agent.
  const onLaunchAgent = useCallback(
    (project: number, tool: string, extraArgs: string[]) => {
      void launchAgent(project, tool, extraArgs).then((id) => {
        if (id !== null) selectProcess(id);
      });
    },
    [launchAgent, selectProcess],
  );

  return (
    <AppearanceProvider>
      <SidebarSettingsProvider>
        <HotkeysProvider>
          <GlobalHotkeys handlers={hotkeyHandlers} />
          <SignalsProvider>
            <TooltipProvider delayDuration={400}>
              <div className="flex h-screen flex-col bg-background text-foreground">
                <Titlebar
                  appName={info?.name ?? "Soloist"}
                  appVersion={info?.version}
                  onOpenProject={projects.open}
                  onLaunchAgent={openPicker}
                />
                {store.error && <ErrorBanner message={store.error} onDismiss={store.clearError} />}
                <div className="flex min-h-0 flex-1">
                  <Sidebar
                    projects={projects.projects}
                    processes={store.processes}
                    lineage={lineage}
                    selectedId={selectedId}
                    onSelect={selectProcess}
                    onStart={store.start}
                    onStop={store.stop}
                    onRestart={store.restart}
                    onResume={store.resume}
                    onTrust={trustById}
                    onStartAll={store.startAll}
                    onRestartRunning={store.restartRunning}
                    onStopAll={store.stopAll}
                    onOpenSettings={() => setSettingsOpen(true)}
                    onOpenProjectSettings={openProjectSettings}
                    onOpenOrchestration={openOrchestration}
                  />
                  <main className="min-w-0 flex-1">
                    {selected ? (
                      <TerminalPane
                        key={selected.id}
                        process={selected}
                        processes={store.processes}
                        onSelectProcess={selectProcess}
                        onStart={() => store.start(selected.id)}
                        onStop={() => store.stop(selected.id)}
                        onRestart={() => store.restart(selected.id)}
                        onResume={() => store.resume(selected.id)}
                        onTrust={() => trustById(selected.id)}
                      />
                    ) : selectedProject ? (
                      <ProjectSettingsPane key={selectedProject.id} project={selectedProject} />
                    ) : orchestrationProject ? (
                      <OrchestrationPane
                        key={orchestrationProject.id}
                        project={orchestrationProject}
                      />
                    ) : (
                      <EmptyState
                        hasProcesses={store.processes.length > 0}
                        onOpenProject={projects.open}
                        notice={projects.notice}
                      />
                    )}
                  </main>
                </div>
                <OrphanDialog
                  orphans={orphans.orphans}
                  onKillOne={orphans.killOne}
                  onKillAll={orphans.killAll}
                  onLeave={orphans.leave}
                />
                <TrustDialog
                  review={trust.review}
                  onTrustCommand={(name) => {
                    if (trust.review) trust.trust(trust.review.project, name);
                  }}
                  onTrustAll={trust.trustAll}
                  onDismiss={trust.dismiss}
                />
                <AgentPicker
                  open={pickerOpen}
                  onOpenChange={setPickerOpen}
                  tools={agents.tools}
                  projects={projects.projects}
                  onLaunch={onLaunchAgent}
                />
                <SettingsOverlay open={settingsOpen} onOpenChange={setSettingsOpen} />
                <QuickJumpPalette
                  open={quickJumpOpen}
                  onOpenChange={setQuickJumpOpen}
                  processes={store.processes}
                  projects={projects.projects}
                  onSelectProcess={selectProcess}
                  onSelectProject={openProjectSettings}
                />
                <QuickActionsPalette
                  open={quickActionsOpen}
                  onOpenChange={setQuickActionsOpen}
                  processes={store.processes}
                  projects={projects.projects}
                  activeProjectId={activeProjectId}
                  onStart={store.start}
                  onStop={store.stop}
                  onRestart={store.restart}
                  onResume={store.resume}
                  onTrust={trust.trust}
                />
                <CommandPalette
                  open={commandPaletteOpen}
                  onOpenChange={setCommandPaletteOpen}
                  processes={store.processes}
                  projects={projects.projects}
                  newAgentOrTerminal={openPicker}
                  openProject={projects.open}
                  openSettings={() => setSettingsOpen(true)}
                  selectProcess={selectProcess}
                  openProjectSettings={openProjectSettings}
                  openOrchestration={openOrchestration}
                  startAll={store.startAll}
                  stopAll={store.stopAll}
                  restartRunning={store.restartRunning}
                  process={{
                    onTrust: trust.trust,
                    onResume: store.resume,
                    onStart: store.start,
                    onStop: store.stop,
                    onRestart: store.restart,
                  }}
                />
              </div>
            </TooltipProvider>
          </SignalsProvider>
        </HotkeysProvider>
      </SidebarSettingsProvider>
    </AppearanceProvider>
  );
}
