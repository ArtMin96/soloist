// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  addSharedCommand,
  createTheme,
  duplicateTheme,
  importTheme,
  inspectTheme,
  makeCommandLocal,
  orchestrationSnapshot,
  projectSettingsPage,
  removeTheme,
  selectTheme,
  setGlassOpacity,
  setProjectAutoStartGate,
  updateTheme,
} from "@/api";
import type {
  Appearance,
  OrchestrationSnapshot,
  ProcessSpec,
  ProjectSettings,
  ProjectSettingsPage,
  TrustReviewCommand,
  ThemeFile,
} from "@/domain";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { BUILT_IN_THEMES } from "@/theme/catalog";

afterEach(() => {
  clearMocks();
});

const SETTINGS: ProjectSettings = {
  auto_start_gate: false,
  auto_trust_command_changes: false,
  editor_override: null,
  notification_level: "all",
  command_notification_levels: {},
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
          env: {},
          visibility: "shared",
          notification_level: null,
          effective_notification_level: "all",
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
      { name: "Api", variant_hash: "api-v1", command: "cargo run", working_dir: null, env: {} },
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

describe("api — orchestration read-model wrapper", () => {
  it("orchestrationSnapshot invokes orchestration_snapshot with the project id and returns the lineage snapshot", async () => {
    const snapshot: OrchestrationSnapshot = {
      project: 4,
      agents: [
        {
          id: 1,
          parent: null,
          label: "lead",
          kind: "Agent",
          status: "Running",
          activity: "Working",
        },
        { id: 2, parent: 1, label: "worker", kind: "Agent", status: "Running", activity: "Idle" },
      ],
      todos: [],
      timers: [],
      leases: [],
      scratchpads: [],
      diagrams: [],
      kv: [],
    };
    const seen = captureIpc("orchestration_snapshot", snapshot);

    const result = await orchestrationSnapshot(4);

    expect(seen).toEqual({ cmd: "orchestration_snapshot", args: { project: 4 } });
    expect(result.agents.find((node) => node.id === 2)?.parent).toBe(1);
  });
});

describe("api — task-shaped theme commands", () => {
  it("uses the registered command names and camel-case invoke arguments", async () => {
    const theme = { ...BUILT_IN_THEMES[1] } as Partial<(typeof BUILT_IN_THEMES)[number]>;
    delete theme.source;
    const file = theme as ThemeFile;
    const appearance: Appearance = { ...DEFAULT_APPEARANCE };
    const seen: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      seen.push({ cmd, args });
      return cmd === "inspect_theme" ? file : appearance;
    });

    await selectTheme("dark", file.id);
    await createTheme(file);
    await updateTheme(file);
    await importTheme("{}", "keep_both");
    await inspectTheme("{}");
    await duplicateTheme(file.id);
    await removeTheme(file.id);
    await setGlassOpacity(80);

    expect(seen).toEqual([
      { cmd: "select_theme", args: { appearance: "dark", themeId: file.id } },
      { cmd: "create_theme", args: { theme: file } },
      { cmd: "update_theme", args: { theme: file } },
      { cmd: "import_theme", args: { themeJson: "{}", conflict: "keep_both" } },
      { cmd: "inspect_theme", args: { themeJson: "{}" } },
      { cmd: "duplicate_theme", args: { themeId: file.id } },
      { cmd: "remove_theme", args: { themeId: file.id } },
      { cmd: "set_glass_opacity", args: { opacity: 80 } },
    ]);
  });
});
