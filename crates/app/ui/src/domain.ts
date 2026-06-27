// Domain shapes mirrored from the Rust core (serde-serialized). This is the single
// source of truth for these types on the TypeScript side — components and the store
// import from here rather than re-declaring statuses or event shapes. Field names match
// the core's serde output (snake_case where the Rust field is snake_case).

export type ProcessKind = "Command" | "Agent" | "Terminal";

export type ProcStatus =
  | "Stopped"
  | "Starting"
  | "Running"
  | "Crashed"
  | "Restarting"
  | "Stopping"
  | "RestartExhausted";

// Port-readiness gate (mirrors core::Readiness): "Ungated" = no gate active, "Waiting" = a
// wait_for_port is in effect and the awaited port has not bound (Running but not Ready),
// "Ready" = the awaited port bound.
export type Readiness = "Ungated" | "Waiting" | "Ready";

export interface ProcessView {
  id: number;
  project: number;
  kind: ProcessKind;
  label: string;
  status: ProcStatus;
  exit_code: number | null;
  // True for a trust-gated command whose variant is not yet trusted; the UI blocks its
  // start and offers a trust affordance.
  requires_trust: boolean;
  // TCP ports the process is currently listening on (discovered while it runs, cleared when
  // it stops). Empty until discovery finds any.
  ports: number[];
  // Port-readiness gate; "Ungated" until a wait_for_port is in effect, and reset to
  // "Ungated" when the process stops.
  ready: Readiness;
}

// Rendered output snapshot (escape sequences applied to plain text) — the Logs source.
export interface RenderedScreen {
  lines: string[];
}

// One rendered line of terminal output.
export interface LogLine {
  text: string;
}

// A `solo.yml` sync diff, carried by ConfigChanged (consumed by the trust/sync dialog).
export interface Rename {
  from: string;
  to: string;
}

export interface ConfigSync {
  added: string[];
  updated: string[];
  removed: string[];
  renamed: Rename[];
}

// A leftover process group awaiting a Kill / Kill All / Leave decision.
export interface OrphanInfo {
  name: string;
  command: string;
  pgid: number;
}

// One command awaiting trust after a config change — enough detail to review what it
// runs (command, working dir, env) before trusting it. Carried by ConfigChanged.
export interface TrustReviewCommand {
  name: string;
  command: string;
  working_dir: string | null;
  env: Record<string, string>;
}

// The outcome of opening a project (the `project_load` command). `processes` is how many
// the folder's solo.yml declared; `created` is true when Soloist auto-created the solo.yml
// from detected commands (the folder had none). The UI turns these facts into a notice so
// opening a project is never silent.
export interface ProjectLoad {
  id: number;
  processes: number;
  created: boolean;
}

// A project's display identity (the `project_list` query): its durable id, resolved name
// (solo.yml `name:` or folder), root, and a ready-to-render icon — a `data:` URL the backend
// loaded from the project's `icon:` (null when none), so name and icon arrive as one shape
// with no separate icon request. The sidebar groups the process tree by project using this.
export interface ProjectView {
  id: number;
  name: string;
  root: string;
  icon: string | null;
}

// The agent CLI providers Soloist knows out of the box (mirrors core::AgentKind), plus
// "Generic" for any other CLI the user configures.
export type AgentKind =
  | "Claude"
  | "Codex"
  | "Amp"
  | "Gemini"
  | "OpenCode"
  | "Copilot"
  | "Kimi"
  | "Generic";

// How a Generic tool receives its prompt (mirrors core::PromptMode); ignored for built-in
// providers, which follow their own conventions.
export type PromptMode = "Stdin" | "AppendedArg";

// A configured agent tool (mirrors core::AgentTool): a launchable CLI, the args appended on
// every launch, and its prompt convention. `name` is the unique key and display label.
export interface AgentTool {
  name: string;
  command: string;
  default_args: string[];
  kind: AgentKind;
  prompt_mode: PromptMode;
}

// A configured tool paired with whether its CLI appears installed (mirrors core::DetectedTool)
// — the result of `--version` auto-detection. Tools outside the probe set (Copilot, Kimi,
// Generic) always report installed: false. The picker badges installed tools as launchable.
export interface DetectedTool {
  tool: AgentTool;
  installed: boolean;
}

// The five-state agent activity (mirrors core::agents::AgentActivity), derived from an agent's
// terminal output by a per-provider heuristic. It answers "busy or available?" (Working/Thinking
// vs Idle) and "does it need a human?" (Permission/Error, the attention states).
export type AgentActivity = "Idle" | "Permission" | "Thinking" | "Working" | "Error";

