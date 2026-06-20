import { formatCpu, formatPorts, formatRss } from "@/lib/format";
import { isStarting, RESTART_LIMIT } from "@/lib/status";
import { cn } from "@/lib/utils";
import type { ProcessView } from "@/domain";
import type { ProcessSignal } from "@/store/signalsContext";

interface ProcessMetaProps extends ProcessSignal {
  status: ProcessView["status"];
  ready: ProcessView["ready"];
  ports: number[];
  /** Roomy labelled form for the terminal header; the compact form is for the dense row. */
  verbose?: boolean;
}

// The at-a-glance read-out beside a process: its restart progress, its readiness, or its live
// ports and CPU/memory — whichever currently carries signal. Rendered in the muted monospace
// data face so digits align; saturated colour stays on the status indicator, never here. Null
// when a resting process has nothing to report.
export function ProcessMeta({
  status,
  ready,
  ports,
  metrics,
  attempt,
  verbose = false,
}: ProcessMetaProps) {
  const resolved = resolve({ status, ready, ports, metrics, attempt, verbose });
  if (!resolved) return null;
  return (
    <span
      title={resolved.title}
      className={cn(
        "font-mono whitespace-nowrap text-muted-foreground tabular-nums",
        verbose ? "text-xs" : "text-[0.6875rem]",
      )}
    >
      {resolved.text}
    </span>
  );
}

function resolve({
  status,
  ready,
  ports,
  metrics,
  attempt,
  verbose,
}: Required<Pick<ProcessMetaProps, "status" | "ready" | "ports" | "verbose">> & ProcessSignal): {
  text: string;
  title?: string;
} | null {
  // An auto-restart in flight: show its position in the rate-limit window.
  if (attempt != null && isStarting(status)) {
    return { text: `restarting ${attempt}/${RESTART_LIMIT}` };
  }
  // Only a running process reports live telemetry.
  if (status !== "Running") return null;
  // Running, but the awaited port has not bound yet.
  if (ready === "Waiting") return { text: "not ready" };

  const port = formatPorts(ports);
  const cpu = metrics ? formatCpu(metrics.cpu_pct) : null;
  const rss = metrics ? formatRss(metrics.rss) : null;
  const parts = verbose ? [port, cpu && `cpu ${cpu}`, rss && `rss ${rss}`] : [port, cpu, rss];
  const text = parts.filter(Boolean).join(verbose ? " · " : "  ");
  if (!text) return null;
  return { text, title: ports.length ? `Listening on ${ports.join(", ")}` : undefined };
}
