import { Suspense, useCallback, useMemo, useState } from "react";
import { DeferredOverlay } from "@/components/DeferredOverlay";
import {
  CommandPalette,
  DiffPane,
  GitRail,
  LaunchPicker,
  OrchestrationPane,
  ProjectSettingsPane,
  QuickActionsPalette,
  PullRequestPane,
  QuickJumpPalette,
  SettingsOverlay,
  TerminalPane,
} from "@/components/deferredAppComponents";
import { ErrorBanner } from "@/components/ErrorBanner";
import { NotificationToasts } from "@/components/NotificationToasts";
import { OrphanDialog } from "@/components/OrphanDialog";
import { RemoveProcessDialog } from "@/components/RemoveProcessDialog";
import { Sidebar } from "@/components/sidebar/Sidebar";
import { StartSurface } from "@/components/StartSurface";
import { Titlebar } from "@/components/titlebar/Titlebar";
import { TitlebarActions } from "@/components/titlebar/TitlebarActions";
import { TooltipProvider } from "@/components/ui/tooltip";
import { TrustDialog } from "@/components/TrustDialog";
import { TrustRequestDialog } from "@/components/TrustRequestDialog";
import type { SettingsTabId } from "@/components/settings/tabs";
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
import { PULL_REQUEST, useDiffSelection } from "@/store/git/useDiffSelection";
import { handoffTarget } from "@/store/git/handoffTarget";
import { useProjects } from "@/store/projects";
import { FileDropProvider } from "@/store/FileDropProvider";
import { SignalsProvider } from "@/store/SignalsProvider";
import { useTrust } from "@/store/useTrust";
import { useTrustRequests } from "@/store/useTrustRequests";
import { AttentionContext, useAttentionMarks } from "@/store/attentionContext";
import { useWatchRefusals } from "@/store/useWatchRefusals";
import { WatchContext } from "@/store/watchContext";
import { OpenSettingsContext, type OpenSettings } from "@/store/settingsContext";
import { useAttention } from "@/store/useAttention";
import { usePresence } from "@/store/usePresence";
import { useWindowActive } from "@/store/useWindowActive";
import type { ProcessActionHandlers } from "@/lib/processActions";
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
  const trustRequests = useTrustRequests(store.refresh, store.reportError);
  const orphans = useOrphans(store.reportError);
  const watchRefusals = useWatchRefusals();
  const agents = useAgents(store.reportError);
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(null);
  const [orchestrationProjectId, setOrchestrationProjectId] = useState<number | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsTab, setSettingsTab] = useState<SettingsTabId | null>(null);
  const [quickJumpOpen, setQuickJumpOpen] = useState(false);
  const [quickActionsOpen, setQuickActionsOpen] = useState(false);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const { stop, stopAll } = store;

  const clearAlternativeView = useCallback(() => {
    setSelectedProjectId(null);
    setOrchestrationProjectId(null);
  }, []);

  // Settings, on the tab a caller named where it had a reason to name one — the assist setting is
  // reached from the commit box that needs it. The tab is forgotten on close, so the next opening
  // that names none lands back wherever the user last left it.
  const openSettings = useCallback<OpenSettings>((tab) => {
    setSettingsTab(tab ?? null);
    setSettingsOpen(true);
  }, []);
  const settingsOpenChanged = useCallback((open: boolean) => {
    setSettingsOpen(open);
    if (!open) setSettingsTab(null);
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
  const attention = useAttention();
  const attentionMarks = useAttentionMarks(attention.snapshot, store.processes);

  const selected = store.processes.find((process) => process.id === selectedId) ?? null;
  const selectedProject = projects.projects.find((p) => p.id === selectedProjectId) ?? null;
  const orchestrationProject =
    projects.projects.find((p) => p.id === orchestrationProjectId) ?? null;

  // The project whose processes the Quick Actions palette shows: whichever project currently
  // has a terminal open, or the settings / orchestration pane open.
  const activeProjectId = selected?.project ?? selectedProjectId ?? orchestrationProjectId ?? null;

  // What the split is showing, if anything. Opening a path in the rail fills it, as does asking
  // for the pull request; Escape or the split's own close empties it.
  const {
    selection: splitView,
    open: openSplit,
    close: closeSplit,
  } = useDiffSelection(activeProjectId);

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
      open_settings: () => openSettings(),
      close_agent_or_terminal: () => {
        const id = getSelectedId();
        if (id !== null) stopProcess(id);
      },
      quick_jump: () => setQuickJumpOpen(true),
      quick_actions: () => setQuickActionsOpen(true),
    }),
    [getSelectedId, openPicker, openSettings, stopProcess],
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

  // The one set of process action callbacks every surface dispatches through — the sidebar
  // tree, every pooled terminal pane, and the command palette all read this same object rather
  // than each rebuilding its own description of "what running an action does".
  const handlers: ProcessActionHandlers = {
    onTrust: trust.requestReview,
    onResume: resumeProcess,
    onStart: startProcess,
    onStop: stopProcess,
    onRestart: restartProcess,
    onRemove: requestProcessRemoval,
  };

  return (
    <AppearanceProvider>
      <SidebarSettingsProvider>
        <HotkeysProvider>
          <GlobalHotkeys handlers={hotkeyHandlers} />
          <FileDropProvider>
            <SignalsProvider>
              <AttentionContext value={attentionMarks}>
                <TooltipProvider delayDuration={400}>
                  <OpenSettingsContext value={openSettings}>
                    <div className="flex h-screen flex-col bg-canvas text-foreground">
                      <Titlebar
                        appName={info?.name ?? "Soloist"}
                        appVersion={info?.version}
                        actions={
                          <TitlebarActions
                            project={activeProjectId}
                            snapshot={attention.snapshot}
                            processes={store.processes}
                            onSelectProcess={selectProcess}
                            onClearAttention={attention.clearAll}
                          />
                        }
                      />
                      {store.error && (
                        <ErrorBanner message={store.error} onDismiss={store.clearError} />
                      )}
                      <div className="flex min-h-0 flex-1">
                        {/* Scoped to the rail: a refused watch is announced on the project header
                            it belongs to, and nothing outside the sidebar reads it. */}
                        <WatchContext value={watchRefusals}>
                          <Sidebar
                            projects={projects.projects}
                            processes={store.processes}
                            lineage={lineage}
                            selectedId={selectedId}
                            onSelect={selectProcess}
                            handlers={handlers}
                            onStartAll={store.startAll}
                            onRestartRunning={store.restartRunning}
                            onStopAll={stopProject}
                            onOpenStart={openStart}
                            startActive={!selected && !selectedProject && !orchestrationProject}
                            onOpenSettings={() => openSettings()}
                            onOpenProjectSettings={openProjectSettings}
                            onOpenOrchestration={openOrchestration}
                            onRemoveProject={projects.remove}
                            onReorderProjects={projects.reorder}
                          />
                        </WatchContext>
                        {/* A column, so the diff opens as a split at the foot of the area
                          rather than in place of what is above it. The panes keep their own
                          region: nothing here remounts the terminal when the split appears. */}
                        <main className="relative flex min-w-0 flex-1 flex-col bg-surface">
                          <div className="relative min-h-0 flex-1">
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
                                  handlers={handlers}
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
                          </div>
                          {activeProjectId !== null && splitView !== null && (
                            <Suspense fallback={null}>
                              {splitView.kind === PULL_REQUEST ? (
                                <PullRequestPane
                                  project={activeProjectId}
                                  agent={handoffTarget(selected, activeProjectId)}
                                  onClose={closeSplit}
                                />
                              ) : (
                                <DiffPane
                                  project={activeProjectId}
                                  selection={splitView}
                                  onClose={closeSplit}
                                />
                              )}
                            </Suspense>
                          )}
                        </main>
                        {/* The version-control rail sits beside the main area rather than
                          replacing it, so a repository's state stays in sight while an agent
                          keeps working. A sibling of <main> rather than a wrapper around it:
                          nothing here remounts the terminal pane when the rail's chunk lands. */}
                        {activeProjectId !== null && (
                          <Suspense fallback={null}>
                            <GitRail
                              key={activeProjectId}
                              project={activeProjectId}
                              onOpen={openSplit}
                              onOpenPullRequest={() => openSplit({ kind: PULL_REQUEST })}
                            />
                          </Suspense>
                        )}
                      </div>
                      <NotificationToasts
                        processes={store.processes}
                        onSelectProcess={selectProcess}
                      />
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
                      <TrustRequestDialog
                        requests={trustRequests.requests}
                        onApprove={trustRequests.approve}
                        onDeny={trustRequests.deny}
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
                          onOpenChange={settingsOpenChanged}
                          project={activeProjectId}
                          tab={settingsTab}
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
                          openSettings={() => openSettings()}
                          selectProcess={selectProcess}
                          openProjectSettings={openProjectSettings}
                          openOrchestration={openOrchestration}
                          startAll={store.startAll}
                          stopAll={stopProject}
                          restartRunning={store.restartRunning}
                          process={handlers}
                        />
                      </DeferredOverlay>
                    </div>
                  </OpenSettingsContext>
                </TooltipProvider>
              </AttentionContext>
            </SignalsProvider>
          </FileDropProvider>
        </HotkeysProvider>
      </SidebarSettingsProvider>
    </AppearanceProvider>
  );
}
