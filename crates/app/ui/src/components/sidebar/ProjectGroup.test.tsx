// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { ProjectGroup } from "@/components/sidebar/ProjectGroup";
import type { ProjectTree } from "@/store/projects";

const tree: ProjectTree = {
  project: { id: 1, name: "Storefront", root: "/p/storefront", icon: null },
  kinds: [],
  count: { running: 2, total: 4 },
};

const noop = () => {};

function renderGroup() {
  render(
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
      onTrust={noop}
      onStartAll={noop}
      onRestartRunning={noop}
      onStopAll={noop}
      onOpenProjectSettings={noop}
      onOpenOrchestration={noop}
      onRemoveProject={noop}
    />,
  );
}

afterEach(cleanup);

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
