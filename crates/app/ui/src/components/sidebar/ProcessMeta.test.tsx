// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { ProcessMeta } from "@/components/sidebar/ProcessMeta";
import type { ProcessMetrics } from "@/store/signals";

const MIB = 1024 * 1024;
const metrics = (cpu_pct: number, rss: number): ProcessMetrics => ({ cpu_pct, rss });

// Renders the compact row form of a Running process carrying the given metrics and floors.
function renderMeta(m: ProcessMetrics, cpuFloor: number, memFloor: number) {
  render(
    <ProcessMeta
      status="Running"
      ready="Ungated"
      ports={[]}
      metrics={m}
      cpuFloor={cpuFloor}
      memFloor={memFloor}
    />,
  );
}

afterEach(cleanup);

describe("ProcessMeta usage thresholds", () => {
  it("shows both read-outs when usage is at or above the floors", () => {
    renderMeta(metrics(50, 200 * MIB), 0, 0);
    expect(screen.getByText(/50%/)).toBeTruthy();
    expect(screen.getByText(/200 MB/)).toBeTruthy();
  });

  it("hides the CPU read-out below its floor but keeps memory", () => {
    // 50% CPU is below the 60% floor; 200 MB is above the 0 memory floor.
    renderMeta(metrics(50, 200 * MIB), 60, 0);
    expect(screen.queryByText(/50%/)).toBeNull();
    expect(screen.getByText(/200 MB/)).toBeTruthy();
  });

  it("hides both read-outs at an unreachable floor ('never')", () => {
    const { container } = render(
      <ProcessMeta
        status="Running"
        ready="Ungated"
        ports={[]}
        metrics={metrics(99, 900 * MIB)}
        cpuFloor={Infinity}
        memFloor={Infinity}
      />,
    );
    // No ports and both read-outs gated off — nothing to render.
    expect(container.textContent).toBe("");
  });
});
