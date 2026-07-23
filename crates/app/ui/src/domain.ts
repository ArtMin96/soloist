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

// The kind of document a template seeds (mirrors core::TemplateKind, serde snake_case). One
// unified template aggregate serves all three.
export type TemplateKind = "prompt" | "scratchpad" | "todo";

// Which scope a template lives in (mirrors core::TemplateScope). The Settings manager edits the
// global library; the project scope is reached over MCP.
export type TemplateScope = "global" | "project";

// A template in a listing: identity, kind, handle, one-line description, the {{placeholders}} the
// core derives from the body, scope, and revision (the guard the next edit carries).
export interface TemplateSummary {
  id: number;
  kind: TemplateKind;
  name: string;
  description: string | null;
  placeholders: string[];
  scope: TemplateScope;
  revision: number;
}

// A template as the manager reads it to edit: the full Markdown body plus everything in a summary.
export interface TemplateView {
  id: number;
  kind: TemplateKind;
  name: string;
  description: string | null;
  body: string;
  placeholders: string[];
  scope: TemplateScope;
  revision: number;
}

// A prompt template rendered with a caller's values (mirrors core::coordination::RenderedPrompt).
// Both reports are advisory: `text` is usable either way, and they name what did not line up —
// `unfilled` the declared placeholders left without a value (their `{{token}}` stays literal in the
// text), `unknown` the supplied names the body declares no placeholder for.
export interface RenderedPrompt {
  text: string;
  unfilled: string[];
  unknown: string[];
}

// The selected default template per seedable kind (mirrors core::settings::TemplateDefaults).
// Global-only in v1; `null` means "seed an empty document". Prompt has no seed default.
export interface TemplateDefaults {
  scratchpad: number | null;
  todo: number | null;
}

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
  // True for an agent whose provider supports "Resume last session"; when it rests, the UI
  // offers resuming the most recent conversation alongside starting fresh. Always false for
  // commands, terminals, and agents whose provider has no documented resume.
  resumable: boolean;
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
  variant_hash: string;
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

// What `--version` auto-detection learned about one agent CLI (mirrors core::Detection).
// "Missing" and "Unknown" are distinct on purpose: the first is a fact about the machine (the
// probe ran, the CLI is not here), the second means the probe reached no answer — it timed out,
// could not run, or the provider is outside the probe set and is never checked. Rendering an
// unanswered probe as "not found" is what let a broken probe look like an empty toolchain.
export type Detection = "Installed" | "Missing" | "Unknown";

// A configured tool paired with what auto-detection found (mirrors core::DetectedTool). Tools
// outside the probe set (Copilot, Kimi, Generic) always report "Unknown". The picker badges
// installed tools as launchable.
export interface DetectedTool {
  tool: AgentTool;
  detection: Detection;
}

// The five-state agent activity (mirrors core::agents::AgentActivity), derived from an agent's
// terminal output by a per-provider heuristic. It answers "busy or available?" (Working/Thinking
// vs Idle) and "does it need a human?" (Permission/Error, the attention states).
export type AgentActivity = "Idle" | "Permission" | "Thinking" | "Working" | "Error";

