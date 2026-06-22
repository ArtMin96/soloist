// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { ProcessIndicator } from "@/components/ProcessIndicator";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { AgentActivity, ProcessView } from "@/domain";

const agent: ProcessView = {
  id: 1,
  project: 1,
  kind: "Agent",
  label: "Claude",
  status: "Running",
  exit_code: null,
  requires_trust: false,
  ports: [],
  ready: "Ungated",
};

function renderIndicator(process: ProcessView, activity?: AgentActivity, showLabel = false) {
  return render(
    <TooltipProvider>
      <ProcessIndicator process={process} activity={activity} showLabel={showLabel} />
    </TooltipProvider>,
  );
}

afterEach(cleanup);

describe("ProcessIndicator", () => {
  it("shows a running agent's activity instead of its status", () => {
    const { container } = renderIndicator(agent, "Working");
    expect(container.querySelector("[data-activity='Working']")).toBeTruthy();
    expect(screen.getByText("Working")).toBeTruthy();
  });

  it("falls back to the status dot for a running agent with no activity yet", () => {
    const { container } = renderIndicator(agent, undefined);
    expect(container.querySelector("[data-status='Running']")).toBeTruthy();
    expect(container.querySelector("[data-activity]")).toBeNull();
  });

  it("shows the status, not stale activity, once the agent is no longer running", () => {
    const { container } = renderIndicator({ ...agent, status: "Stopped" }, "Working");
    expect(container.querySelector("[data-status='Stopped']")).toBeTruthy();
    expect(container.querySelector("[data-activity]")).toBeNull();
  });

  it("shows a command's status (commands never carry activity)", () => {
    const { container } = renderIndicator({ ...agent, kind: "Command", label: "web" });
    expect(container.querySelector("[data-status='Running']")).toBeTruthy();
  });

  it("renders an inline label in the header form", () => {
    renderIndicator(agent, "Permission", true);
    expect(screen.getByText("Permission")).toBeTruthy();
  });
});
