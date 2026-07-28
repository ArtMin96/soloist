// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ProjectSettingsPane } from "@/components/project-settings/ProjectSettingsPane";
import type {
  NotificationLevel,
  ProjectCommandView,
  ProjectSettings,
  ProjectSettingsPage,
  ProjectView,
} from "@/domain";

const project: ProjectView = {
  id: 1,
  name: "storefront",
  root: "/home/dev/storefront",
  icon: null,
};

function command(name: string, overrides: Partial<ProjectCommandView> = {}): ProjectCommandView {
  return {
    name,
    command: "npm run dev",
    working_dir: null,
    auto_start: true,
    auto_restart: false,
    restart_when_changed: [],
    env: {},
    visibility: "shared",
    notification_level: null,
    effective_notification_level: "all",
    status: null,
    ...overrides,
  };
}

function pageOf(settings: ProjectSettings, commands: ProjectCommandView[]): ProjectSettingsPage {
  return {
    project: 1,
    root: "/home/dev/storefront",
    config: { valid: true, error: null },
    running: 0,
    total: commands.length,
    settings,
    resolved_editor: "code",
    commands,
  };
}

// A stand-in for the core's stored settings: the setters write into it and every page read is
// projected from what it currently holds, so a control can be observed rendering what was actually
// stored rather than what it locally believes.
//
// The project stays at `all` here, the one level where nothing has to be resolved: an overridden
// command is exactly its override, and one that inherits is exactly the project. So the stand-in
// only ever echoes what it was given — it never decides which of two levels wins when a command
// asks to be louder than its project. That is the core's rule, asserted in
// `the_settings_page_resolves_each_commands_effective_level`, and the clamped case is served below
// as a fixed page with no round trip.
function coreStandIn() {
  let level: NotificationLevel = "all";
  const overrides = new Map<string, NotificationLevel>();

  const settings = (): ProjectSettings => ({
    auto_start_gate: false,
    auto_trust_command_changes: false,
    editor_override: null,
    notification_level: level,
    command_notification_levels: Object.fromEntries(overrides),
    local_commands: {},
  });

  mockIPC((cmd, args) => {
    if (cmd === "set_project_notification_level") {
      level = (args as { level: NotificationLevel }).level;
    }
    if (cmd === "set_command_notification_level") {
      const { command: name, level: next } = args as {
        command: string;
        level: NotificationLevel | null;
      };
      if (next === null) overrides.delete(name);
      else overrides.set(name, next);
    }
    if (cmd === "project_settings_page") {
      return pageOf(settings(), [
        command("Web", {
          notification_level: overrides.get("Web") ?? null,
          effective_notification_level: overrides.get("Web") ?? level,
        }),
      ]);
    }
    return settings();
  });

  return {
    /** What the stand-in currently holds for a command: its level, or null when it has none. */
    stored: (name: string): NotificationLevel | null => overrides.get(name) ?? null,
    /** Whether an entry exists at all — an absent key is what "inherit" is stored as. */
    isOverridden: (name: string) => overrides.has(name),
  };
}

/** Renders the pane and waits for the first page load, then opens `tab`. */
async function openTab(tab: string) {
  render(<ProjectSettingsPane project={project} />);
  fireEvent.click(await screen.findByRole("radio", { name: tab }));
}

/** Opens the Commands tab and expands `name`'s editor. */
async function openCommand(name: string) {
  await openTab("Commands");
  fireEvent.click(await screen.findByText(name));
}

