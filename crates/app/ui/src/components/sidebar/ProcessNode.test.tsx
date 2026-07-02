// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { ProcessNode } from "@/components/sidebar/ProcessNode";
import { TooltipProvider } from "@/components/ui/tooltip";
import { EMPTY_SIGNALS } from "@/store/signals";
import { SignalsContext } from "@/store/signalsContext";
import type { ProcessNode as Node } from "@/store/grouping";
import type { ToggleSet } from "@/store/useToggleSet";
import type { ProcessView } from "@/domain";

const noop = () => {};

function agent(id: number, label: string): ProcessView {
  return {
    id,
    project: 1,
    kind: "Agent",
    label,
    status: "Running",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
  };
}

const leadWithWorker: Node = {
  process: agent(1, "lead"),
  children: [{ process: agent(2, "worker"), children: [] }],
};

const expandedLeads: ToggleSet = { has: () => false, toggle: noop };

function renderNode(node: Node, collapsedLeads: ToggleSet = expandedLeads) {
  return render(
    <TooltipProvider>
      <SignalsContext value={EMPTY_SIGNALS}>
        <ProcessNode
          node={node}
          depth={0}
          treeColumn
          collapsedLeads={collapsedLeads}
          selectedId={null}
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

describe("ProcessNode", () => {
  it("nests a worker one level under its lead inside a group", () => {
    renderNode(leadWithWorker);
    const lead = screen.getByRole("treeitem", { name: /lead/ });
    expect(lead.getAttribute("aria-level")).toBe("1");
    expect(lead.getAttribute("aria-expanded")).toBe("true");
    const worker = screen.getByRole("treeitem", { name: /worker/ });
    expect(worker.getAttribute("aria-level")).toBe("2");
    expect(worker.closest("[role='group']")).toBeTruthy();
  });

  it("hides the workers of a collapsed lead", () => {
    renderNode(leadWithWorker, { has: (id) => id === 1, toggle: noop });
    expect(screen.getByRole("treeitem", { name: /lead/ }).getAttribute("aria-expanded")).toBe(
      "false",
    );
    expect(screen.queryByRole("treeitem", { name: /worker/ })).toBeNull();
  });

  it("renders a childless node as a plain row with no disclosure", () => {
    renderNode({ process: agent(3, "solo"), children: [] });
    expect(screen.getByRole("treeitem", { name: /solo/ }).getAttribute("aria-expanded")).toBeNull();
    expect(screen.queryByRole("button", { name: /workers/ })).toBeNull();
  });
});