// Mirrors the core's `DomainEvent` (serde `tag = "type"`).
export type DomainEvent =
  | {
      type: "ProcessSpawned";
      id: number;
      project: number;
      kind: ProcessKind;
      label: string;
      status: ProcStatus;
      requires_trust: boolean;
    }
  | {
      type: "ProcessStatusChanged";
      id: number;
      from: ProcStatus;
      to: ProcStatus;
      exit_code: number | null;
    }
  | { type: "ProcessRemoved"; id: number }
  // A process's display label changed (display-only; trust and identity are unaffected).
  | { type: "ProcessRenamed"; id: number; label: string }
  // A periodic CPU/memory reading for a running process, sampled across its whole group.
  // cpu_pct is normalised to the whole machine (100 = every core busy, never above); rss is
  // the group's memory in bytes, shared pages counted once. Emitted ~1 Hz; consumers
  // coalesce it (never a per-tick re-render).
  | { type: "MetricsTick"; id: number; cpu_pct: number; rss: number }
  // A process's set of bound (listening) TCP ports changed — discovered while it runs,
  // emptied when it stops. The new sorted set is carried so the read model updates without
  // a snapshot round-trip; also reflected on ProcessView.ports.
  | { type: "PortsChanged"; id: number; ports: number[] }
  // A process's readiness changed while a port wait is active: false = Running but the
  // awaited port has not bound yet, true = it bound. Reflected on ProcessView.ready.
  | { type: "ReadyStateChanged"; id: number; ready: boolean }
  // The restart policy is relaunching a crashed auto_restart command; `attempt` is its
  // position in the rate-limit window (the status also moves Crashed -> Starting).
  | { type: "RestartScheduled"; id: number; attempt: number }
  // The restart policy gave up after too many restarts in the window; the command is held
  // in RestartExhausted until the user restarts it.
  | { type: "RestartExhausted"; id: number }
  // A command was restarted because a watched file changed; the status also cycles through
  // the usual restart deltas, so this is the discrete signal for a file-watch banner.
  | { type: "FileRestart"; id: number }
  // A project was opened/changed. The UI re-reads the rendered project snapshot on this
  // (which carries each project's loaded icon); it doesn't consume the event's domain fields.
  | { type: "ProjectOpened"; id: number }
  | {
      type: "ConfigChanged";
      project: number;
      diff: ConfigSync;
      requires_trust: boolean;
      commands: TrustReviewCommand[];
    }
  | { type: "TerminalTitleChanged"; id: number; title: string }
  | { type: "TerminalBell"; id: number }
  // An agent's activity changed (the five-state idle FSM). Edge-triggered (only on a
  // transition), so the agent's row updates without polling; Permission/Error raise attention.
  | { type: "AgentActivityChanged"; id: number; state: AgentActivity }
  | { type: "OrphansFound"; orphans: OrphanInfo[] };

export interface AppInfo {
  name: string;
  version: string;
}

// ── Settings (mirrors core::settings) ───────────────────────────────────────
// The durable global preference document, one sub-document per Settings tab. Enum string
// values are the core's serde `snake_case` output; discrete pickers are closed enums (never
// bare strings/numbers) so the valid set is the single source of truth, mapped to a concrete
// CSS/xterm value in one `lib/` place on the frontend.

// The app color scheme (mirrors core::Theme). "system" follows the OS light/dark preference.
export type Theme = "light" | "dark" | "system";

// A discrete text-size step for the interface/terminal size pickers (mirrors core::FontScale).
export type FontScale = "extra_small" | "small" | "medium" | "large" | "extra_large";

// A terminal font weight — the CSS 100–900 steps (mirrors core::FontWeight).
export type FontWeight =
  | "w100"
  | "w200"
  | "w300"
  | "w400"
  | "w500"
  | "w600"
  | "w700"
  | "w800"
  | "w900";

// Terminal line height — vertical spacing between rows (mirrors core::LineHeight).
export type LineHeight = "compact" | "default" | "comfortable" | "spacious";

// Terminal letter spacing — horizontal spacing between characters (mirrors core::LetterSpacing).
export type LetterSpacing = "tight" | "default" | "wide" | "wider";

