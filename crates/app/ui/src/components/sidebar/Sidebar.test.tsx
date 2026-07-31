// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { Sidebar } from "@/components/sidebar/Sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { DEFAULT_SIDEBAR } from "@/lib/sidebar";
import { HotkeysContext } from "@/store/hotkeysContext";
import { SidebarSettingsContext } from "@/store/sidebarSettingsContext";
import type { HotkeyBindingView, ProcessView, ProjectView } from "@/domain";
import type { Sidebar as SidebarSettings } from "@/domain";

const noop = () => {};

const PROJECT_A = { id: 1, name: "alpha", root: "/a", icon: null };
const PROJECT_B = { id: 2, name: "beta", root: "/b", icon: null };

const PROCESSES = [
  {
    id: 10,
    project: 1,
    kind: "Agent" as const,
    label: "claude",
    status: "Running" as const,
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated" as const,
  },
  {
    id: 11,
    project: 1,
    kind: "Command" as const,
    label: "build",
    status: "Stopped" as const,
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated" as const,
  },
  {
    id: 20,
    project: 2,
    kind: "Agent" as const,
    label: "worker",
    status: "Running" as const,
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated" as const,
  },
];

// A minimal sidebar-scope keymap for testing: only the rows under test.
function makeBindings(
  partial: Partial<Record<string, { key: string; ctrl?: boolean; alt?: boolean }>>,
) {
  return Object.entries(partial).map(
    ([action, binding]): HotkeyBindingView => ({
      action: action as HotkeyBindingView["action"],
      scope: "sidebar",
      binding: binding
        ? {
            ctrl: binding.ctrl ?? false,
            alt: binding.alt ?? false,
            shift: false,
            super: false,
            key: binding.key,
          }
        : null,
      is_default: true,
      conflict: false,
    }),
  );
}

const DEFAULT_BINDINGS = makeBindings({
  restart_selection: { key: "R" },
  next_project_group: { ctrl: true, key: "ArrowDown" },
  prev_project_group: { ctrl: true, key: "ArrowUp" },
  jump_to_agents: { alt: true, key: "A" },
  jump_to_commands: { alt: true, key: "C" },
  next_section: { alt: true, key: "ArrowDown" },
  prev_section: { alt: true, key: "ArrowUp" },
});

function renderSidebar(
  overrides: {
    settings?: SidebarSettings;
    projects?: ProjectView[];
    processes?: ProcessView[];
    selectedId?: number | null;
    onSelect?: (id: number) => void;
    onRestart?: (id: number) => void;
    onOpenStart?: () => void;
    bindings?: HotkeyBindingView[];
    lineage?: ReadonlyMap<number, number>;
    onReorderProjects?: (order: number[]) => void;
  } = {},
) {
  const {
    settings = DEFAULT_SIDEBAR,
    projects = [PROJECT_A, PROJECT_B],
    processes = PROCESSES,
    selectedId = null,
    onSelect = noop,
    onRestart = noop,
    onOpenStart = noop,
    bindings = DEFAULT_BINDINGS,
    lineage = new Map(),
    onReorderProjects = noop,
  } = overrides;
  render(
    <TooltipProvider>
      <HotkeysContext value={{ bindings, remap: noop, disable: noop, reset: noop, resetAll: noop }}>
        <SidebarSettingsContext value={{ sidebar: settings, setSidebar: noop }}>
          <Sidebar
            projects={projects}
            processes={processes}
            lineage={lineage}
            selectedId={selectedId}
            onSelect={onSelect}
            onStart={noop}
            onStop={noop}
            onRestart={onRestart}
            onResume={noop}
            onRemove={noop}
            onTrust={noop}
            onStartAll={noop}
            onRestartRunning={noop}
            onStopAll={noop}
            onOpenStart={onOpenStart}
            startActive={selectedId === null}
            onOpenSettings={noop}
            onOpenProjectSettings={noop}
            onOpenOrchestration={noop}
            onRemoveProject={noop}
            onReorderProjects={onReorderProjects}
          />
        </SidebarSettingsContext>
      </HotkeysContext>
    </TooltipProvider>,
  );
  return screen.getByRole("navigation");
}

