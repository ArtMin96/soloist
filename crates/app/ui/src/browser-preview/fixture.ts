import { Channel, type InvokeArgs } from "@tauri-apps/api/core";
import type {
  AgentSignal,
  AppInfo,
  AttentionSnapshot,
  GitStatus,
  HotkeyBindingView,
  LineageEdge,
  ProcessView,
  ProjectFile,
  ProjectView,
} from "@/domain";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { DEFAULT_SIDEBAR } from "@/lib/sidebar";
import { PTY_FRAME_RESYNC } from "@/api";

const COMMAND = {
  agentActivity: "agent_activity",
  appInfo: "app_info",
  appearance: "appearance",
  attentionSnapshot: "attention_snapshot",
  gitFiles: "git_files",
  gitStatus: "git_status",
  hotkeys: "hotkeys",
  lineageEdges: "lineage_edges",
  procList: "proc_list",
  projectList: "project_list",
  ptyAttach: "pty_attach",
  ptyDetach: "pty_detach",
  ptyResize: "pty_resize",
  ptyWrite: "pty_write",
  setPresence: "set_presence",
  sidebarSettings: "sidebar_settings",
} as const;

const PLUGIN_COMMAND = {
  resourceClose: "plugin:resources|close",
  storeClear: "plugin:store|clear",
  storeDelete: "plugin:store|delete",
  storeEntries: "plugin:store|entries",
  storeGet: "plugin:store|get",
  storeHas: "plugin:store|has",
  storeKeys: "plugin:store|keys",
  storeLength: "plugin:store|length",
  storeLoad: "plugin:store|load",
  storeReload: "plugin:store|reload",
  storeReset: "plugin:store|reset",
  storeSave: "plugin:store|save",
  storeSet: "plugin:store|set",
  storeValues: "plugin:store|values",
  windowFocused: "plugin:window|is_focused",
  windowMaximized: "plugin:window|is_maximized",
} as const;

const PLUGIN_COMMAND_PREFIX = "plugin:";
const WEBVIEW_COMMAND_PREFIX = "plugin:webview|";
const WINDOW_COMMAND_PREFIX = "plugin:window|";

export const PREVIEW_IDS = {
  storefront: 101,
  docs: 202,
  web: 1001,
  agent: 1002,
  terminal: 1003,
  tests: 1004,
  deploy: 1005,
  docsServer: 2001,
  reviewer: 2002,
} as const;

export const PREVIEW_PTY_MAX_BYTES = 128;

const PREVIEW_APP_INFO = {
  name: "Soloist",
  version: "browser preview",
} satisfies AppInfo;

export const PREVIEW_PROJECTS = [
  {
    id: PREVIEW_IDS.storefront,
    name: "Storefront Preview",
    root: "/preview/soloist-browser-preview-fixture/storefront",
    icon: null,
  },
  {
    id: PREVIEW_IDS.docs,
    name: "Soloist Docs",
    root: "/preview/soloist-browser-preview-fixture/docs",
    icon: null,
  },
] satisfies ProjectView[];

export const PREVIEW_PROCESSES = [
  {
    id: PREVIEW_IDS.web,
    project: PREVIEW_IDS.storefront,
    kind: "Command",
    label: "Web server",
    status: "Running",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [1420],
    ready: "Ready",
  },
  {
    id: PREVIEW_IDS.agent,
    project: PREVIEW_IDS.storefront,
    kind: "Agent",
    label: "Implementation agent",
    status: "Running",
    exit_code: null,
    requires_trust: false,
    resumable: true,
    ports: [],
    ready: "Ungated",
  },
  {
    id: PREVIEW_IDS.terminal,
    project: PREVIEW_IDS.storefront,
    kind: "Terminal",
    label: "Workspace shell",
    status: "Running",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
  },
  {
    id: PREVIEW_IDS.tests,
    project: PREVIEW_IDS.storefront,
    kind: "Command",
    label: "Unit tests",
    status: "Crashed",
    exit_code: 1,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
  },
  {
    id: PREVIEW_IDS.deploy,
    project: PREVIEW_IDS.storefront,
    kind: "Command",
    label: "Deploy preview",
    status: "Stopped",
    exit_code: null,
    requires_trust: true,
    resumable: false,
    ports: [],
    ready: "Ungated",
  },
  {
    id: PREVIEW_IDS.docsServer,
    project: PREVIEW_IDS.docs,
    kind: "Command",
    label: "Documentation",
    status: "Running",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Waiting",
  },
  {
    id: PREVIEW_IDS.reviewer,
    project: PREVIEW_IDS.docs,
    kind: "Agent",
    label: "Review agent",
    status: "Running",
    exit_code: null,
    requires_trust: false,
    resumable: true,
    ports: [],
    ready: "Ungated",
  },
] satisfies ProcessView[];

const PREVIEW_LINEAGE = [] satisfies LineageEdge[];

const PREVIEW_AGENT_ACTIVITY = [
  { id: PREVIEW_IDS.agent, activity: "Working" },
  { id: PREVIEW_IDS.reviewer, activity: "Permission" },
] satisfies AgentSignal[];

const PREVIEW_ATTENTION = {
  processes: [
    { process: PREVIEW_IDS.tests, kind: "crashed", alerts: 1 },
    { process: PREVIEW_IDS.reviewer, kind: "agent_permission", alerts: 1 },
  ],
  total: 2,
} satisfies AttentionSnapshot;

const PREVIEW_HOTKEYS = [] satisfies HotkeyBindingView[];

