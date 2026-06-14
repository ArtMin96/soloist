import { useCallback, useEffect, useState } from "react";
import { appInfo, type AppInfo } from "./api";
import { Button } from "@/components/ui/button";

export default function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    appInfo()
      .then((i) => {
        setInfo(i);
        setError(null);
      })
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(load, [load]);

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-3">
      <h1 className="text-2xl font-semibold tracking-tight">{info?.name ?? "Soloist"}</h1>
      {info && <p className="text-muted-foreground text-sm">version {info.version}</p>}
      {error && <p className="text-destructive text-sm">{error}</p>}
      <Button variant="outline" size="sm" onClick={load}>
        Refresh
      </Button>
    </main>
  );
}