afterEach(cleanup);

describe("Sidebar footer", () => {
  it("keeps Start available when the Settings footer preference is off", () => {
    const onOpenStart = vi.fn();
    renderSidebar({
      settings: { ...DEFAULT_SIDEBAR, show_settings_footer: false },
      onOpenStart,
    });

    const start = screen.getByRole("button", { name: "Start page" });
    expect(start.getAttribute("aria-current")).toBe("page");
    fireEvent.click(start);
    expect(onOpenStart).toHaveBeenCalledOnce();
    expect(screen.queryByRole("button", { name: "Settings" })).toBeNull();
  });

  it("shows the Settings footer button when the setting is on", () => {
    renderSidebar({ settings: { ...DEFAULT_SIDEBAR, show_settings_footer: true } });
    expect(screen.getByRole("button", { name: "Settings" })).toBeTruthy();
  });

  it("hides the Settings footer button when the setting is off", () => {
    renderSidebar({ settings: { ...DEFAULT_SIDEBAR, show_settings_footer: false } });
    expect(screen.queryByRole("button", { name: "Settings" })).toBeNull();
  });
});

describe("Sidebar filter", () => {
  it("narrows the tree to processes matching the query", () => {
    renderSidebar();
    // All three processes present before filtering.
    expect(screen.getByRole("treeitem", { name: /claude/ })).toBeTruthy();
    expect(screen.getByRole("treeitem", { name: /build/ })).toBeTruthy();
    expect(screen.getByRole("treeitem", { name: /worker/ })).toBeTruthy();

    fireEvent.change(screen.getByRole("searchbox", { name: "Filter processes" }), {
      target: { value: "claude" },
    });

    expect(screen.getByRole("treeitem", { name: /claude/ })).toBeTruthy();
    expect(screen.queryByRole("treeitem", { name: /build/ })).toBeNull();
    expect(screen.queryByRole("treeitem", { name: /worker/ })).toBeNull();
  });

  it("hides the filter input and shows every process when the setting is off", () => {
    renderSidebar({ settings: { ...DEFAULT_SIDEBAR, show_filter_input: false } });
    expect(screen.queryByRole("searchbox", { name: "Filter processes" })).toBeNull();
    expect(screen.getByRole("treeitem", { name: /claude/ })).toBeTruthy();
    expect(screen.getByRole("treeitem", { name: /worker/ })).toBeTruthy();
  });
});

