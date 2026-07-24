// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ProcessControls } from "@/components/ProcessControls";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { ProcessActionHandlers } from "@/lib/processActions";
import type { ProcessView } from "@/domain";

const noop = () => {};
const handlers: ProcessActionHandlers = {
  onTrust: noop,
  onResume: noop,
  onStart: noop,
  onStop: noop,
  onRestart: noop,
};

function process(overrides: Partial<ProcessView> = {}): ProcessView {
  return {
    id: 1,
    project: 1,
    kind: "Agent",
    label: "Claude",
    status: "Stopped",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
    ...overrides,
  };
}

function renderControls(overrides: Partial<ProcessView>, actionHandlers = handlers) {
  return render(
    <TooltipProvider>
      <ProcessControls process={process(overrides)} handlers={actionHandlers} />
    </TooltipProvider>,
  );
}

afterEach(cleanup);

describe("ProcessControls", () => {
  it("renders only Start for a stopped non-resumable process", () => {
    renderControls({ status: "Stopped" });
    expect(screen.getByLabelText("Start")).toBeTruthy();
    expect(screen.queryByLabelText("Stop")).toBeNull();
    expect(screen.queryByLabelText("Restart")).toBeNull();
  });

  it("makes Resume primary and progressively discloses Start fresh", () => {
    renderControls({ status: "Stopped", resumable: true });
    expect(screen.getByLabelText("Resume last session")).toBeTruthy();
    expect(screen.queryByLabelText("Start")).toBeNull();
    expect(screen.getByLabelText("More actions for Claude")).toBeTruthy();
  });

  it("invokes Resume from the primary action", () => {
    const onResume = vi.fn();
    renderControls({ status: "Stopped", resumable: true }, { ...handlers, onResume });
    fireEvent.click(screen.getByLabelText("Resume last session"));
    expect(onResume).toHaveBeenCalledWith(1);
  });

  it("renders no conflicting controls while stopping", () => {
    const { container } = renderControls({ status: "Stopping", resumable: true });
    expect(container.querySelector("button")).toBeNull();
  });

  it("prioritizes Restart for a running command", () => {
    renderControls({ kind: "Command", status: "Running" });
    expect(screen.getByLabelText("Restart")).toBeTruthy();
    expect(screen.getByLabelText("More actions for Claude")).toBeTruthy();
    expect(screen.queryByLabelText("Stop")).toBeNull();
  });

  it("offers only Trust for an untrusted resting command", () => {
    renderControls({ kind: "Command", status: "Stopped", requires_trust: true });
    expect(screen.getByLabelText("Trust")).toBeTruthy();
    expect(screen.queryByLabelText("Start")).toBeNull();
  });
});
