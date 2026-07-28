import { Suspense, useCallback, useMemo, useState } from "react";
import { DeferredOverlay } from "@/components/DeferredOverlay";
import {
  CommandPalette,
  LaunchPicker,
  OrchestrationPane,
  ProjectSettingsPane,
  QuickActionsPalette,
  QuickJumpPalette,
  SettingsOverlay,
  TerminalPane,
} from "@/components/deferredAppComponents";
import { ErrorBanner } from "@/components/ErrorBanner";
import { OrphanDialog } from "@/components/OrphanDialog";
import { RemoveProcessDialog } from "@/components/RemoveProcessDialog";
import { Sidebar } from "@/components/sidebar/Sidebar";
import { StartSurface } from "@/components/StartSurface";
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
import { liveWorkerCount, useLineage } from "@/store/useLineage";
import { useProcesses } from "@/store/useProcesses";
import { useProcessRemoval } from "@/store/useProcessRemoval";
import { useProcessActivationNavigation } from "@/store/useProcessActivationNavigation";
import { TERMINAL_POOL_CAP, useTerminalPool } from "@/store/useTerminalPool";
import { useProjects } from "@/store/projects";
import { FileDropProvider } from "@/store/FileDropProvider";
import { SignalsProvider } from "@/store/SignalsProvider";
import { useTrust } from "@/store/useTrust";
import { usePresence } from "@/store/usePresence";
import { useWindowActive } from "@/store/useWindowActive";
import type { HotkeyAction, ProcessView } from "@/domain";

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
  const removal = useProcessRemoval(store.processes, store.close);
  const { pending: pendingRemoval, request: requestRemoval, confirm: confirmRemoval } = removal;
  const lineage = useLineage();
  const projects = useProjects(store.reportError);
  const trust = useTrust(store.refresh, store.reportError);
  const orphans = useOrphans(store.reportError);
  const agents = useAgents(store.reportError);
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(null);
  const [orchestrationProjectId, setOrchestrationProjectId] = useState<number | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [quickJumpOpen, setQuickJumpOpen] = useState(false);
  const [quickActionsOpen, setQuickActionsOpen] = useState(false);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const { stop, stopAll } = store;

  const clearAlternativeView = useCallback(() => {
    setSelectedProjectId(null);
    setOrchestrationProjectId(null);
  }, []);
  const {
    selectedId,
    selectProcess,
    deselectProcess,
    openStart,
    getSelectedId,
    processStopped,
    startProcess,
    restartProcess,
    resumeProcess,
    projectStopped,
    removalRequested,
    processRemoved,
  } = useProcessActivationNavigation(store.processes, {
    onClearAlternativeView: clearAlternativeView,
    onStart: store.start,
    onRestart: store.restart,
    onResume: store.resume,
  });
  usePresence(selectedId);

  const selected = store.processes.find((process) => process.id === selectedId) ?? null;
  const selectedProject = projects.projects.find((p) => p.id === selectedProjectId) ?? null;
  const orchestrationProject =
    projects.projects.find((p) => p.id === orchestrationProjectId) ?? null;

  // The project whose processes the Quick Actions palette shows: whichever project currently
  // has a terminal open, or the settings / orchestration pane open.
  const activeProjectId = selected?.project ?? selectedProjectId ?? orchestrationProjectId ?? null;

  // Project views deselect the process without erasing its MRU lifecycle history.
  const openProjectSettings = useCallback(
    (projectId: number) => {
      deselectProcess();
      setSelectedProjectId(projectId);
      setOrchestrationProjectId(null);
    },
    [deselectProcess],
  );
  const openOrchestration = useCallback(
    (projectId: number) => {
      deselectProcess();
      setOrchestrationProjectId(projectId);
      setSelectedProjectId(null);
    },
    [deselectProcess],
  );

  const stopProcess = useCallback(
    (id: number) => {
      stop(id);
      processStopped(id);
    },
    [stop, processStopped],
  );

  const stopProject = useCallback(
    (projectId: number) => {
      stopAll(projectId);
      projectStopped(projectId);
    },
    [stopAll, projectStopped],
  );

  const requestProcessRemoval = useCallback(
    (id: number) => {
      requestRemoval(id);
      removalRequested(id);
    },
    [requestRemoval, removalRequested],
  );

  const confirmProcessRemoval = useCallback(() => {
    const id = pendingRemoval?.id ?? null;
    confirmRemoval();
    if (id !== null) processRemoved(id);
  }, [pendingRemoval, confirmRemoval, processRemoved]);

  // Review a command by id: the row/header carries the project and name, and the review
  // shows what trusting it would run — the row itself shows only a name the solo.yml chose.
  const reviewById = useCallback(
    (id: number) => {
      const process = store.processes.find((candidate) => candidate.id === id);
      if (process) trust.requestReview(process.project, process.label);
    },
    [store.processes, trust],
  );

  // Open the launch picker, refreshing the tool list each time so detection is current.
  const { reload: reloadAgents, launch: launchAgent } = agents;
  const openPicker = useCallback(() => {
    reloadAgents();
    setPickerOpen(true);
  }, [reloadAgents]);

  // The keyboard-first paths run through the remappable keymap (the Hotkeys settings tab): a
  // pressed General chord dispatches its action's handler here. Wiring a new action is one
  // more entry; an action with no handler yet is simply inert.
  const hotkeyHandlers = useMemo<Partial<Record<HotkeyAction, () => void>>>(
    () => ({
      open_command_palette: () => setCommandPaletteOpen(true),
      new_agent_or_terminal: openPicker,
      open_settings: () => setSettingsOpen(true),
      close_agent_or_terminal: () => {
        const id = getSelectedId();
        if (id !== null) stopProcess(id);
      },
      quick_jump: () => setQuickJumpOpen(true),
      quick_actions: () => setQuickActionsOpen(true),
    }),
    [getSelectedId, openPicker, stopProcess],
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

  // Open a terminal and focus it, so the user lands on a live shell ready to type into —
  // the same landing the agent launch gives.
  const { createTerminal } = store;
  const onCreateTerminal = useCallback(
    (project: number) => {
      void createTerminal(project).then((id) => {
        if (id !== null) selectProcess(id);
      });
    },
    [createTerminal, selectProcess],
  );

  // Keep-alive terminal pool: the recently-viewed processes whose terminals stay mounted so
  // switching back is instant. The pool tracks selection over renders; the current selection is
  // folded in immediately (the effect that formalizes it lands next tick) so a first-time selection
  // never flashes blank, and the result is capped so a fold-in never mounts one past the pool cap.
  // Only the selected process renders visible — the rest sit hidden.
  const pool = useTerminalPool(
    selectedId,
    store.processes.map((process) => process.id),
  );
  const poolIds = (
    selectedId !== null && !pool.includes(selectedId) ? [selectedId, ...pool] : pool
  ).slice(0, TERMINAL_POOL_CAP);
  const poolProcesses = poolIds
    .map((id) => store.processes.find((process) => process.id === id))
    .filter((process): process is ProcessView => process !== undefined);

  return (
    <AppearanceProvider>
      <SidebarSettingsProvider>
        <HotkeysProvider>
          <GlobalHotkeys handlers={hotkeyHandlers} />
          <FileDropProvider>
            <SignalsProvider>
              <TooltipProvider delayDuration={400}>
                <div className="flex h-screen flex-col bg-background text-foreground">
                  <Titlebar appName={info?.name ?? "Soloist"} appVersion={info?.version} />
                  {store.error && (
                    <ErrorBanner message={store.error} onDismiss={store.clearError} />
                  )}
                  <div className="flex min-h-0 flex-1">
                    <Sidebar
                      projects={projects.projects}
                      processes={store.processes}
                      lineage={lineage}
                      selectedId={selectedId}
                      onSelect={selectProcess}
                      onStart={startProcess}
                      onStop={stopProcess}
                      onRestart={restartProcess}
                      onResume={resumeProcess}
                      onTrust={reviewById}
                      onRemove={requestProcessRemoval}
                      onStartAll={store.startAll}
                      onRestartRunning={store.restartRunning}
                      onStopAll={stopProject}
                      onOpenStart={openStart}
                      startActive={!selected && !selectedProject && !orchestrationProject}
                      onOpenSettings={() => setSettingsOpen(true)}
                      onOpenProjectSettings={openProjectSettings}
                      onOpenOrchestration={openOrchestration}
                      onRemoveProject={projects.remove}
                    />
                    <main className="min-w-0 flex-1">
                      <Suspense fallback={<div className="h-full w-full bg-background" />}>
                        {/* Keep-alive pool: every recently-viewed process keeps its terminal mounted
                          (xterm + live stream) so switching back is instant; only the selected one
                          is visible, the rest sit hidden with both their renderer and their byte
                          parsing paused, so a hidden pane costs no per-frame main-thread work. */}
                        {poolProcesses.map((process) => (
                          <TerminalPane
                            key={process.id}
                            process={process}
                            visible={process.id === selectedId}
                            processes={store.processes}
                            onSelectProcess={selectProcess}
                            onStart={() => startProcess(process.id)}
                            onStop={() => stopProcess(process.id)}
                            onRestart={() => restartProcess(process.id)}
                            onResume={() => resumeProcess(process.id)}
                            onTrust={() => reviewById(process.id)}
                            onRemove={() => requestProcessRemoval(process.id)}
                          />
                        ))}
                        {!selected &&
                          (selectedProject ? (
                            <ProjectSettingsPane
                              key={selectedProject.id}
                              project={selectedProject}
                            />
                          ) : orchestrationProject ? (
                            <OrchestrationPane
                              key={orchestrationProject.id}
                              project={orchestrationProject}
                            />
                          ) : (
                            <StartSurface
                              hasProjects={projects.projects.length > 0}
                              onOpenProject={projects.open}
                              onLaunchAgent={openPicker}
                              notice={projects.notice}
                            />
                          ))}
                      </Suspense>
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
                  <RemoveProcessDialog
                    process={pendingRemoval}
                    workers={
                      pendingRemoval
                        ? liveWorkerCount(lineage, store.processes, pendingRemoval.id)
                        : 0
                    }
                    onConfirm={confirmProcessRemoval}
                    onDismiss={removal.dismiss}
                  />
                  <DeferredOverlay open={pickerOpen}>
                    <LaunchPicker
                      open={pickerOpen}
                      onOpenChange={setPickerOpen}
                      tools={agents.tools}
                      projects={projects.projects}
                      onLaunch={onLaunchAgent}
                      onCreateTerminal={onCreateTerminal}
                    />
                  </DeferredOverlay>
                  <DeferredOverlay open={settingsOpen}>
                    <SettingsOverlay
                      open={settingsOpen}
                      onOpenChange={setSettingsOpen}
                      project={activeProjectId}
                    />
                  </DeferredOverlay>
                  <DeferredOverlay open={quickJumpOpen}>
                    <QuickJumpPalette
                      open={quickJumpOpen}
                      onOpenChange={setQuickJumpOpen}
                      processes={store.processes}
                      projects={projects.projects}
                      onSelectProcess={selectProcess}
                      onSelectProject={openProjectSettings}
                    />
                  </DeferredOverlay>
                  <DeferredOverlay open={quickActionsOpen}>
                    <QuickActionsPalette
                      open={quickActionsOpen}
                      onOpenChange={setQuickActionsOpen}
                      processes={store.processes}
                      projects={projects.projects}
                      activeProjectId={activeProjectId}
                      onStart={startProcess}
                      onStop={stopProcess}
                      onRestart={restartProcess}
                      onResume={resumeProcess}
                      onTrust={trust.requestReview}
                      onRemove={requestProcessRemoval}
                    />
                  </DeferredOverlay>
                  <DeferredOverlay open={commandPaletteOpen}>
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
                      stopAll={stopProject}
                      restartRunning={store.restartRunning}
                      process={{
                        onTrust: trust.requestReview,
                        onResume: resumeProcess,
                        onStart: startProcess,
                        onStop: stopProcess,
                        onRestart: restartProcess,
                        onRemove: requestProcessRemoval,
                      }}
                    />
                  </DeferredOverlay>
                </div>
              </TooltipProvider>
            </SignalsProvider>
          </FileDropProvider>
        </HotkeysProvider>
      </SidebarSettingsProvider>
    </AppearanceProvider>
  );
}