describe("Sidebar lineage nesting", () => {
  const WORKER = {
    id: 12,
    project: 1,
    kind: "Agent" as const,
    label: "codex-worker",
    status: "Running" as const,
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated" as const,
  };

  const TERMINAL_LEAD = {
    id: 30,
    project: 1,
    kind: "Terminal" as const,
    label: "shell",
    status: "Running" as const,
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated" as const,
  };

  it("nests a spawned worker under its lead in the Agents group", () => {
    renderSidebar({
      projects: [PROJECT_A],
      processes: [...PROCESSES.filter((p) => p.project === 1), WORKER],
      lineage: new Map([[12, 10]]),
    });
    const lead = screen.getByRole("treeitem", { name: /claude/ });
    expect(lead.getAttribute("aria-expanded")).toBe("true");
    const worker = screen.getByRole("treeitem", { name: /codex-worker/ });
    expect(worker.getAttribute("aria-level")).toBe("2");
    // The Commands group carries no lineage, so its row stays a flat level-1 row.
    expect(screen.getByRole("treeitem", { name: /build/ }).getAttribute("aria-level")).toBe("1");
  });

  // A lead is always an agent, so a parent of another kind is data that should not exist. If it
  // ever arrives, every section must still say what it holds — the count a user reads and the rows
  // beneath it cannot disagree.
  it("shows an agent in its own section when its recorded lead is of another kind", () => {
    renderSidebar({
      projects: [PROJECT_A],
      processes: [TERMINAL_LEAD, WORKER],
      lineage: new Map([[12, 30]]),
    });
    const terminalRows = within(screen.getByRole("tree", { name: "Terminals" })).getAllByRole(
      "treeitem",
    );
    expect(terminalRows).toHaveLength(1);
    expect(terminalRows[0].textContent).toContain("shell");
    expect(screen.getByRole("button", { name: /^Terminals\s*1$/ })).toBeTruthy();
    const agents = screen.getByRole("tree", { name: "Agents" });
    expect(
      within(agents)
        .getByRole("treeitem", { name: /codex-worker/ })
        .getAttribute("aria-level"),
    ).toBe("1");
    expect(screen.getByRole("button", { name: /^Agents\s*1$/ })).toBeTruthy();
  });

  it("keeps the Agents section while hiding empty ones, when a lead is of another kind", () => {
    renderSidebar({
      settings: { ...DEFAULT_SIDEBAR, hide_empty_sections: true },
      projects: [PROJECT_A],
      processes: [TERMINAL_LEAD, WORKER],
      lineage: new Map([[12, 30]]),
    });
    expect(screen.getByRole("tree", { name: "Agents" })).toBeTruthy();
    expect(screen.getByRole("treeitem", { name: /codex-worker/ })).toBeTruthy();
  });

  it("jumps to an agent whose recorded lead is of another kind", () => {
    const onSelect = vi.fn();
    const nav = renderSidebar({
      projects: [PROJECT_A],
      processes: [TERMINAL_LEAD, WORKER],
      lineage: new Map([[12, 30]]),
      selectedId: 30,
      onSelect,
    });
    fireEvent.keyDown(nav, { key: "A", altKey: true });
    expect(onSelect).toHaveBeenCalledWith(12);
  });

  it("keeps every agent flat when no lineage exists", () => {
    renderSidebar();
    const agent = screen.getByRole("treeitem", { name: /claude/ });
    expect(agent.getAttribute("aria-level")).toBe("1");
    expect(agent.getAttribute("aria-expanded")).toBeNull();
  });
});

describe("Sidebar hotkeys", () => {
  it("keeps one selected tree row in the tab order", () => {
    renderSidebar({ selectedId: 10 });
    expect(screen.getByRole("treeitem", { name: /claude/ }).getAttribute("tabindex")).toBe("0");
    expect(screen.getByRole("treeitem", { name: /build/ }).getAttribute("tabindex")).toBe("-1");
  });

  it("moves through visible rows with an unmodified ArrowDown", () => {
    const onSelect = vi.fn();
    renderSidebar({ selectedId: 10, onSelect });
    const current = screen.getByRole("treeitem", { name: /claude/ });
    current.focus();
    fireEvent.keyDown(current, { key: "ArrowDown" });
    expect(onSelect).toHaveBeenCalledWith(11);
    expect(document.activeElement).toBe(screen.getByRole("treeitem", { name: /build/ }));
  });

  it("moves to the last visible row with End", () => {
    const onSelect = vi.fn();
    renderSidebar({ selectedId: 10, onSelect });
    const current = screen.getByRole("treeitem", { name: /claude/ });
    current.focus();
    fireEvent.keyDown(current, { key: "End" });
    expect(onSelect).toHaveBeenCalledWith(20);
  });

  it("restart_selection calls onRestart with the selected id", () => {
    const onRestart = vi.fn();
    const nav = renderSidebar({ selectedId: 10, onRestart });
    fireEvent.keyDown(nav, { key: "R" });
    expect(onRestart).toHaveBeenCalledWith(10);
  });

  it("restart_selection is a no-op when nothing is selected", () => {
    const onRestart = vi.fn();
    const nav = renderSidebar({ selectedId: null, onRestart });
    fireEvent.keyDown(nav, { key: "R" });
    expect(onRestart).not.toHaveBeenCalled();
  });

  it("next_project_group selects the first process of the next project", () => {
    const onSelect = vi.fn();
    const nav = renderSidebar({ selectedId: 10, onSelect });
    fireEvent.keyDown(nav, { key: "ArrowDown", ctrlKey: true });
    expect(onSelect).toHaveBeenCalledWith(20);
  });

  it("prev_project_group selects the first process of the previous project", () => {
    const onSelect = vi.fn();
    const nav = renderSidebar({ selectedId: 20, onSelect });
    fireEvent.keyDown(nav, { key: "ArrowUp", ctrlKey: true });
    expect(onSelect).toHaveBeenCalledWith(10);
  });

  it("jump_to_agents selects the first Agent in the current project", () => {
    const onSelect = vi.fn();
    const nav = renderSidebar({ selectedId: 11, onSelect }); // 11 is a Command in project 1
    fireEvent.keyDown(nav, { key: "A", altKey: true });
    expect(onSelect).toHaveBeenCalledWith(10); // 10 is the Agent in project 1
  });

  it("next_section advances from Agent to the next populated section", () => {
    const onSelect = vi.fn();
    const nav = renderSidebar({ selectedId: 10, onSelect }); // 10 is Agent in project 1
    fireEvent.keyDown(nav, { key: "ArrowDown", altKey: true });
    expect(onSelect).toHaveBeenCalledWith(11); // 11 is Command in project 1 (Agents → Commands)
  });

  it("does not fire when a hotkey has no binding (disabled)", () => {
    const onRestart = vi.fn();
    const bindings = makeBindings({ restart_selection: undefined });
    const nav = renderSidebar({ selectedId: 10, onRestart, bindings });
    fireEvent.keyDown(nav, { key: "R" });
    expect(onRestart).not.toHaveBeenCalled();
  });
});