// Terminal typography (mirrors core::TerminalAppearance) — the xterm.js renderer is restyled
// from these. `font_family` is null to use the app default.
export interface TerminalAppearance {
  focus_on_click: boolean;
  font_family: string | null;
  font_weight: FontWeight;
  bold_font_weight: FontWeight;
  font_scale: FontScale;
  line_height: LineHeight;
  letter_spacing: LetterSpacing;
}

// The Appearance tab document (mirrors core::Appearance).
export interface Appearance {
  theme: Theme;
  interface_font_scale: FontScale;
  terminal: TerminalAppearance;
}

// When a project header shows its CPU/memory badge (mirrors the core Sidebar threshold enums).
// The option sets differ between project and process headers, so each is its own closed enum.
export type ProjectCpuThreshold = "always" | "pct25" | "pct50" | "pct100" | "pct200" | "never";
export type ProjectMemThreshold = "always" | "mb500" | "gb1" | "gb2" | "gb8" | "never";
export type ProcessCpuThreshold = "always" | "pct10" | "pct30" | "pct60" | "pct90" | "never";
export type ProcessMemThreshold = "always" | "mb100" | "mb500" | "gb1" | "gb2" | "never";

// The Sidebar tab document (mirrors core::Sidebar) — what the process-tree sidebar shows.
export interface Sidebar {
  show_filter_input: boolean;
  hide_empty_sections: boolean;
  project_cpu_threshold: ProjectCpuThreshold;
  project_mem_threshold: ProjectMemThreshold;
  project_open_in_editor: boolean;
  project_open_in_terminal: boolean;
  project_reveal_in_file_manager: boolean;
  process_cpu_threshold: ProcessCpuThreshold;
  process_mem_threshold: ProcessMemThreshold;
  show_settings_footer: boolean;
}

// The context a hotkey is active in (mirrors core::HotkeyScope). Bindings only conflict within
// the same scope.
export type HotkeyScope = "general" | "sidebar" | "terminal";

// A named, remappable action (mirrors core::HotkeyAction). The closed set is the single source
// the settings panel and the keyboard handler iterate.
export type HotkeyAction =
  | "open_command_palette"
  | "quick_actions"
  | "quick_jump"
  | "new_agent_or_terminal"
  | "open_settings"
  | "open_terminal_search"
  | "close_agent_or_terminal"
  | "next_project_group"
  | "prev_project_group"
  | "next_section"
  | "prev_section"
  | "jump_to_agents"
  | "jump_to_commands"
  | "jump_to_terminals"
  | "collapse_or_section"
  | "jump_to_parent_project"
  | "expand_project"
  | "restart_selection"
  | "previous_process"
  | "next_process"
  | "increase_terminal_font_size"
  | "decrease_terminal_font_size";

// A key chord (mirrors core::Binding): the modifier flags plus the main key (a
// `KeyboardEvent.key` token, e.g. "K", "ArrowDown", "="). `super` is the core's `super_key`.
export interface Binding {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  super: boolean;
  key: string;
}

// One action's effective state in the keymap read model (mirrors core::HotkeyBindingView).
// `binding` is null when the action is disabled; `is_default` is true when no override is set;
// `conflict` is true when the binding collides with another action in the same scope.
export interface HotkeyBindingView {
  action: HotkeyAction;
  scope: HotkeyScope;
  binding: Binding | null;
  is_default: boolean;
  conflict: boolean;
}

// The Agents tab document (mirrors core::AgentSettings) — the auto-summarization opt-in only
// (OFF by default: both null). The agent tool registry itself is the Phase-7 surface.
export interface AgentSettings {
  summarizer_tool: string | null;
  summarizer_model: string | null;
}

// The Tools tab document (mirrors core::ToolDefaults) — the default editor and terminal launch
// names (null = use the system default).
export interface ToolDefaults {
  default_editor: string | null;
  default_terminal: string | null;
}

// The Integrations tab document (mirrors core::Integrations) — the two master integration
// toggles. The per-group MCP enablement is `McpToolGroups`.
export interface Integrations {
  mcp_enabled: boolean;
  http_api_enabled: boolean;
}

// A toggleable MCP feature-tool group (mirrors core::McpFeatureGroup). Core groups are always
// served and are not represented here.
export type McpFeatureGroup = "scratchpads" | "todos" | "timers" | "key_value";

// Which MCP feature-tool groups the server exposes (mirrors core::McpToolGroups). Scratchpads,
// Todos and Timers default on; Key-Value defaults off.
export interface McpToolGroups {
  scratchpads: boolean;
  todos: boolean;
  timers: boolean;
  key_value: boolean;
}
