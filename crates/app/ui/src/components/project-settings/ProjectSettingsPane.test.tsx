// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ProjectSettingsPane } from "@/components/project-settings/ProjectSettingsPane";
import type {
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

const settings: ProjectSettings = {
  auto_start_gate: false,
  editor_override: null,
  crash_exit_alerts: true,
  terminal_alerts: true,
  command_terminal_alerts: {},
  local_commands: {},
};

const webCommand: ProjectCommandView = {
  name: "Web",
  command: "npm run dev",
  working_dir: null,
  auto_start: true,
  auto_restart: false,
  restart_when_changed: [],
  visibility: "shared",
  terminal_alerts: true,
  status: "Running",
};

const page: ProjectSettingsPage = {
  project: 1,
  root: "/home/dev/storefront",
  config: { valid: true, error: null },
  running: 2,
  total: 3,
  settings,
  resolved_editor: "code",
  commands: [webCommand],
};

// Serve the page and echo every setter, recording each invoked command for assertions.
function mockPage(calls: string[]) {
  mockIPC((cmd) => {
    calls.push(cmd);
    if (cmd === "project_settings_page") return page;
    return settings;
  });
}

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("Per-project settings page", () => {
  it("renders the Overview and the command roster from the loaded page", async () => {
    const calls: string[] = [];
    mockPage(calls);

    render(<ProjectSettingsPane project={project} />);

    await waitFor(() => expect(calls).toContain("project_settings_page"));
    expect(await screen.findByText("Valid")).toBeTruthy();
    expect(screen.getByText(/2 running/)).toBeTruthy();
    expect(screen.getByText(/3 total/)).toBeTruthy();

    fireEvent.click(screen.getByRole("tab", { name: "Commands" }));
    expect(await screen.findByText("Web")).toBeTruthy();
  });

  it("persists an auto-start-gate toggle through the core command", async () => {
    const calls: string[] = [];
    mockPage(calls);

    render(<ProjectSettingsPane project={project} />);
    await waitFor(() => expect(calls).toContain("project_settings_page"));

    fireEvent.click(screen.getByRole("tab", { name: "Settings" }));
    fireEvent.click(await screen.findByLabelText("Suppress auto-start"));

    await waitFor(() => expect(calls).toContain("set_project_auto_start_gate"));
  });

  it("moves a shared command to local storage via make_command_local", async () => {
    const calls: string[] = [];
    mockPage(calls);

    render(<ProjectSettingsPane project={project} />);
    await waitFor(() => expect(calls).toContain("project_settings_page"));

    fireEvent.click(screen.getByRole("tab", { name: "Commands" }));
    fireEvent.click(await screen.findByText("Web"));
    fireEvent.click(await screen.findByRole("button", { name: "Make local" }));

    await waitFor(() => expect(calls).toContain("make_command_local"));
  });
});