describe("Sidebar project arrangement", () => {
  // Opening a Radix menu is a pointer-down, not a click.
  const openMenu = (project: string) =>
    fireEvent.pointerDown(screen.getByRole("button", { name: `Actions for ${project}` }));

  const projectRow = (name: string) =>
    screen.getByText(name).closest("div[class*='group/project']");

  it("makes the whole project line the drag handle, with no grip to find", () => {
    renderSidebar();

    // The row itself carries the drag, so there is no dead zone on it and no separate handle
    // competing with the project name for width.
    expect(projectRow("alpha")?.className).toContain("cursor-grab");
    expect(screen.queryByRole("button", { name: /drag/i })).toBeNull();
  });

  it("rearranges the list from the project menu, reporting the whole new order", () => {
    const onReorderProjects = vi.fn();
    renderSidebar({ onReorderProjects });

    openMenu("beta");
    fireEvent.click(screen.getByRole("menuitem", { name: "Move up" }));

    expect(onReorderProjects).toHaveBeenCalledWith([2, 1]);
  });

  it("withholds the move a project at an end of the list cannot make", () => {
    renderSidebar();

    openMenu("alpha");
    // alpha leads the list, so there is nowhere above it to go.
    expect(screen.queryByRole("menuitem", { name: "Move up" })).toBeNull();
    expect(screen.getByRole("menuitem", { name: "Move down" })).toBeTruthy();
  });

  it("does not offer to arrange a list a filter has narrowed", () => {
    renderSidebar({ settings: { ...DEFAULT_SIDEBAR, show_filter_input: true } });

    fireEvent.change(screen.getByLabelText("Filter processes"), { target: { value: "beta" } });

    // Only part of the list is on screen, so an order arranged from it would not be the user's
    // answer for the whole of it.
    expect(projectRow("beta")?.className).not.toContain("cursor-grab");
  });

  // The query leaves *two* projects on screen deliberately. With one, both moves are refused by
  // arithmetic alone and the guard is never actually asked — which is how a filtered move reached
  // the core unnoticed.
  it("withholds the move actions too while a filter narrows the list", () => {
    const onReorderProjects = vi.fn();
    renderSidebar({
      settings: { ...DEFAULT_SIDEBAR, show_filter_input: true },
      onReorderProjects,
    });

    fireEvent.change(screen.getByLabelText("Filter processes"), { target: { value: "e" } });
    expect(screen.getByText("alpha")).toBeTruthy();
    expect(screen.getByText("beta")).toBeTruthy();

    openMenu("alpha");
    expect(screen.queryByRole("menuitem", { name: "Move down" })).toBeNull();
    expect(screen.queryByRole("menuitem", { name: "Move up" })).toBeNull();
    expect(onReorderProjects).not.toHaveBeenCalled();
  });
});