// One tracked agent's current idle activity (mirrors core::orchestration::AgentSignal). The
// snapshot the signal store seeds its idle badges from — only agents classified at least once —
// so a webview reload or a dropped `AgentActivityChanged` recovers the true state.
export interface AgentSignal {
  id: number;
  activity: AgentActivity;
}

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
      resumable: boolean;
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
  // position in the rate-limit window and `limit` is what that window allows before the
  // command is held exhausted (the status also moves Crashed -> Starting).
  | { type: "RestartScheduled"; id: number; attempt: number; limit: number }
  // The restart policy gave up after too many restarts in the window; the command is held
  // in RestartExhausted until the user restarts it.
  | { type: "RestartExhausted"; id: number }
  // A command was restarted because a watched file changed; the status also cycles through
  // the usual restart deltas, so this is the discrete signal for a file-watch banner.
  | { type: "FileRestart"; id: number }
  // A project was opened/changed. The UI re-reads the rendered project snapshot on this
  // (which carries each project's loaded icon); it doesn't consume the event's domain fields.
  | { type: "ProjectOpened"; id: number }
  // A project was removed: its processes were closed (each also announcing ProcessRemoved)
  // and its Soloist state deleted. The UI re-reads the project snapshot and drops any
  // state keyed to the id; files on disk are untouched.
  | { type: "ProjectRemoved"; id: number }
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
  | { type: "OrphansFound"; orphans: OrphanInfo[] }
  // Coordination change-notifications (C6) for the orchestration read-model. Each carries ids
  // only — the UI re-reads orchestration_snapshot (coalesced) rather than trusting a payload.
  | { type: "TodoChanged"; project: number; id: number }
  | { type: "TimerArmed"; owner: number; id: number }
  // A timer fired: its body was delivered to the owner as a fresh turn and it left the armed set.
  | { type: "TimerFired"; owner: number; id: number }
  | { type: "TimerCleared"; owner: number; id: number }
  // A paused timer's countdown is frozen; a resumed one re-arms with the time that remained.
  | { type: "TimerPaused"; owner: number; id: number }
  | { type: "TimerResumed"; owner: number; id: number }
  | { type: "LeaseChanged"; project: number; key: string }
  // Keyed by the scratchpad's `name` handle (the addressing key its surface uses).
  | { type: "ScratchpadChanged"; project: number; name: string }
  | { type: "KvChanged"; project: number; key: string }
  // A template of `kind` was created, updated, or deleted. `project` names the scope it changed in
  // — null for the global library, an id for that project's — because the two are separate lists a
  // surface reads separately. A templates surface re-reads that (kind, scope) list (coalesced); the
  // selected default is read separately.
  | { type: "TemplateChanged"; kind: TemplateKind; project: number | null };

export interface AppInfo {
  name: string;
  version: string;
}

// ── Coordination read-model (mirrors core::coordination view types) ──────────
// Projected by the orchestration snapshot; the orchestration UI renders these.
// Enum string values are the core's serde `snake_case` output. Ids serialize as numbers.

export type TodoStatus = "open" | "blocked" | "in_progress" | "done";

// The revision-guarded document a todo carries: a title, a free-form Markdown body, and status.
export interface TodoDoc {
  title: string;
  body: string;
  status: TodoStatus;
}

// Who wrote a comment, stamped by the core from the caller's identity (serde tag = "kind"). A bound
// process carries its live id plus the durable label the board shows; an external caller carries its
// label. `null` author (below) means the caller was unbound.
export type CommentAuthor =
  | { kind: "process"; id: number; label: string }
  | { kind: "external"; label: string };

// A comment on a todo (its per-todo sequential id, body, and author when attributable).
export interface Comment {
  id: number;
  body: string;
  author: CommentAuthor | null;
}

// A todo as the board reads it: the document plus its live columns (tags, blockers, the unmet
// subset, the derived blocked flag, comments, lock owner) and the revision to guard the next write.
export interface TodoView {
  id: number;
  doc: TodoDoc;
  tags: string[];
  blockers: number[];
  blocked_by: number[];
  blocked: boolean;
  comments: Comment[];
  locked_by: number | null;
  /**
   * The scratchpad this todo was derived from, or `null` — the permanently valid default. Only the
   * durable id is stored; the core resolves the handle on read, so a rename follows the link.
   */
  scratchpad: ScratchpadRef | null;
  revision: number;
}

// A reference to one scratchpad: its durable id and the `name` handle resolved when it was read.
export interface ScratchpadRef {
  id: number;
  name: string;
}

// What a timer waits for (serde tag = "kind").
export type FireCond =
  | { kind: "at" }
  | { kind: "when_idle_any"; watched: number[] }
  | { kind: "when_idle_all"; watched: number[] };

export type TimerStatus = "armed" | "paused";

