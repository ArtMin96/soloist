// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ProjectSettingsPane } from "@/components/project-settings/ProjectSettingsPane";
import type {
  ProcessSpec,
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
  auto_trust_command_changes: false,
  editor_override: null,
  notification_level: "all",
  command_notification_levels: {},
  local_commands: {},
};

const webCommand: ProjectCommandView = {
  name: "Web",
  command: "npm run dev",
  working_dir: null,
  auto_start: true,
  auto_restart: false,
  restart_when_changed: [],
  env: {},
  visibility: "shared",
  notification_level: null,
  effective_notification_level: "all",
  status: "Running",
};

const apiCommand: ProjectCommandView = {
  ...webCommand,
  name: "API",
  command: "npm run api",
  auto_start: false,
  status: "Stopped",
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

// One recorded IPC invocation: which core command fired and the payload it carried.
interface Call {
  cmd: string;
  payload?: Record<string, unknown>;
}

// Serve the page and echo every setter, recording every invoked command together with its payload
// so a test can assert either which command fired or exactly what it carried.
function mockPage(calls: Call[]) {
  mockIPC((cmd, payload) => {
    calls.push({ cmd, payload: payload as Record<string, unknown> | undefined });
    if (cmd === "project_settings_page") return page;
    if (cmd === "trust_grants") return [];
    return settings;
  });
}

const names = (calls: Call[]) => calls.map((c) => c.cmd);

// Serve the page as `mockPage` does, but reject the first `edit_shared_command` call — so a test
// can drive a write through to an observed failure before dispatching the next one.
function mockPageFailingFirstEdit(calls: Call[]) {
  let editAttempts = 0;
  mockIPC((cmd, payload) => {
    calls.push({ cmd, payload: payload as Record<string, unknown> | undefined });
    if (cmd === "project_settings_page") return page;
    if (cmd === "trust_grants") return [];
    if (cmd === "edit_shared_command" && ++editAttempts === 1) throw new Error("boom");
    return settings;
  });
}

function mockDuplicateRename() {
  let commands = [webCommand, apiCommand];
  let rejectRename: (error: Error) => void = () => undefined;
  const rename = new Promise<never>((_, reject) => {
    rejectRename = reject;
  });

  mockIPC((cmd, payload) => {
    if (cmd === "project_settings_page") return { ...page, commands };
    if (cmd === "trust_grants") return [];
    if (cmd === "rename_shared_command") return rename;
    if (cmd === "edit_shared_command") {
      const edit = payload as { name: string; spec: ProcessSpec };
      commands = commands.map((command) =>
        command.name === edit.name ? { ...command, ...edit.spec } : command,
      );
    }
    return settings;
  });

  return () => rejectRename(new Error("command API already exists"));
}

afterEach(() => {
  cleanup();
  clearMocks();
});

// Opens the Commands tab and expands the one seeded command's editor; returns its Command line,
// Name and "Start when the project opens" controls, ready for a test to drive.
async function openCommandEditor() {
  fireEvent.click(screen.getByRole("radio", { name: "Commands" }));
  fireEvent.click(await screen.findByText("Web"));
  return {
    commandInput: await screen.findByLabelText("Command"),
    nameInput: await screen.findByLabelText("Name"),
    autoStartToggle: await screen.findByLabelText("Start when the project opens"),
  };
}

describe("Per-project settings page", () => {
  it("renders the Overview and the command roster from the loaded page", async () => {
    const calls: Call[] = [];
    mockPage(calls);

    render(<ProjectSettingsPane project={project} />);

    await waitFor(() => expect(names(calls)).toContain("project_settings_page"));
    expect(await screen.findByText("Valid")).toBeTruthy();
    expect(screen.getByText(/2 running/)).toBeTruthy();
    expect(screen.getByText(/3 total/)).toBeTruthy();

    fireEvent.click(screen.getByRole("radio", { name: "Commands" }));
    expect(await screen.findByText("Web")).toBeTruthy();
  });

  it("persists an auto-start-gate toggle through the core command", async () => {
    const calls: Call[] = [];
    mockPage(calls);

    render(<ProjectSettingsPane project={project} />);
    await waitFor(() => expect(names(calls)).toContain("project_settings_page"));

    fireEvent.click(screen.getByRole("radio", { name: "Settings" }));
    fireEvent.click(await screen.findByLabelText("Suppress auto-start"));

    await waitFor(() => expect(names(calls)).toContain("set_project_auto_start_gate"));
  });

  it("persists the auto-trust-command-changes toggle through the core command", async () => {
    const calls: Call[] = [];
    mockPage(calls);

    render(<ProjectSettingsPane project={project} />);
    await waitFor(() => expect(names(calls)).toContain("project_settings_page"));

    fireEvent.click(screen.getByRole("radio", { name: "Settings" }));
    fireEvent.click(await screen.findByLabelText("Automatically trust command changes"));

    await waitFor(() => expect(names(calls)).toContain("set_project_auto_trust_command_changes"));
  });

  it("moves a shared command to local storage via make_command_local", async () => {
    const calls: Call[] = [];
    mockPage(calls);

    render(<ProjectSettingsPane project={project} />);
    await waitFor(() => expect(names(calls)).toContain("project_settings_page"));

    fireEvent.click(screen.getByRole("radio", { name: "Commands" }));
    fireEvent.click(await screen.findByText("Web"));
    fireEvent.click(await screen.findByRole("button", { name: "Make local" }));

    await waitFor(() => expect(names(calls)).toContain("make_command_local"));
  });

  // Regression coverage for the lost-update bug: a whole-record replace assembled from the
  // rendered (and by then stale) command dropped whatever an earlier, still in-flight edit had
  // just changed. Two edits fire before the first's `project_settings_page` reload lands; the
  // second write must still carry the first's change, not revert it.
  it("carries an earlier in-flight edit's change forward instead of reverting it", async () => {
    const calls: Call[] = [];
    mockPage(calls);

    render(<ProjectSettingsPane project={project} />);
    await waitFor(() => expect(names(calls)).toContain("project_settings_page"));

    const { commandInput, autoStartToggle } = await openCommandEditor();

    // Write #1: edit the command line. Its reload is never awaited before write #2 fires.
    fireEvent.change(commandInput, { target: { value: "npm run build" } });
    fireEvent.blur(commandInput);

    // Write #2: flip the auto-start toggle before write #1 resolves.
    fireEvent.click(autoStartToggle);

    const edits = calls.filter((c) => c.cmd === "edit_shared_command");
    expect(edits).toHaveLength(2);
    expect(edits[1].payload?.spec).toMatchObject({
      command: "npm run build",
      auto_start: false,
    });
  });

  // A write that settles unsuccessfully must not contribute its field to a later write's merge
  // base: `mutate` reloads only on success, so after this rejection `page` still holds the
  // pre-write value and the error is already on screen — carrying the rejected command line
  // forward would silently resurrect a change the error said did not apply.
  it("drops a failed edit's field from a later edit's merge base", async () => {
    const calls: Call[] = [];
    mockPageFailingFirstEdit(calls);

    render(<ProjectSettingsPane project={project} />);
    await waitFor(() => expect(names(calls)).toContain("project_settings_page"));

    const { commandInput, autoStartToggle } = await openCommandEditor();

    // Write #1: rejects. Wait for the failure to surface before the next write fires.
    fireEvent.change(commandInput, { target: { value: "npm run build" } });
    fireEvent.blur(commandInput);
    await screen.findByText(/boom/);

    // Write #2: a fresh edit on the same row, issued after write #1 has settled unsuccessfully.
    fireEvent.click(autoStartToggle);

    const edits = calls.filter((c) => c.cmd === "edit_shared_command");
    expect(edits).toHaveLength(2);
    expect(edits[1].payload?.spec).toMatchObject({
      command: webCommand.command,
      auto_start: false,
    });
  });

  it("keeps an overlapping edit away from the command that rejected a duplicate rename", async () => {
    const rejectRename = mockDuplicateRename();

    render(<ProjectSettingsPane project={project} />);
    await screen.findByText("Valid");

    const { nameInput, autoStartToggle } = await openCommandEditor();
    expect(screen.getByText(apiCommand.command)).toBeTruthy();
    fireEvent.change(nameInput, { target: { value: apiCommand.name } });
    fireEvent.blur(nameInput);
    fireEvent.click(autoStartToggle);
    rejectRename();

    await screen.findByText(/command API already exists/);
    await waitFor(() => expect(screen.queryByText("AUTO")).toBeNull());
    expect(screen.getByText(apiCommand.command)).toBeTruthy();
  });
});
