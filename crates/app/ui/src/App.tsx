import { ProcessList } from "@/components/ProcessList";
import { Toolbar } from "@/components/Toolbar";
import { useAppInfo } from "@/store/useAppInfo";
import { useProcesses } from "@/store/useProcesses";

// Debug harness: a deliberately minimal panel that proves the
// invoke -> facade -> event -> webview thread end to end. It composes the process
// store and reusable components; the real dashboard is built through the design
// system in a dedicated surface.
export default function App() {
  const info = useAppInfo();
  const { processes, error, start, stop, refresh } = useProcesses();

  return (
    <main className="mx-auto flex min-h-screen max-w-xl flex-col gap-4 p-6 font-mono text-sm">
      <header className="flex items-baseline justify-between border-b pb-2">
        <h1 className="text-base font-semibold">{info?.name ?? "Soloist"}</h1>
        <span className="text-muted-foreground">{info ? `v${info.version} · debug` : "debug"}</span>
      </header>

      <Toolbar onStart={start} onRefresh={refresh} />

      {error && (
        <p className="text-destructive" role="alert">
          {error}
        </p>
      )}

      <ProcessList processes={processes} onStop={stop} />
    </main>
  );
}