const PREVIEW_GIT_STATUS = {
  branch: {
    name: "feature/browser-preview",
    upstream: "origin/feature/browser-preview",
    sync: { state: "ahead", ahead: 2 },
  },
  changes: [
    {
      path: "src/components/Dashboard.tsx",
      status: { staged: null, unstaged: "modified" },
      original_path: null,
    },
    {
      path: "src/components/StatusBadge.tsx",
      status: { staged: "added", unstaged: null },
      original_path: null,
    },
  ],
  merging: false,
  capabilities: {
    pull: false,
    push: true,
    stash: true,
    discardablePaths: ["src/components/Dashboard.tsx"],
  },
  changeCounts: { added: 1, removed: 0 },
  lineCounts: { additions: 42, deletions: 9, complete: true },
} satisfies GitStatus;

const PREVIEW_GIT_FILES = [
  { path: "src/components/Dashboard.tsx", ignored: false },
  { path: "src/components/StatusBadge.tsx", ignored: false },
] satisfies ProjectFile[];

const PREVIEW_PTY_TEXT = [
  "$ pnpm dev",
  "Soloist browser preview fixture ready",
  "Local: http://localhost:1420/",
  "[web] GET /api/projects 200 12ms · [agent] inspecting the dashboard read model",
  "",
].join("\r\n");

const STORE_RESOURCE_ID = 1;
const PTY_ATTACHMENT_TOKEN = 1;

function recordPayload(payload?: InvokeArgs): Record<string, unknown> | null {
  if (
    !payload ||
    Array.isArray(payload) ||
    payload instanceof ArrayBuffer ||
    ArrayBuffer.isView(payload)
  ) {
    return null;
  }
  return payload;
}

function projectFrom(payload?: InvokeArgs): number | null {
  const project = recordPayload(payload)?.project;
  return typeof project === "number" ? project : null;
}

function sendPtyResync(payload?: InvokeArgs): void {
  const channel = recordPayload(payload)?.onChunk;
  if (!(channel instanceof Channel)) {
    throw new Error("Browser preview received an invalid PTY attachment");
  }
  const replay = new TextEncoder().encode(PREVIEW_PTY_TEXT).subarray(0, PREVIEW_PTY_MAX_BYTES - 1);
  const frame = new Uint8Array(replay.byteLength + 1);
  frame[0] = PTY_FRAME_RESYNC;
  frame.set(replay, 1);
  channel.onmessage(frame.buffer as ArrayBuffer);
}

function handlePluginCommand(command: string): unknown {
  switch (command) {
    case PLUGIN_COMMAND.windowFocused:
      return true;
    case PLUGIN_COMMAND.windowMaximized:
      return false;
    case PLUGIN_COMMAND.storeLoad:
      return STORE_RESOURCE_ID;
    case PLUGIN_COMMAND.storeGet:
      return [null, false];
    case PLUGIN_COMMAND.storeHas:
      return false;
    case PLUGIN_COMMAND.storeKeys:
    case PLUGIN_COMMAND.storeValues:
    case PLUGIN_COMMAND.storeEntries:
      return [];
    case PLUGIN_COMMAND.storeLength:
      return 0;
    case PLUGIN_COMMAND.storeSet:
    case PLUGIN_COMMAND.storeDelete:
    case PLUGIN_COMMAND.storeClear:
    case PLUGIN_COMMAND.storeReset:
    case PLUGIN_COMMAND.storeReload:
    case PLUGIN_COMMAND.storeSave:
    case PLUGIN_COMMAND.resourceClose:
      return undefined;
    default:
      if (command.startsWith(WINDOW_COMMAND_PREFIX) || command.startsWith(WEBVIEW_COMMAND_PREFIX)) {
        return undefined;
      }
      throw new Error(`Browser preview does not support "${command}"`);
  }
}

/** Creates the static development adapter consumed by Tauri's official IPC mock. */
export function createBrowserPreviewDispatcher(): (
  command: string,
  payload?: InvokeArgs,
) => unknown {
  return (command, payload) => {
    if (command.startsWith(PLUGIN_COMMAND_PREFIX)) return handlePluginCommand(command);

    switch (command) {
      case COMMAND.appInfo:
        return PREVIEW_APP_INFO;
      case COMMAND.procList:
        return PREVIEW_PROCESSES;
      case COMMAND.projectList:
        return PREVIEW_PROJECTS;
      case COMMAND.appearance:
        return DEFAULT_APPEARANCE;
      case COMMAND.sidebarSettings:
        return DEFAULT_SIDEBAR;
      case COMMAND.hotkeys:
        return PREVIEW_HOTKEYS;
      case COMMAND.lineageEdges:
        return PREVIEW_LINEAGE;
      case COMMAND.agentActivity:
        return PREVIEW_AGENT_ACTIVITY;
      case COMMAND.attentionSnapshot:
        return PREVIEW_ATTENTION;
      case COMMAND.setPresence:
      case COMMAND.ptyWrite:
      case COMMAND.ptyResize:
      case COMMAND.ptyDetach:
        return undefined;
      case COMMAND.gitStatus:
        return projectFrom(payload) === PREVIEW_IDS.storefront ? PREVIEW_GIT_STATUS : null;
      case COMMAND.gitFiles:
        return projectFrom(payload) === PREVIEW_IDS.storefront ? PREVIEW_GIT_FILES : null;
      case COMMAND.ptyAttach:
        sendPtyResync(payload);
        return PTY_ATTACHMENT_TOKEN;
      default:
        throw new Error(`Browser preview does not support "${command}"`);
    }
  };
}
