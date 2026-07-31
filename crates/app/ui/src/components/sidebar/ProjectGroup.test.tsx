// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ProjectGroup } from "@/components/sidebar/ProjectGroup";
import { SortableList } from "@/components/SortableList";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { ProjectTree } from "@/store/projects";

const tree: ProjectTree = {
  project: { id: 1, name: "Storefront", root: "/p/storefront", icon: null },
  kinds: [],
  count: { running: 2, total: 4 },
};

const noop = () => {};

const groupProps = {
  tree,
  open: true,
  onOpenChange: noop,
  kindOpen: () => true,
  onKindOpenChange: noop,
  collapsedLeads: { has: () => false, toggle: noop },
  selectedId: null,
  onSelect: noop,
  onStart: noop,
  onStop: noop,
  onRestart: noop,
  onResume: noop,
  onRemove: noop,
  onTrust: noop,
  onStartAll: noop,
  onRestartRunning: noop,
  onStopAll: noop,
  onOpenProjectSettings: noop,
  onOpenOrchestration: noop,
  onRemoveProject: noop,
};

// A project header inside the arrangeable project list, which is what supplies its move actions —
// composed the way the sidebar does.
function renderGroup(ids: string[] = ["1"]) {
  render(
    <TooltipProvider>
      <SortableList ids={ids} onReorder={noop}>
        <ProjectGroup
          tree={tree}
          open
          onOpenChange={noop}
          kindOpen={() => true}
          onKindOpenChange={noop}
          collapsedLeads={{ has: () => false, toggle: noop }}
          selectedId={null}
          onSelect={noop}
          onStart={noop}
          onStop={noop}
          onRestart={noop}
          onResume={noop}
          onRemove={noop}
          onTrust={noop}
          onStartAll={noop}
          onRestartRunning={noop}
          onStopAll={noop}
          onOpenProjectSettings={noop}
          onOpenOrchestration={noop}
          onRemoveProject={noop}
        />
      </SortableList>
    </TooltipProvider>,
  );
}

afterEach(cleanup);

describe("ProjectGroup outside a list", () => {
  // The design harness renders a project row on its own to look at it. A row is a presentational
  // component, so standing it up must not require the list it usually sits in — it simply has
  // nowhere to move to.
  it("renders on its own, offering no move it has no list to make", () => {
    expect(() =>
      render(
        <TooltipProvider>
          <ProjectGroup {...groupProps} />
        </TooltipProvider>,
      ),
    ).not.toThrow();

    expect(screen.getByText("Storefront")).toBeTruthy();
    fireEvent.pointerDown(screen.getByRole("button", { name: "Actions for Storefront" }));
    expect(screen.queryByRole("menuitem", { name: "Move up" })).toBeNull();
    expect(screen.queryByRole("menuitem", { name: "Move down" })).toBeNull();
    expect(screen.getByRole("menuitem", { name: "Start all" })).toBeTruthy();
  });
});

describe("ProjectGroup header", () => {
  it("keeps the project name visible and collapses every action into one menu", () => {
    renderGroup();
    expect(screen.getByText("Storefront")).toBeTruthy();
    // The fix: a single ••• actions affordance in the header, not a row of inline controls
    // that crush the truncating name. The bulk controls now live only inside the menu.
    expect(screen.getByRole("button", { name: "Actions for Storefront" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Start all" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Restart running" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Stop all" })).toBeNull();
  });

  it("shows the running count for the project", () => {
    renderGroup();
    expect(screen.getByLabelText("2 of 4 processes running").textContent).toBe("2/4");
  });
});
