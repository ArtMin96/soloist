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

// Serves the page and applies each `edit_shared_command` call to a local `commands` list, so a
// test can assert what the pane ends up rendering rather than what it sent. The first edit always
// rejects; later ones apply normally.
function mockFailFirstEditThenApply() {
  let commands = [webCommand];
  let editAttempts = 0;
  mockIPC((cmd, payload) => {
    if (cmd === "project_settings_page") return { ...page, commands };
    if (cmd === "trust_grants") return [];
    if (cmd === "edit_shared_command") {
      editAttempts += 1;
      if (editAttempts === 1) throw new Error("boom");
      const edit = payload as { name: string; spec: ProcessSpec };
      commands = commands.map((c) => (c.name === edit.name ? { ...c, ...edit.spec } : c));
    }
    return settings;
  });
}

// Serves the page and applies each `edit_shared_command` call to a local `commands` list. Web's
// edit is held open until the test releases it, so the test controls exactly when it settles.
function mockGatedFirstEdit() {
  let commands = [webCommand];
  let editCalls = 0;
  let release: (() => void) | null = null;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });

  mockIPC((cmd, payload) => {
    if (cmd === "project_settings_page") return { ...page, commands };
    if (cmd === "trust_grants") return [];
    if (cmd === "edit_shared_command") {
      editCalls += 1;
      const edit = payload as { name: string; spec: ProcessSpec };
      const apply = () => {
        commands = commands.map((c) => (c.name === edit.name ? { ...c, ...edit.spec } : c));
      };
      if (editCalls === 1) return gate.then(apply);
      apply();
    }
    return settings;
  });

  return { release: () => release?.(), editCalls: () => editCalls };
}

