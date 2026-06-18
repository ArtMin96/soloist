# Phase 7 — Agents & Idle Detection (C4)

**Goal:** First-class **agents**: configurable agent tools, a launch flow, and the **5-state idle FSM**
(`IDLE/PERMISSION/THINKING/WORKING/ERROR`, ref §6) that powers notifications (Phase 6) and fire-when-idle
timers (Phase 9). Auto-summarization is **optional and degradable** (`04` §8) — the heuristic works with
no LLM.

**Delivers:** E1–E7 (E7 completed with Phase 9). **Architecture:** context C4; consumes C3 output + OSC
titles; `Summarizer` + `Clock` ports.

## Scope
**In:** agent-tool registry (config); `--version` autodetect; launch (picker + flags); the idle FSM with
per-provider heuristics; optional auto-summarization; activity surfacing in the UI. The main agent
process launches **interactively in its real PTY** (Phase 4) — never headless `-p` — so the CLI's
**own** native login/OAuth flow runs in-terminal (E8). **Out:** the MCP
`spawn_agent`/coordination tools (Phases 8/9 expose these); the settings *screen* (Phase 11; the model
lives here); **agent credential management** — Soloist never stores, prompts for, or injects API keys /
OAuth tokens; agents use the auth the user already configured, exactly as Solo does (`05` §6, E8).

## Tasks
1. **Agent-tool registry (E1/E3, ref §6):** persisted (SQLite) tool defs — built-in types **Claude,
   Codex, Amp, Gemini, OpenCode, Generic** (+ Copilot CLI, Kimi CLI). Per-tool: name, command, default
   args (appended every launch), tool-type mode (auto-detect/manual), prompt mode for generic
   (`stdin`|appended arg).
2. **Auto-detect (E2):** probe `--version` for `claude`,`codex`,`amp`,`gemini`,`opencode`; mark
   present/absent. We never install the CLIs.
3. **Launch (E4, ref §6):** create an **Agent** process (Phase 3 subtype) in the project's dir via the
   tool's command+args; "agent with flags" lets the user edit flags for one launch; many agents
   concurrently. "Resume last session" for a stopped agent (B9) where supported. Launch **interactively**
   on the real PTY (no `-p`) and pass the env through (`$DISPLAY`/`BROWSER` for the browser step, any
   `ANTHROPIC_*` the user set) so a not-yet-authenticated CLI can complete its **own** native login
   (OAuth loopback, or paste-code fallback) right in the terminal pane — Soloist authenticates nothing
   itself (E8, `05` §6).
4. **Idle FSM (E5, ref §6):** classify each agent into `IDLE/PERMISSION/THINKING/WORKING/ERROR` using a
   **Strategy per provider** (`04` §9): Claude/OpenCode → visible-output deltas; Codex/Amp → **OSC title
   stability** (from Phase 4 `TerminalTitleChanged`); Gemini → OSC title status. Emit
   `AgentActivityChanged{id,state}`; `PERMISSION`/`ERROR` raise attention (Phase 6 bell).
5. **Optional auto-summarization (E6, ref §6):** when enabled, on a quiet window send a **compact
   rendered-text snapshot** (from C3 rendered buffer) to the `Summarizer` port; the real adapter shells
   out to the user's **own** agent CLI headless (e.g. `claude -p`), cadence-limited (15s/30s/1min).
   **Default off**; if unavailable, idle detection runs heuristic-only — never blocks the core (K5).
6. **UI surfacing:** agent rows show the activity state (working/idle/permission/error) with smooth,
   non-flickering updates (ref §10).

## Interfaces
```rust
struct AgentTool { name:String, command:String, default_args:Vec<String>, kind:AgentKind, prompt_mode:PromptMode }
trait Summarizer { async fn summarize(&self, snapshot:&str)->Result<String>; }   // optional adapter
enum DomainEvent { AgentActivityChanged{id:ProcessId,state:AgentActivity}, AgentSummary{id,text} }
impl Agents { async fn launch(&self, project:ProjectId, tool:&str, extra_args:Vec<String>)->Result<ProcessId>; }
```

## Acceptance criteria
- Configure Claude/Codex/Gemini tools; `--version` autodetect flags which are installed.
- Launch an agent (with and without extra flags); it runs interactively in its PTY in the project dir.
- The idle FSM tracks a real agent: transitions to `WORKING` under output, `IDLE` when quiet,
  `PERMISSION` on a permission prompt (drives the attention bell).
- With summarization **disabled** everything works (heuristic-only); with it **enabled** and a working
  headless CLI, a short summary appears; with the CLI missing, no crash, graceful fallback.
- **Native agent auth works untouched (E8):** a `claude` with no stored credentials, launched as an
  agent, can complete its **own** login (browser/loopback OAuth, or paste-code) inside its Soloist
  terminal; afterwards relaunches run authenticated. Soloist stores/injects no API key or OAuth token,
  and never reads/writes the CLI's credential file.

## Test plan
- **Unit:** the idle FSM per provider against recorded output/OSC-title fixtures (deterministic, mock
  clock); summarizer port with a canned adapter + a failing adapter (degradation).
- **Integration:** launch a stub "agent" script that emits known OSC titles/output and assert state
  transitions; real `claude` smoke test (manual).

## Risks & mitigations
- **Idle heuristic is fuzzy (ref §6 caveat)** → keep per-provider strategies isolated + fixture-tested;
  treat "quiet ≠ done" explicitly; never auto-act on idle without a timer/user.
- **Summarizer cost/availability** → optional, opt-in, user's own CLI, cadence-limited, degradable.
- **Provider drift (new agent CLIs)** → `Generic` type + pluggable strategy keeps it open.
- **Native OAuth login needs a reachable browser** → the CLI opens a loopback browser; on a headless or
  no-`$DISPLAY` host it falls back to paste-a-code in the PTY (Claude Code documents both). Don't strip
  `$DISPLAY`/`BROWSER` from the agent's env; the interactive PTY already lets the user paste the code.
  This is the CLI's flow, not ours — we only avoid breaking it.

## Effort
~5–7 days.
