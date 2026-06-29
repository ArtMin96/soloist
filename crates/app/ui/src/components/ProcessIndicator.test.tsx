// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { ProcessIndicator } from "@/components/ProcessIndicator";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { AgentActivity, ProcStatus } from "@/domain";

function renderIndicator(status: ProcStatus, activity?: AgentActivity, showLabel = false) {
  return render(
    <TooltipProvider>
      <ProcessIndicator status={status} activity={activity} showLabel={showLabel} />
    </TooltipProvider>,
  );
}

afterEach(cleanup);

describe("ProcessIndicator", () => {
  it("shows a running agent's activity instead of its status", () => {
    const { container } = renderIndicator("Running", "Working");
    expect(container.querySelector("[data-activity='Working']")).toBeTruthy();
    expect(screen.getByText("Working")).toBeTruthy();
  });

  it("falls back to the status dot for a running agent with no activity yet", () => {
    const { container } = renderIndicator("Running", undefined);
    expect(container.querySelector("[data-status='Running']")).toBeTruthy();
    expect(container.querySelector("[data-activity]")).toBeNull();
  });

  it("shows the status, not stale activity, once the agent is no longer running", () => {
    const { container } = renderIndicator("Stopped", "Working");
    expect(container.querySelector("[data-status='Stopped']")).toBeTruthy();
    expect(container.querySelector("[data-activity]")).toBeNull();
  });

  it("shows a non-running status as itself", () => {
    const { container } = renderIndicator("Crashed");
    expect(container.querySelector("[data-status='Crashed']")).toBeTruthy();
  });

  it("renders an inline label in the header form", () => {
    renderIndicator("Running", "Permission", true);
    expect(screen.getByText("Permission")).toBeTruthy();
  });
});
