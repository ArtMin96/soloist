// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { ProcessRow } from "@/components/sidebar/ProcessRow";
import { TooltipProvider } from "@/components/ui/tooltip";
import { EMPTY_SIGNALS, type SignalState } from "@/store/signals";
import { SignalsContext } from "@/store/signalsContext";
import type { ProcessView } from "@/domain";

const noop = () => {};

const running: ProcessView = {
  id: 1,
  project: 1,
  kind: "Command",
  label: "web",
  status: "Running",
  exit_code: null,
  requires_trust: false,
  resumable: false,
  ports: [],
  ready: "Ungated",
};

function renderRow(process: ProcessView, signals: SignalState = EMPTY_SIGNALS) {
  return render(
    <TooltipProvider>
      <SignalsContext value={signals}>
        <ProcessRow
          process={process}
          selected={false}
          onSelect={noop}
          onStart={noop}
          onStop={noop}
          onRestart={noop}
          onResume={noop}
          onTrust={noop}
        />
      </SignalsContext>
    </TooltipProvider>,
  );
}

afterEach(cleanup);

describe("ProcessRow telemetry", () => {
  it("shows the listening port and CPU/memory for a running process", () => {
    renderRow(
      { ...running, ports: [5173] },
      {
        metrics: new Map([[1, { cpu_pct: 4, rss: 86 * 1024 * 1024 }]]),
        attempts: new Map(),
        activity: new Map(),
      },
    );
    const meta = screen.getByText(/:5173/);
    expect(meta.textContent).toContain("4%");
    expect(meta.textContent).toContain("86 MB");
  });

  it("shows 'not ready' while a running process awaits its port", () => {
    renderRow({ ...running, ready: "Waiting" });
    expect(screen.getByText("not ready")).toBeTruthy();
  });

  it("shows the auto-restart attempt against the limit while restarting", () => {
    renderRow(
      { ...running, status: "Starting" },
      { metrics: new Map(), attempts: new Map([[1, 3]]), activity: new Map() },
    );
    expect(screen.getByText("restarting 3/10")).toBeTruthy();
  });

  it("shows no telemetry for a stopped process", () => {
    renderRow({ ...running, status: "Stopped" });
    expect(screen.queryByText(/:|restarting|not ready/)).toBeNull();
  });
});
