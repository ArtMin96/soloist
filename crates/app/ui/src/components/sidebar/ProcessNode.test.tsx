// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { ProcessNode } from "@/components/sidebar/ProcessNode";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { ProcessActionHandlers } from "@/lib/processActions";
import { EMPTY_STORE } from "@/store/signalStore";
import { SignalsContext } from "@/store/signalsContext";
import type { ProcessNode as Node } from "@/store/grouping";
import type { ToggleSet } from "@/store/useToggleSet";
import type { ProcessView } from "@/domain";

const noop = () => {};

const NOOP_HANDLERS: ProcessActionHandlers = {
  onTrust: noop,
  onResume: noop,
  onStart: noop,
  onStop: noop,
  onRestart: noop,
  onRemove: noop,
};

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
      <SignalsContext value={EMPTY_STORE}>
        <ProcessNode
          node={node}
          depth={0}
          treeColumn
          collapsedLeads={collapsedLeads}
          selectedId={null}
          onSelect={noop}
          handlers={NOOP_HANDLERS}
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

describe("ProcessNode action targeting", () => {
  // Every row in the tree shares one `ProcessActionHandlers`, id-taking under the hood; a
  // swapped verb or a mis-targeted id in the binding between a node and its row would
  // type-check silently. This proves each row's control acts on that row's own process and
  // never its neighbor's.
  it("targets the activating row's own process, not the other row's", () => {
    const onStop = vi.fn();
    render(
      <TooltipProvider>
        <SignalsContext value={EMPTY_STORE}>
          <ProcessNode
            node={leadWithWorker}
            depth={0}
            treeColumn
            collapsedLeads={expandedLeads}
            selectedId={null}
            onSelect={noop}
            handlers={{ ...NOOP_HANDLERS, onStop }}
          />
        </SignalsContext>
      </TooltipProvider>,
    );

    within(screen.getByRole("treeitem", { name: /lead/ }))
      .getByLabelText("Stop")
      .click();
    expect(onStop).toHaveBeenCalledTimes(1);
    expect(onStop).toHaveBeenCalledWith(1);

    within(screen.getByRole("treeitem", { name: /worker/ }))
      .getByLabelText("Stop")
      .click();
    expect(onStop).toHaveBeenCalledTimes(2);
    expect(onStop).toHaveBeenLastCalledWith(2);
  });
});
