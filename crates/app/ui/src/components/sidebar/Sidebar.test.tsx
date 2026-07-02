// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Sidebar } from "@/components/sidebar/Sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { DEFAULT_SIDEBAR } from "@/lib/sidebar";
import { HotkeysContext } from "@/store/hotkeysContext";
import { SidebarSettingsContext } from "@/store/sidebarSettingsContext";
import type { HotkeyBindingView } from "@/domain";
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
    selectedId?: number | null;
    onSelect?: (id: number) => void;
    onRestart?: (id: number) => void;
    bindings?: HotkeyBindingView[];
    lineage?: ReadonlyMap<number, number>;
  } = {},
) {
  const {
    settings = DEFAULT_SIDEBAR,
    selectedId = null,
    onSelect = noop,
    onRestart = noop,
    bindings = DEFAULT_BINDINGS,
    lineage = new Map(),
  } = overrides;
  render(
    <TooltipProvider>
      <HotkeysContext value={{ bindings, remap: noop, disable: noop, reset: noop, resetAll: noop }}>
        <SidebarSettingsContext value={{ sidebar: settings, setSidebar: noop }}>
          <Sidebar
            projects={[PROJECT_A, PROJECT_B]}
            processes={PROCESSES}
            lineage={lineage}
            selectedId={selectedId}
            onSelect={onSelect}
            onStart={noop}
            onStop={noop}
            onRestart={onRestart}
            onResume={noop}
            onTrust={noop}
            onStartAll={noop}
            onRestartRunning={noop}
            onStopAll={noop}
            onOpenSettings={noop}
            onOpenProjectSettings={noop}
            onOpenOrchestration={noop}
          />
        </SidebarSettingsContext>
      </HotkeysContext>
    </TooltipProvider>,
  );
  return screen.getByRole("navigation");
}

afterEach(cleanup);

describe("Sidebar footer", () => {
  it("shows the Settings footer button when the setting is on", () => {
    renderSidebar({ settings: { ...DEFAULT_SIDEBAR, show_settings_footer: true } });
    expect(screen.getByRole("button", { name: "Settings" })).toBeTruthy();
  });

  it("hides the Settings footer button when the setting is off", () => {
    renderSidebar({ settings: { ...DEFAULT_SIDEBAR, show_settings_footer: false } });
    expect(screen.queryByRole("button", { name: "Settings" })).toBeNull();
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

  it("nests a spawned worker under its lead in the Agents group", () => {
    render(
      <TooltipProvider>
        <HotkeysContext
          value={{
            bindings: DEFAULT_BINDINGS,
            remap: noop,
            disable: noop,
            reset: noop,
            resetAll: noop,
          }}
        >
          <SidebarSettingsContext value={{ sidebar: DEFAULT_SIDEBAR, setSidebar: noop }}>
            <Sidebar
              projects={[PROJECT_A]}
              processes={[...PROCESSES.filter((p) => p.project === 1), WORKER]}
              lineage={new Map([[12, 10]])}
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
              onOpenSettings={noop}
              onOpenProjectSettings={noop}
              onOpenOrchestration={noop}
            />
          </SidebarSettingsContext>
        </HotkeysContext>
      </TooltipProvider>,
    );
    const lead = screen.getByRole("treeitem", { name: /claude/ });
    expect(lead.getAttribute("aria-expanded")).toBe("true");
    const worker = screen.getByRole("treeitem", { name: /codex-worker/ });
    expect(worker.getAttribute("aria-level")).toBe("2");
    // The Commands group carries no lineage, so its row stays a flat level-1 row.
    expect(screen.getByRole("treeitem", { name: /build/ }).getAttribute("aria-level")).toBe("1");
  });

  it("keeps every agent flat when no lineage exists", () => {
    renderSidebar();
    const agent = screen.getByRole("treeitem", { name: /claude/ });
    expect(agent.getAttribute("aria-level")).toBe("1");
    expect(agent.getAttribute("aria-expanded")).toBeNull();
  });
});

describe("Sidebar hotkeys", () => {
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
