// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ProjectGroup } from "@/components/sidebar/ProjectGroup";
import { SortableList } from "@/components/SortableList";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { ProcessActionHandlers } from "@/lib/processActions";
import type { ProjectTree } from "@/store/projects";

const tree: ProjectTree = {
  project: { id: 1, name: "Storefront", root: "/p/storefront", icon: null },
  kinds: [],
  count: { running: 2, total: 4 },
};

const noop = () => {};

const NOOP_HANDLERS: ProcessActionHandlers = {
  onTrust: noop,
  onResume: noop,
  onStart: noop,
  onStop: noop,
  onRestart: noop,
  onRemove: noop,
};

const groupProps = {
  tree,
  open: true,
  onOpenChange: noop,
  kindOpen: () => true,
  onKindOpenChange: noop,
  collapsedLeads: { has: () => false, toggle: noop },
  selectedId: null,
  onSelect: noop,
  handlers: NOOP_HANDLERS,
  onStartAll: noop,
  onRestartRunning: noop,
  onStopAll: noop,
  onOpenProjectSettings: noop,
  onOpenOrchestration: noop,
  onRemoveProject: noop,
};

interface ProjectActionOverrides {
  onStartAll?: () => void;
  onRestartRunning?: () => void;
  onStopAll?: () => void;
  onOpenOrchestration?: () => void;
  onOpenProjectSettings?: () => void;
  onRemoveProject?: () => void;
}

// A project header inside the arrangeable project list, which is what supplies its move actions —
// composed the way the sidebar does.
function renderGroup(
  ids: string[] = ["1"],
  overrides: ProjectActionOverrides = {},
  onReorder: (ids: string[]) => void = noop,
) {
  render(
    <TooltipProvider>
      <SortableList ids={ids} onReorder={onReorder}>
        <ProjectGroup
          tree={tree}
          open
          onOpenChange={noop}
          kindOpen={() => true}
          onKindOpenChange={noop}
          collapsedLeads={{ has: () => false, toggle: noop }}
          selectedId={null}
          onSelect={noop}
          handlers={NOOP_HANDLERS}
          onStartAll={overrides.onStartAll ?? noop}
          onRestartRunning={overrides.onRestartRunning ?? noop}
          onStopAll={overrides.onStopAll ?? noop}
          onOpenProjectSettings={overrides.onOpenProjectSettings ?? noop}
          onOpenOrchestration={overrides.onOpenOrchestration ?? noop}
          onRemoveProject={overrides.onRemoveProject ?? noop}
        />
      </SortableList>
    </TooltipProvider>,
  );
}

