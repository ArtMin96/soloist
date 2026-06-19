import { useCallback, useState } from "react";
import { EmptyState } from "@/components/EmptyState";
import { ErrorBanner } from "@/components/ErrorBanner";
import { OrphanDialog } from "@/components/OrphanDialog";
import { Sidebar } from "@/components/sidebar/Sidebar";
import { TerminalPane } from "@/components/terminal/TerminalPane";
import { Toolbar } from "@/components/Toolbar";
import { TrustDialog } from "@/components/TrustDialog";
import { useAppInfo } from "@/store/useAppInfo";
import { useOrphans } from "@/store/useOrphans";
import { useProcesses } from "@/store/useProcesses";
import { useProjects } from "@/store/useProjects";
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
  const [selectedId, setSelectedId] = useState<number | null>(null);

  const selected = store.processes.find((process) => process.id === selectedId) ?? null;

  // Trust a command by id: the row/header carries the project and name the gate needs.
  const trustById = useCallback(
    (id: number) => {
      const process = store.processes.find((candidate) => candidate.id === id);
      if (process) trust.trust(process.project, process.label);
    },
    [store.processes, trust],
  );

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <Toolbar
        projectName={info?.name ?? "Soloist"}
        appVersion={info?.version}
        canBulk={store.projectId !== null}
        onOpenProject={projects.open}
        onStartAll={store.startAll}
        onStopAll={store.stopAll}
        onRestartRunning={store.restartRunning}
      />
      {store.error && <ErrorBanner message={store.error} onDismiss={store.clearError} />}
      <div className="flex min-h-0 flex-1">
        <Sidebar
          processes={store.processes}
          selectedId={selectedId}
          onSelect={setSelectedId}
          onStart={store.start}
          onStop={store.stop}
          onRestart={store.restart}
          onTrust={trustById}
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
            <EmptyState hasProcesses={store.processes.length > 0} onOpenProject={projects.open} />
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
    </div>
  );
}
