// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  addSharedCommand,
  makeCommandLocal,
  projectSettingsPage,
  setProjectAutoStartGate,
} from "@/api";
import type {
  ProcessSpec,
  ProjectSettings,
  ProjectSettingsPage,
  TrustReviewCommand,
} from "@/domain";

afterEach(() => {
  clearMocks();
});

const SETTINGS: ProjectSettings = {
  auto_start_gate: false,
  auto_trust_command_changes: false,
  editor_override: null,
  crash_exit_alerts: true,
  terminal_alerts: true,
  command_terminal_alerts: {},
  local_commands: {},
};

// Records the single IPC call the wrapper makes, and returns `reply` for `command`.
function captureIpc(command: string, reply: unknown) {
  const seen: { cmd: string; args: unknown } = { cmd: "", args: undefined };
  mockIPC((cmd, args) => {
    seen.cmd = cmd;
    seen.args = args;
    return cmd === command ? reply : undefined;
  });
  return seen;
}

describe("api — per-project settings wrappers", () => {
  it("projectSettingsPage invokes project_settings_page with the project id and returns the page", async () => {
    const page: ProjectSettingsPage = {
      project: 7,
      root: "/work/storefront",
      config: { valid: true, error: null },
      running: 1,
      total: 2,
      settings: SETTINGS,
      resolved_editor: "code",
      commands: [
        {
          name: "Web",
          command: "npm run dev",
          working_dir: null,
          auto_start: true,
          auto_restart: false,
          restart_when_changed: [],
          visibility: "shared",
          terminal_alerts: true,
          status: "Running",
        },
      ],
    };
    const seen = captureIpc("project_settings_page", page);

    const result = await projectSettingsPage(7);

    expect(seen).toEqual({ cmd: "project_settings_page", args: { project: 7 } });
    expect(result).toEqual(page);
    expect(result.commands[0].visibility).toBe("shared");
  });

  it("setProjectAutoStartGate invokes set_project_auto_start_gate with project and engaged", async () => {
    const updated: ProjectSettings = { ...SETTINGS, auto_start_gate: true };
    const seen = captureIpc("set_project_auto_start_gate", updated);

    const result = await setProjectAutoStartGate(3, true);

    expect(seen).toEqual({
      cmd: "set_project_auto_start_gate",
      args: { project: 3, engaged: true },
    });
    expect(result.auto_start_gate).toBe(true);
  });

  it("addSharedCommand invokes add_shared_command with project, name and spec and returns the trust list", async () => {
    const spec: ProcessSpec = { command: "cargo run" };
    const pending: TrustReviewCommand[] = [
      { name: "Api", command: "cargo run", working_dir: null, env: {} },
    ];
    const seen = captureIpc("add_shared_command", pending);

    const result = await addSharedCommand(1, "Api", spec);

    expect(seen).toEqual({ cmd: "add_shared_command", args: { project: 1, name: "Api", spec } });
    expect(result).toEqual(pending);
  });

  it("makeCommandLocal invokes make_command_local with project and name and returns the updated settings", async () => {
    const updated: ProjectSettings = {
      ...SETTINGS,
      local_commands: { Api: { command: "cargo run" } },
    };
    const seen = captureIpc("make_command_local", updated);

    const result = await makeCommandLocal(1, "Api");

    expect(seen).toEqual({ cmd: "make_command_local", args: { project: 1, name: "Api" } });
    expect(result.local_commands.Api.command).toBe("cargo run");
  });
});