// A timer as the panel reads it: its body, fire condition, status, absolute deadline, and — for
// fire-when-idle timers — which watched processes are not yet idle and whether the quorum was
// already met at read time. `waiting_on` and `already_idle` are computed by the façade from live
// idle state and default to [] / false for plain `At` timers (serde `default`).
export interface TimerView {
  id: number;
  /** The process that owns this timer (the delivery target). */
  owner: number;
  body: string;
  fire: FireCond;
  status: TimerStatus;
  deadline_unix_millis: number;
  /** Watched processes not yet idle — empty for `At` timers and once the quorum is met. */
  waiting_on: number[];
  /** Whether the idle condition was already satisfied at the moment the snapshot was built. */
  already_idle: boolean;
  /**
   * For a paused timer, the milliseconds that remained when it was paused — the frozen value the
   * panel shows, so a paused countdown does not drift with the wall clock. `null` for an armed
   * timer, whose remaining time is derived from `deadline_unix_millis`.
   */
  paused_remaining_millis: number | null;
}

// A live lease: its key, the process that holds it, and the absolute expiry.
export interface LeaseView {
  key: string;
  owner: number;
  expires_unix_millis: number;
}

// A scratchpad in a listing (identity, handle, tags, archived flag, revision, and a one-line gist of
// the body — its first non-heading line).
export interface ScratchpadSummary {
  id: number;
  name: string;
  tags: string[];
  archived: boolean;
  revision: number;
  gist: string;
  /** Unix millis of the last body write (0 for a document predating the field) — the recency sort key. */
  updated_at: number;
}

// A scratchpad as the panel reads it: the free-form Markdown body plus its tags, revision (to guard
// the next write), and the canonical Markdown rendering the core derives (the body under its name).
export interface ScratchpadView {
  id: number;
  name: string;
  tags: string[];
  archived: boolean;
  revision: number;
  body: string;
  rendered: string;
}

// A project-scoped key-value entry; `value` is arbitrary JSON.
export interface KvEntry {
  key: string;
  value: unknown;
}

// ── Orchestration read-model (mirrors core::orchestration) ───────────────────
// One node in the agent lineage tree: a worker nests under the lead that spawned it (`parent`); a
// manually launched agent, a command, or a terminal is a root (`parent` null). A node whose parent
// has left the registry is re-rooted, so a closed lead never strands its workers.
export interface AgentNode {
  id: number;
  parent: number | null;
  // The process's display label — the tree row's name.
  label: string;
  kind: ProcessKind;
  status: ProcStatus;
  activity: AgentActivity | null;
}

// One live spawn-lineage edge: a worker and the lead that spawned it, both still in the registry.
// The cross-project shape the sidebar joins onto its process list to nest workers under leads; an
// edge disappears once either end leaves the registry, so a closed lead re-roots its workers.
export interface LineageEdge {
  child: number;
  parent: number;
}

