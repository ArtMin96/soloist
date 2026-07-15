import { lazy, Suspense, useCallback, useMemo, useRef, useState } from "react";
import { DeferredOverlay } from "@/components/DeferredOverlay";
import { EmptyState } from "@/components/EmptyState";
import { ErrorBanner } from "@/components/ErrorBanner";
import { OrphanDialog } from "@/components/OrphanDialog";
import { Sidebar } from "@/components/sidebar/Sidebar";
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
import { TERMINAL_POOL_CAP, useTerminalPool } from "@/store/useTerminalPool";
import { useProjects } from "@/store/projects";
import { SignalsProvider } from "@/store/SignalsProvider";
import { useTrust } from "@/store/useTrust";
import { useWindowActive } from "@/store/useWindowActive";
import type { HotkeyAction, ProcessView } from "@/domain";

// The main-area panes and the overlays are code-split: each loads its own chunk the first time it
// is shown, keeping the heaviest dependencies (the xterm.js emulator behind the terminal, cmdk and
// the settings primitives behind the palettes) out of the initial bundle. The shell — sidebar,
// titlebar, empty state, and the safety-critical trust/orphan dialogs — stays eager.
const TerminalPane = lazy(() =>
  import("@/components/terminal/TerminalPane").then((m) => ({ default: m.TerminalPane })),
);
const ProjectSettingsPane = lazy(() =>
  import("@/components/project-settings/ProjectSettingsPane").then((m) => ({
    default: m.ProjectSettingsPane,
  })),
);
const OrchestrationPane = lazy(() =>
  import("@/components/orchestration/OrchestrationPane").then((m) => ({
    default: m.OrchestrationPane,
  })),
);
const SettingsOverlay = lazy(() =>
  import("@/components/settings/SettingsOverlay").then((m) => ({ default: m.SettingsOverlay })),
);
const AgentPicker = lazy(() =>
  import("@/components/AgentPicker").then((m) => ({ default: m.AgentPicker })),
);
const QuickJumpPalette = lazy(() =>
  import("@/components/QuickJumpPalette").then((m) => ({ default: m.QuickJumpPalette })),
);
const QuickActionsPalette = lazy(() =>
  import("@/components/QuickActionsPalette").then((m) => ({ default: m.QuickActionsPalette })),
);
const CommandPalette = lazy(() =>
  import("@/components/CommandPalette").then((m) => ({ default: m.CommandPalette })),
);

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
  const orphans = useOrphans(store.reportError);
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
                          onStart={() => store.start(process.id)}
                          onStop={() => store.stop(process.id)}
                          onRestart={() => store.restart(process.id)}
                          onResume={() => store.resume(process.id)}
                          onTrust={() => trustById(process.id)}
                        />
                      ))}
                      {!selected &&
                        (selectedProject ? (
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
                <DeferredOverlay open={pickerOpen}>
                  <AgentPicker
                    open={pickerOpen}
                    onOpenChange={setPickerOpen}
                    tools={agents.tools}
                    projects={projects.projects}
                    onLaunch={onLaunchAgent}
                  />
                </DeferredOverlay>
                <DeferredOverlay open={settingsOpen}>
                  <SettingsOverlay open={settingsOpen} onOpenChange={setSettingsOpen} />
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
                    onStart={store.start}
                    onStop={store.stop}
                    onRestart={store.restart}
                    onResume={store.resume}
                    onTrust={trust.trust}
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
                </DeferredOverlay>
              </div>
            </TooltipProvider>
          </SignalsProvider>
        </HotkeysProvider>
      </SidebarSettingsProvider>
    </AppearanceProvider>
  );
}
