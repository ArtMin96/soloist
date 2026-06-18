import { useState } from "react";
import { EmptyState } from "@/components/EmptyState";
import { ErrorBanner } from "@/components/ErrorBanner";
import { Sidebar } from "@/components/sidebar/Sidebar";
import { TerminalPane } from "@/components/terminal/TerminalPane";
import { Toolbar } from "@/components/Toolbar";
import { useAppInfo } from "@/store/useAppInfo";
import { useProcesses } from "@/store/useProcesses";

// The dashboard shell: a top bar of stack controls, the process tree, and the selected
// process's terminal. All state is a projection of the core read model; this composes the
// pieces and tracks only which process is selected.
export default function App() {
  const info = useAppInfo();
  const store = useProcesses();
  const [selectedId, setSelectedId] = useState<number | null>(null);

  const selected = store.processes.find((process) => process.id === selectedId) ?? null;

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <Toolbar
        projectName={info?.name ?? "Soloist"}
        appVersion={info?.version}
        canBulk={store.projectId !== null}
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
        />
        <main className="min-w-0 flex-1">
          {selected ? (
            <TerminalPane
              key={selected.id}
              process={selected}
              onStart={() => store.start(selected.id)}
              onStop={() => store.stop(selected.id)}
              onRestart={() => store.restart(selected.id)}
            />
          ) : (
            <EmptyState hasProcesses={store.processes.length > 0} />
          )}
        </main>
      </div>
    </div>
  );
}