// Serves a page with two commands and applies each `edit_shared_command` call to a local
// `commands` list. Each command's edit is held open until the test releases it, and any
// `project_settings_page` read taken before Web's edit has landed is held open too, so the test
// fully controls both the write order and exactly when that stale snapshot is delivered relative
// to one that already reflects Web's change.
function mockOutOfOrderCommandEdits() {
  let commands = [webCommand, apiCommand];
  const editGates = new Map<string, { release: () => void; promise: Promise<void> }>();
  for (const name of ["Web", "API"]) {
    let release: (() => void) | null = null;
    const promise = new Promise<void>((resolve) => {
      release = resolve;
    });
    editGates.set(name, { release: () => release?.(), promise });
  }
  let releaseStaleRead: (() => void) | null = null;
  const staleReadGate = new Promise<void>((resolve) => {
    releaseStaleRead = resolve;
  });
  let firstRead = true;

  mockIPC((cmd, payload) => {
    if (cmd === "trust_grants") return [];
    if (cmd === "project_settings_page") {
      if (firstRead) {
        firstRead = false;
        return { ...page, commands };
      }
      const snapshot = { ...page, commands };
      const webLanded = commands.find((c) => c.name === "Web")?.command !== webCommand.command;
      return webLanded ? snapshot : staleReadGate.then(() => snapshot);
    }
    if (cmd === "edit_shared_command") {
      const edit = payload as { name: string; spec: ProcessSpec };
      const gate = editGates.get(edit.name);
      return gate?.promise.then(() => {
        commands = commands.map((c) => (c.name === edit.name ? { ...c, ...edit.spec } : c));
      });
    }
    return settings;
  });

  return {
    releaseWebEdit: () => editGates.get("Web")?.release(),
    releaseApiEdit: () => editGates.get("API")?.release(),
    releaseStaleRead: () => releaseStaleRead?.(),
  };
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

  // Regression coverage for the lost-update bug, now at the write level: writes to the same
  // command must never be in flight at once. Write #2 must not even reach the wire until write #1
  // settles, and once it does, it must carry both patches merged.
  it("sends the merged edit only after the first settles, never concurrently, for the same command", async () => {
    const { release, editCalls } = mockGatedFirstEdit();

    render(<ProjectSettingsPane project={project} />);
    const { commandInput, autoStartToggle } = await openCommandEditor();

    // Write #1: edit the command line. Held open by the mock until released below.
    fireEvent.change(commandInput, { target: { value: "npm run build" } });
    fireEvent.blur(commandInput);

    // Write #2: flip auto-start before write #1 settles. Must not go out yet.
    fireEvent.click(autoStartToggle);
    expect(editCalls()).toBe(1);

    release();

    await waitFor(() => expect(editCalls()).toBe(2));
    await waitFor(() => expect(screen.getByText("npm run build")).toBeTruthy());
    expect(screen.queryByText("AUTO")).toBeNull();
  });

  // Regression coverage for the lost-update bug across commands: firing writes for two different
  // commands concurrently lets whichever `project_settings_page` read landed last win, even when
  // it read the page before an earlier command's write had durably applied. Serializing every
  // write removes the possibility entirely — there is only ever one in-flight write, and only one
  // reload, so a stale snapshot can never be captured in the first place.
  it("keeps a stale reload from clobbering an earlier command's already-applied change", async () => {
    const { releaseWebEdit, releaseApiEdit, releaseStaleRead } = mockOutOfOrderCommandEdits();

    render(<ProjectSettingsPane project={project} />);
    fireEvent.click(screen.getByRole("radio", { name: "Commands" }));

    // Write #1: edit Web's command line. Held open until the test releases it.
    fireEvent.click(await screen.findByText("Web"));
    const commandInput = await screen.findByLabelText("Command");
    fireEvent.change(commandInput, { target: { value: "npm run build" } });
    fireEvent.blur(commandInput);

    // Write #2: switch to a different command and flip its toggle. Also held open.
    fireEvent.click(await screen.findByText("API"));
    fireEvent.click(await screen.findByLabelText("Start when the project opens"));

    // Let write #2 settle well before write #1. Its own reload, if it fires this early, reads the
    // page while write #1's change is still outstanding.
    releaseApiEdit();
    await new Promise((resolve) => setTimeout(resolve, 0));

    // Now let write #1 finally settle; its own reload lands promptly and reflects both changes.
    releaseWebEdit();
    await waitFor(() => expect(screen.getByText("npm run build")).toBeTruthy());

    // A read taken before write #1 landed arrives now, after the correct one already did.
    releaseStaleRead();

    // Web already carries its own AUTO badge from the fixture; API's is what proves write #2 also
    // survived.
    await waitFor(() => expect(screen.getAllByText("AUTO")).toHaveLength(2));
    expect(screen.getByText("npm run build")).toBeTruthy();
  });

  // The queue drains before the page reloads: two writes queued back to back must produce exactly
  // one reload, not one per write, which is also what removes the out-of-order-reload window.
  it("reloads once after the whole queue drains, not once per queued write", async () => {
    const calls: Call[] = [];
    mockPage(calls);
    const reads = () => names(calls).filter((n) => n === "project_settings_page").length;

    render(<ProjectSettingsPane project={project} />);
    await waitFor(() => expect(reads()).toBe(1));
    const readsAfterLoad = reads();

    fireEvent.click(screen.getByRole("radio", { name: "Settings" }));
    const autoStartSwitch = await screen.findByLabelText("Suppress auto-start");
    const autoTrustSwitch = await screen.findByLabelText("Automatically trust command changes");

    // Both writes fire back to back, with no await between them, so the second is still queued
    // (not yet sent) behind the first when it is issued.
    fireEvent.click(autoStartSwitch);
    fireEvent.click(autoTrustSwitch);

    await waitFor(() => expect(names(calls)).toContain("set_project_auto_trust_command_changes"));
    await waitFor(() => expect(reads()).toBeGreaterThan(readsAfterLoad));
    // Give a wrongly per-write reload a chance to land before pinning the final count.
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(reads()).toBe(readsAfterLoad + 1);
  });

  // A write that settles unsuccessfully must not contribute its field to a later write's merge
  // base, and the queue must still drain normally afterwards.
  it("drops a failed edit's field from a later edit's merge base", async () => {
    mockFailFirstEditThenApply();

    render(<ProjectSettingsPane project={project} />);
    const { commandInput, autoStartToggle } = await openCommandEditor();

    // Write #1: rejects. Wait for the failure to surface before the next write fires.
    fireEvent.change(commandInput, { target: { value: "npm run build" } });
    fireEvent.blur(commandInput);
    await screen.findByText(/boom/);

    // Write #2: a fresh edit on the same row, issued after write #1 has settled unsuccessfully.
    fireEvent.click(autoStartToggle);

    await waitFor(() => expect(screen.queryByText("AUTO")).toBeNull());
    expect(screen.getByText(webCommand.command)).toBeTruthy();
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
