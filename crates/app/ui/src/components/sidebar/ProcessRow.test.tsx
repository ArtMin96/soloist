// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { ProcessRow } from "@/components/sidebar/ProcessRow";
import { TooltipProvider } from "@/components/ui/tooltip";
import { EMPTY_SIGNALS, type SignalState } from "@/store/signals";
import { EMPTY_STORE, fixedSignalStore } from "@/store/signalStore";
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
      <SignalsContext value={fixedSignalStore(signals)}>
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

describe("ProcessRow as a tree row", () => {
  it("stays a flat level-1 row without a tree column", () => {
    renderRow(running);
    const row = screen.getByRole("treeitem", { name: /web/ });
    expect(row.getAttribute("aria-level")).toBe("1");
    expect(row.getAttribute("aria-expanded")).toBeNull();
    expect(screen.queryByRole("button", { name: /workers/ })).toBeNull();
  });

  it("exposes a lead's disclosure with its expanded state", () => {
    render(
      <TooltipProvider>
        <SignalsContext value={EMPTY_STORE}>
          <ProcessRow
            process={running}
            selected={false}
            onSelect={noop}
            onStart={noop}
            onStop={noop}
            onRestart={noop}
            onResume={noop}
            onTrust={noop}
            treeColumn
            hasChildren
            expanded={false}
            onToggleExpand={noop}
          />
        </SignalsContext>
      </TooltipProvider>,
    );
    const row = screen.getByRole("treeitem", { name: /web/ });
    expect(row.getAttribute("aria-expanded")).toBe("false");
    expect(screen.getByRole("button", { name: "Expand web's workers" })).toBeTruthy();
  });

  it("toggles the disclosure without selecting the row", () => {
    let selected = 0;
    let toggled = 0;
    render(
      <TooltipProvider>
        <SignalsContext value={EMPTY_STORE}>
          <ProcessRow
            process={running}
            selected={false}
            onSelect={() => {
              selected += 1;
            }}
            onStart={noop}
            onStop={noop}
            onRestart={noop}
            onResume={noop}
            onTrust={noop}
            treeColumn
            hasChildren
            expanded
            onToggleExpand={() => {
              toggled += 1;
            }}
          />
        </SignalsContext>
      </TooltipProvider>,
    );
    screen.getByRole("button", { name: "Collapse web's workers" }).click();
    expect(toggled).toBe(1);
    expect(selected).toBe(0);
  });

  it("indents one step per lineage level", () => {
    render(
      <TooltipProvider>
        <SignalsContext value={EMPTY_STORE}>
          <ProcessRow
            process={running}
            selected={false}
            onSelect={noop}
            onStart={noop}
            onStop={noop}
            onRestart={noop}
            onResume={noop}
            onTrust={noop}
            depth={1}
            treeColumn
          />
        </SignalsContext>
      </TooltipProvider>,
    );
    const row = screen.getByRole("treeitem", { name: /web/ });
    expect(row.style.paddingLeft).toBe("26px");
    expect(row.getAttribute("aria-level")).toBe("2");
  });
});
