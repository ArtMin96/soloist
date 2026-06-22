import { useCallback, useEffect, useState } from "react";
import { AgentPicker } from "@/components/AgentPicker";
import { EmptyState } from "@/components/EmptyState";
import { ErrorBanner } from "@/components/ErrorBanner";
import { OrphanDialog } from "@/components/OrphanDialog";
import { Sidebar } from "@/components/sidebar/Sidebar";
import { TerminalPane } from "@/components/terminal/TerminalPane";
import { Toolbar } from "@/components/Toolbar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { TrustDialog } from "@/components/TrustDialog";
import { useAgents } from "@/store/useAgents";
import { useAppInfo } from "@/store/useAppInfo";
import { useOrphans } from "@/store/useOrphans";
import { useProcesses } from "@/store/useProcesses";
import { useProjects } from "@/store/projects";
import { SignalsProvider } from "@/store/SignalsProvider";
import { useTrust } from "@/store/useTrust";

// The dashboard shell: a top bar of stack controls, the process tree, and the selected
// process's terminal. All state is a projection of the core read model; this composes the
// pieces and tracks only which process is selected.
export default function App() {
  const info = useAppInfo();
  const store = useProcesses();
  const projects = useProjects(store.reportError);
  const trust = useTrust(store.refresh, store.reportError);
  const orphans = useOrphans();
  const agents = useAgents(store.reportError);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);

  const selected = store.processes.find((process) => process.id === selectedId) ?? null;

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

  // Cmd/Ctrl+T opens the picker from anywhere — the keyboard-first launch path.
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && !event.altKey && event.key.toLowerCase() === "t") {
        event.preventDefault();
        openPicker();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [openPicker]);

  // Launch an agent and focus its new terminal, so the user lands on the running agent.
  const onLaunchAgent = useCallback(
    (project: number, tool: string, extraArgs: string[]) => {
      void launchAgent(project, tool, extraArgs).then((id) => {
        if (id !== null) setSelectedId(id);
      });
    },
    [launchAgent],
  );

  return (
    <SignalsProvider>
      <TooltipProvider delayDuration={400}>
        <div className="flex h-screen flex-col bg-background text-foreground">
          <Toolbar
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
              selectedId={selectedId}
              onSelect={setSelectedId}
              onStart={store.start}
              onStop={store.stop}
              onRestart={store.restart}
              onTrust={trustById}
              onStartAll={store.startAll}
              onRestartRunning={store.restartRunning}
              onStopAll={store.stopAll}
            />
            <main className="min-w-0 flex-1">
              {selected ? (
                <TerminalPane
                  key={selected.id}
                  process={selected}
                  onStart={() => store.start(selected.id)}
                  onStop={() => store.stop(selected.id)}
                  onRestart={() => store.restart(selected.id)}
                  onTrust={() => trustById(selected.id)}
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
            activeProjectId={selected?.project ?? null}
            onLaunch={onLaunchAgent}
          />
        </div>
      </TooltipProvider>
    </SignalsProvider>
  );
}