/** The named option, asserted to be the chosen one exactly as it reports itself to assistive tech. */
const chosen = (name: string) => screen.getByRole("radio", { name, checked: true });
/** The named option, asserted to be offered but not chosen. */
const offered = (name: string) => screen.getByRole("radio", { name, checked: false });

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("The project notification level", () => {
  it("shows the stored level as the chosen option", async () => {
    mockIPC((cmd) => {
      const settings: ProjectSettings = {
        auto_start_gate: false,
        auto_trust_command_changes: false,
        editor_override: null,
        notification_level: "important",
        command_notification_levels: {},
        local_commands: {},
      };
      return cmd === "project_settings_page" ? pageOf(settings, []) : settings;
    });

    await openTab("Notifications");

    expect(
      await screen.findByRole("radio", { name: "Important only", checked: true }),
    ).toBeTruthy();
    expect(offered("All")).toBeTruthy();
    expect(offered("None")).toBeTruthy();
  });

  it("states what each level admits, so 'Important only' cannot read as more alerts", async () => {
    mockIPC((cmd) => {
      const settings: ProjectSettings = {
        auto_start_gate: false,
        auto_trust_command_changes: false,
        editor_override: null,
        notification_level: "all",
        command_notification_levels: {},
        local_commands: {},
      };
      return cmd === "project_settings_page" ? pageOf(settings, []) : settings;
    });

    await openTab("Notifications");

    // Every option's description is on screen at rest — the user compares them without opening
    // anything — and the narrower level names what it drops rather than only what it keeps.
    expect(
      await screen.findByText(/terminal bells, and notifications a script sends/i),
    ).toBeTruthy();
    expect(screen.getByText(/terminal bells and script notifications are dropped/i)).toBeTruthy();
    expect(screen.getByText(/nothing at all, not even a crash/i)).toBeTruthy();
  });

  it("persists a change and reflects the stored level back", async () => {
    coreStandIn();

    await openTab("Notifications");
    fireEvent.click(await screen.findByRole("radio", { name: "None" }));

    await waitFor(() => expect(chosen("None")).toBeTruthy());
    expect(offered("All")).toBeTruthy();
  });
});

describe("A command's notification level", () => {
  it("stores inheriting and silence as different things, each rendering back as itself", async () => {
    const core = coreStandIn();

    await openCommand("Web");
    // Silence is an explicit, stored choice.
    fireEvent.click(await screen.findByRole("radio", { name: "None" }));

    await waitFor(() => expect(core.stored("Web")).toBe("none"));
    expect(chosen("None")).toBeTruthy();
    expect(offered("Same as project")).toBeTruthy();

    // Inheriting is the absence of a choice, not a third spelling of silence.
    fireEvent.click(offered("Same as project"));

    await waitFor(() => expect(core.isOverridden("Web")).toBe(false));
    expect(core.stored("Web")).toBeNull();
    expect(chosen("Same as project")).toBeTruthy();
    expect(offered("None")).toBeTruthy();
  });

  it("says the project holds a louder override down, rather than showing the override alone", async () => {
    const settings: ProjectSettings = {
      auto_start_gate: false,
      auto_trust_command_changes: false,
      editor_override: null,
      notification_level: "important",
      command_notification_levels: { Web: "all" },
      local_commands: {},
    };
    const page = pageOf(settings, [
      command("Web", {
        notification_level: "all",
        effective_notification_level: "important",
      }),
    ]);
    mockIPC((cmd) => (cmd === "project_settings_page" ? page : settings));

    await openCommand("Web");

    // The control still holds the override the user set, so touching it edits what is stored...
    expect(await screen.findByRole("radio", { name: "All", checked: true })).toBeTruthy();
    // ...while the resolved level the core handed down is stated outright.
    expect(screen.getByText(/holds this command to Important only/i)).toBeTruthy();
  });

  it("says nothing about a clamp when the project is not holding the command down", async () => {
    const settings: ProjectSettings = {
      auto_start_gate: false,
      auto_trust_command_changes: false,
      editor_override: null,
      notification_level: "all",
      command_notification_levels: { Web: "none" },
      local_commands: {},
    };
    const page = pageOf(settings, [
      command("Web", {
        notification_level: "none",
        effective_notification_level: "none",
      }),
    ]);
    mockIPC((cmd) => (cmd === "project_settings_page" ? page : settings));

    await openCommand("Web");

    expect(await screen.findByRole("radio", { name: "None", checked: true })).toBeTruthy();
    expect(screen.queryByText(/holds this command to/i)).toBeNull();
  });
});
