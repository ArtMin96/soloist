// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { OrchestrationTree } from "@/components/orchestration/OrchestrationTree";
import { TooltipProvider } from "@/components/ui/tooltip";
import { buildOrchestrationTree } from "@/store/orchestrationTree";
import type { AgentNode } from "@/domain";

function node(id: number, label: string, parent: number | null = null): AgentNode {
  return { id, parent, label, kind: "Agent", status: "Running", activity: "Working" };
}

function renderTree(agents: AgentNode[]) {
  return render(
    <TooltipProvider>
      <OrchestrationTree tree={buildOrchestrationTree(agents)} />
    </TooltipProvider>,
  );
}

afterEach(cleanup);

describe("OrchestrationTree", () => {
  it("teaches the empty case instead of rendering a blank panel", () => {
    renderTree([]);
    expect(screen.getByText(/no agents in this project yet/i)).toBeTruthy();
    expect(screen.queryByRole("tree")).toBeNull();
  });

  it("nests a worker under its lead, each as a treeitem at its own level", () => {
    renderTree([node(1, "lead"), node(2, "worker", 1)]);
    const items = screen.getAllByRole("treeitem");
    const lead = items.find((item) => item.textContent?.includes("lead"));
    const worker = items.find((item) => item.textContent?.includes("worker"));
    expect(lead?.getAttribute("aria-level")).toBe("1");
    expect(lead?.getAttribute("aria-expanded")).toBe("true");
    expect(worker?.getAttribute("aria-level")).toBe("2");
    // A childless worker exposes no expanded state.
    expect(worker?.getAttribute("aria-expanded")).toBeNull();
  });

  it("shows each row's kind and offers a disclosure only for a lead with workers", () => {
    renderTree([node(1, "lead"), node(2, "worker", 1), node(3, "solo")]);
    expect(screen.getAllByText("Agent").length).toBe(3);
    // Only the lead (which has a worker) gets a collapse control.
    expect(screen.getByRole("button", { name: /collapse lead's workers/i })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /solo/i })).toBeNull();
  });
});