// The row that carries both the ••• menu and the right-click menu — the same element the
// drag handle and `ContextMenuTrigger` are bound to.
function projectRow(name: string): Element {
  const row = screen.getByText(name).closest("div[class*='group/project']");
  if (!row) throw new Error(`project row for ${name} not found`);
  return row;
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

describe("ProjectGroup right-click menu", () => {
  it("offers the same sections, in the same order, as the ••• menu", () => {
    // The middle of a three-item list, so both moves are on offer and the full order is visible.
    renderGroup(["0", "1", "2"]);
    fireEvent.contextMenu(projectRow("Storefront"));

    const items = screen.getAllByRole("menuitem").map((item) => item.textContent);
    expect(items).toEqual([
      "Start all",
      "Restart running",
      "Stop all",
      "Orchestration",
      "Project settings",
      "Move up",
      "Move down",
      "Remove project",
    ]);
  });

  it("withholds Move up for a project already leading the list", () => {
    renderGroup(["1", "2"]);
    fireEvent.contextMenu(projectRow("Storefront"));

    expect(screen.queryByRole("menuitem", { name: "Move up" })).toBeNull();
    expect(screen.getByRole("menuitem", { name: "Move down" })).toBeTruthy();
  });

  it("withholds Move down for a project already trailing the list", () => {
    renderGroup(["2", "1"]);
    fireEvent.contextMenu(projectRow("Storefront"));

    expect(screen.getByRole("menuitem", { name: "Move up" })).toBeTruthy();
    expect(screen.queryByRole("menuitem", { name: "Move down" })).toBeNull();
  });

  it("withholds both moves for a project alone in its list", () => {
    renderGroup(["1"]);
    fireEvent.contextMenu(projectRow("Storefront"));

    expect(screen.queryByRole("menuitem", { name: "Move up" })).toBeNull();
    expect(screen.queryByRole("menuitem", { name: "Move down" })).toBeNull();
  });

  it("marks Remove project destructive, unlike the routine actions around it", () => {
    renderGroup();
    fireEvent.contextMenu(projectRow("Storefront"));

    expect(
      screen.getByRole("menuitem", { name: "Remove project" }).getAttribute("data-variant"),
    ).toBe("destructive");
    expect(screen.getByRole("menuitem", { name: "Start all" }).getAttribute("data-variant")).toBe(
      "default",
    );
  });

  it("runs the bulk handler a selected item names", () => {
    const onStartAll = vi.fn();
    renderGroup(["1"], { onStartAll });
    fireEvent.contextMenu(projectRow("Storefront"));

    fireEvent.click(screen.getByRole("menuitem", { name: "Start all" }));
    expect(onStartAll).toHaveBeenCalledTimes(1);
  });

  it("runs the view handler a selected item names", () => {
    const onOpenProjectSettings = vi.fn();
    renderGroup(["1"], { onOpenProjectSettings });
    fireEvent.contextMenu(projectRow("Storefront"));

    fireEvent.click(screen.getByRole("menuitem", { name: "Project settings" }));
    expect(onOpenProjectSettings).toHaveBeenCalledTimes(1);
  });

  it("reorders the list when a selected move item names its direction", () => {
    const onReorder = vi.fn();
    renderGroup(["2", "1"], {}, onReorder);
    fireEvent.contextMenu(projectRow("Storefront"));

    fireEvent.click(screen.getByRole("menuitem", { name: "Move up" }));
    expect(onReorder).toHaveBeenCalledWith(["1", "2"]);
  });

  it("opens the removal confirmation rather than removing straight from the menu", () => {
    const onRemoveProject = vi.fn();
    renderGroup(["1"], { onRemoveProject });
    fireEvent.contextMenu(projectRow("Storefront"));

    fireEvent.click(screen.getByRole("menuitem", { name: "Remove project" }));
    expect(onRemoveProject).not.toHaveBeenCalled();
    expect(screen.getByRole("heading", { name: "Remove “Storefront”?" })).toBeTruthy();
  });
});

// A menuitem's accessible name says nothing about the separator beside it, so a menu can pass
// every item-order and item-content assertion above while still misplacing, dropping, or
// doubling a separator. These read the menu content's own children, in order, to pin that
// layout directly — the one thing an orphan separator (an empty section rendered anyway) or a
// swapped section would actually change that a menuitem query cannot see.
function openDropdown() {
  fireEvent.pointerDown(screen.getByRole("button", { name: "Actions for Storefront" }));
}

function menuContentSlots(menu: "dropdown" | "context"): (string | null)[] {
  const content = document.querySelector(`[data-slot="${menu}-menu-content"]`);
  if (!content) throw new Error(`${menu} menu content not found`);
  return Array.from(content.children).map((child) => child.getAttribute("data-slot"));
}

function expectedMenuSlots(menu: "dropdown" | "context", sectionCount: number): string[] {
  const slots = [`${menu}-menu-label`];
  for (let i = 0; i < sectionCount; i += 1) {
    slots.push(`${menu}-menu-separator`, `${menu}-menu-group`);
  }
  return slots;
}

const SEPARATOR_CASES = [
  {
    menu: "dropdown" as const,
    scenario: "both moves available",
    ids: ["0", "1", "2"],
    sectionCount: 4,
  },
  { menu: "dropdown" as const, scenario: "one move available", ids: ["1", "2"], sectionCount: 4 },
  { menu: "dropdown" as const, scenario: "no moves available", ids: ["1"], sectionCount: 3 },
  {
    menu: "context" as const,
    scenario: "both moves available",
    ids: ["0", "1", "2"],
    sectionCount: 4,
  },
  { menu: "context" as const, scenario: "one move available", ids: ["1", "2"], sectionCount: 4 },
  { menu: "context" as const, scenario: "no moves available", ids: ["1"], sectionCount: 3 },
];

describe("ProjectGroup menu section layout", () => {
  it.each(SEPARATOR_CASES)(
    "puts exactly one separator before every section in the $menu menu, $scenario",
    ({ menu, ids, sectionCount }) => {
      renderGroup(ids);
      if (menu === "dropdown") {
        openDropdown();
      } else {
        fireEvent.contextMenu(projectRow("Storefront"));
      }

      expect(menuContentSlots(menu)).toEqual(expectedMenuSlots(menu, sectionCount));
    },
  );
});