// The orchestration read-model for one project: its agent tree plus the coordination state agents
// share. Produced by the `orchestration_snapshot` query (exposed to the UI by a Tauri command).
export interface OrchestrationSnapshot {
  project: number;
  agents: AgentNode[];
  todos: TodoView[];
  timers: TimerView[];
  leases: LeaseView[];
  scratchpads: ScratchpadSummary[];
  kv: KvEntry[];
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

// When a process row shows its CPU/memory read-out (mirrors the core Sidebar threshold enums).
export type ProcessCpuThreshold = "always" | "pct10" | "pct30" | "pct60" | "pct90" | "never";
export type ProcessMemThreshold = "always" | "mb100" | "mb500" | "gb1" | "gb2" | "never";

// The Sidebar tab document (mirrors core::Sidebar) — what the process-tree sidebar shows.
export interface Sidebar {
  show_filter_input: boolean;
  hide_empty_sections: boolean;
  process_cpu_threshold: ProcessCpuThreshold;
  process_mem_threshold: ProcessMemThreshold;
  show_settings_footer: boolean;
}

// The context a hotkey is active in (mirrors core::HotkeyScope). Bindings only conflict within
// the same scope.
export type HotkeyScope = "general" | "sidebar" | "terminal" | "scratchpad";

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
  | "decrease_terminal_font_size"
  | "archive_scratchpad";

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

// The Notifications settings — the master on/off for every desktop toast. Off silences
// notifications everywhere; the per-project crash/exit and terminal-alert switches refine what an
// enabled reactor shows. Mirrors soloist_core::Notifications.
export interface Notifications {
  enabled: boolean;
}

// A toggleable MCP feature-tool group (mirrors core::McpFeatureGroup). Core groups are always
// served and are not represented here.
export type McpFeatureGroup = "scratchpads" | "todos" | "timers" | "key_value" | "prompt_templates";

// Which MCP feature-tool groups the server exposes (mirrors core::McpToolGroups). Scratchpads,
// Todos and Timers default on; Key-Value and Prompt Templates default off.
export interface McpToolGroups {
  scratchpads: boolean;
  todos: boolean;
  timers: boolean;
  key_value: boolean;
  prompt_templates: boolean;
}

// What a generated MCP client snippet needs (mirrors the app's McpSetupInfo): the helper command
// (absolute when installed beside the app binary, else the bare name) and the data-directory
// facts — when overridden, every snippet must carry the env var or the helper misses the socket.
export interface McpSetupInfo {
  helper_path: string;
  data_dir: string;
  data_dir_overridden: boolean;
}

// ── Per-project settings (mirrors core::settings::project + core::projects::page) ────────────
// The durable, app-local preference surface for one project plus the assembled settings-page read
// model. Field names match the core's serde output; the command-spec fields on `ProcessSpec` are
// optional to match the core's `skip_serializing_if` (they may be absent when left at their default).

// A project's durable id (mirrors core::ProjectId — a number on the wire).
export type ProjectId = number;

// Where a command lives (mirrors core::Visibility): in the shared `solo.yml` ("shared", committed)
// or the app-local overlay ("local", this machine only).
export type Visibility = "shared" | "local";

// One command definition (mirrors core::config::ProcessSpec). Defaulted fields are omitted by the
// core's serde, so they are optional here; only `command` is always present.
export interface ProcessSpec {
  command: string;
  working_dir?: string | null;
  auto_start?: boolean;
  auto_restart?: boolean;
  restart_when_changed?: string[];
  env?: Record<string, string>;
}

// One project's local settings (mirrors core::ProjectSettings) — the auto-start gate, the
// auto-trust-command-changes toggle, editor override, alert toggles, per-command alert overrides,
// and app-local commands.
export interface ProjectSettings {
  auto_start_gate: boolean;
  auto_trust_command_changes: boolean;
  editor_override: string | null;
  crash_exit_alerts: boolean;
  terminal_alerts: boolean;
  command_terminal_alerts: Record<string, boolean>;
  local_commands: Record<string, ProcessSpec>;
}

// One command on the settings page (mirrors core::ProjectCommandView). The spec fields are flattened
// so they are always present; `visibility` is where it lives; `status` is its live state, or null
// when no process of that name is registered.
export interface ProjectCommandView {
  name: string;
  command: string;
  working_dir: string | null;
  auto_start: boolean;
  auto_restart: boolean;
  restart_when_changed: string[];
  env: Record<string, string>;
  visibility: Visibility;
  terminal_alerts: boolean;
  status: ProcStatus | null;
}

// Whether the project's `solo.yml` currently loads (mirrors core::ConfigStatus); `error` carries the
// parse/IO message when it does not.
export interface ConfigStatus {
  valid: boolean;
  error: string | null;
}

// The assembled per-project settings page (mirrors core::ProjectSettingsPage) — one read the page
// renders directly: the project's root, config validity, command roster, live counts, local
// settings, and resolved editor.
export interface ProjectSettingsPage {
  project: ProjectId;
  root: string;
  config: ConfigStatus;
  running: number;
  total: number;
  settings: ProjectSettings;
  resolved_editor: string | null;
  commands: ProjectCommandView[];
}
