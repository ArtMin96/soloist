import { formatCpu, formatPorts, formatRss } from "@/lib/format";
import { isStarting } from "@/lib/status";
import { cn } from "@/lib/utils";
import type { ProcessView } from "@/domain";
import type { ProcessSignal } from "@/store/signalsContext";

interface ProcessMetaProps extends ProcessSignal {
  status: ProcessView["status"];
  ready: ProcessView["ready"];
  ports: number[];
  /** Roomy labelled form for the terminal header; the compact form is for the dense row. */
  verbose?: boolean;
  /** CPU-percent floor below which the CPU read-out is hidden; 0 (default) always shows it. */
  cpuFloor?: number;
  /** Resident-bytes floor below which the memory read-out is hidden; 0 (default) always shows it. */
  memFloor?: number;
}

// The at-a-glance read-out beside a process: its restart progress, its readiness, or its live
// ports and CPU/memory — whichever currently carries signal. Rendered in the muted monospace
// data face so digits align; saturated colour stays on the status indicator, never here. Null
// when a resting process has nothing to report. The CPU/memory read-outs are gated by the
// sidebar's usage thresholds (the caller passes the mapped floors); the terminal header passes
// none, so it always shows what it has.
export function ProcessMeta({
  status,
  ready,
  ports,
  metrics,
  restart,
  verbose = false,
  cpuFloor = 0,
  memFloor = 0,
}: ProcessMetaProps) {
  const resolved = resolve({ status, ready, ports, metrics, restart, verbose, cpuFloor, memFloor });
  if (!resolved) return null;
  return (
    <span
      data-testid="process-meta"
      title={resolved.title}
      className={cn(
        "font-mono whitespace-nowrap text-muted-foreground tabular-nums",
        verbose ? "text-xs" : "max-w-28 truncate text-[0.6875rem] @max-[220px]/sidebar:hidden",
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
  restart,
  verbose,
  cpuFloor,
  memFloor,
}: Required<
  Pick<ProcessMetaProps, "status" | "ready" | "ports" | "verbose" | "cpuFloor" | "memFloor">
> &
  ProcessSignal): {
  text: string;
  title?: string;
} | null {
  // An auto-restart in flight: show its position in the core's rate-limit window.
  if (restart != null && isStarting(status)) {
    return { text: `restarting ${restart.attempt}/${restart.limit}` };
  }
  // Only a running process reports live telemetry.
  if (status !== "Running") return null;
  // Running, but the awaited port has not bound yet.
  if (ready === "Waiting") return { text: "not ready" };

  const port = formatPorts(ports);
  // Each read-out shows only once its usage reaches the sidebar's threshold floor.
  const cpu = metrics && metrics.cpu_pct >= cpuFloor ? formatCpu(metrics.cpu_pct) : null;
  const rss = metrics && metrics.rss >= memFloor ? formatRss(metrics.rss) : null;
  const parts = verbose ? [port, cpu && `cpu ${cpu}`, rss && `rss ${rss}`] : [port, cpu, rss];
  const text = parts.filter(Boolean).join(verbose ? " · " : "  ");
  if (!text) return null;
  return { text, title: ports.length ? `Listening on ${ports.join(", ")}` : undefined };
}
