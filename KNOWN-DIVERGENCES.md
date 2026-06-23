# KNOWN-DIVERGENCES.md — where Soloist intentionally differs from Solo

> Soloist is a clean-room rebuild from Solo's **public behavior** (`plan/05`). Where we
> deliberately differ from a documented Solo behavior — or resolve a documented gap in a way
> that observably differs — it is recorded here with a rationale, so the divergence is a
> *decision*, not a drift. (CLAUDE.md §9; the formal parity walk in Phase 13 reads this file.)
>
> This is **not** the gap log. Undocumented-behavior decisions live in `plan/05 §12`. This file
> is for cases where Solo's behavior *is* documented and we chose to do something different.

Status key: 🟢 settled · 🟡 revisit in a later phase.

---

## D-1 — Trust variant scope is narrower than Solo's sync re-trust triggers 🟢

**Introduced:** Phase 2 (Config & Projects).

**Solo (ref `plan/05` §4):** trust is "scoped to project + remembered command variant"; *changing
command string / working_dir / env invalidates it*. Separately, on sync Solo states "re-trust
required after changes to **command / working_dir / auto-start / auto-restart / watch / env**" — a
broader trigger set than the variant fields.

**Soloist:** the trust **variant hash** (`ProcessSpec::variant_hash`) covers **command +
working_dir + env** only (per the Phase 2 plan, Task 5, which matches Solo's *variant* definition).
A sync therefore flags `requires_trust` when an added/updated command's command/dir/env variant is
not already trusted, but **not** when only `auto_start`, `auto_restart`, or `restart_when_changed`
changed.

**Rationale:** trust is a security boundary — it answers "is *this exact thing that will run*
trusted?". `command`, `working_dir`, and `env` determine what executes; `auto_start` /
`auto_restart` / `restart_when_changed` change only *when/whether* an already-trusted command runs,
not *what* runs. Gating trust on the execution-defining fields keeps the boundary meaningful while
avoiding re-trust churn on benign scheduling edits. The change still appears in the
`ConfigChanged` diff (the row is `updated`), so the UI can surface it; it just does not force
re-trust.

**Effect on parity:** A6/A7 verify on the execution-defining fields exactly as Solo describes.
The only observable difference is that an `auto_start`-only edit does not re-prompt trust.

---

## D-2 — `solo.yml` live OS watcher lands in Phase 6, not Phase 2 🟡

**Introduced:** Phase 2 (Config & Projects); resolves in Phase 6 (Monitoring & self-healing).

**Plan wording:** the Phase 2 scope lists "the file-watcher + hash-diff + debounced sync."

**Soloist:** Phase 2 ships the deterministic, headless-tested sync engine — content-hash change
detection, the add/update/remove/rename diff, rename detection, the trust-aware `requires_trust`
decision, and a Clock-driven `Debouncer` (tested on the mock clock) — behind the `FileWatcher`
port. The **live `notify`-backed watcher** that drives this from real filesystem events is
deferred to **Phase 6**, where generic glob file-watch restart (parity D6) is the headline and
uses the same `notify` infrastructure — so we build and own that adapter once.

**Rationale:** the risk and the testable substance of sync is the pure engine, which is complete
and verified now; the OS watcher is a thin adapter best built alongside its other consumer.
User-approved at the Phase 2 planning checkpoint.

**Effect on parity:** A9's hash-diff/diff/rename/no-auto-start behavior is delivered and verified
in Phase 2 via direct-drive integration tests; the live-trigger + debounce-against-real-events
wiring completes in Phase 6. This is a build-sequencing divergence, not a behavioral one — the end
state matches Solo.

---

## D-3 — The core's *rendered* output is line-oriented, not a full cell grid 🟡

**Introduced:** Phase 4 (PTY & Terminal I/O).

**Solo (ref `plan/05` §10):** ships a GPU terminal renderer (v0.6.0) and a `get_process_output`
(rendered) tool; a full terminal emulator resolves cursor addressing, scroll regions, and screen
clears into an on-screen cell grid.

**Soloist:** the **core** maintains two bounded buffers from one PTY read stream — a byte-accurate
**raw** scrollback (every escape sequence preserved) and a **line-oriented rendered** buffer
(printable text with carriage-return overwrite and tab stops; colour/cursor escapes consumed, not
leaked). The core does **not** maintain a positional cell grid. Faithful rendering of a full-screen
TUI (vim, htop, an agent's live UI) is the **frontend terminal emulator's** job (xterm.js, Phase 5
/ parity C8), which consumes the raw stream. The core's rendered text for such an app is therefore
*approximate* (no cursor addressing); for ordinary line-based CLI output it is exact.

**Rationale:** the frontend xterm.js *is* the real terminal emulator; duplicating a full cell-grid
emulator in the core would be redundant and weigh against the size/footprint budget (§6). The
rendered projection answers "what plain text did this print" — correct for the common case and for
MCP/CLI output reads — while the raw buffer answers "exactly which bytes," which drives grid-exact
rendering downstream. Both buffers are bounded (raw 256 KB, rendered 5,000 lines).

**Effect on parity:** C4 (rendered text retrievable) and C2/C5 (raw stream with control sequences)
verify as specified; `get_process_raw_output` is byte-exact. The only difference from a
hypothetical grid-in-core design is that `get_process_output` of a cursor-addressed TUI is
line-approximate rather than grid-exact. Revisit (→🟢) if a consumer needs grid-exact rendered text
inside the core; a cell-grid model can be added behind the same buffer interface without touching
callers.

---

## D-4 — File-watch default ignore list is our own 🟢

**Introduced:** Phase 6 (Monitoring / file-watch restarts).

**Solo (ref `plan/05` §4):** file-watch restart watches the project directory recursively for
create/modify events, debounces them, and restarts on a matching `restart_when_changed` glob. The
docs explicitly note **no documented ignore list** ("❓ No documented ignore-list
(`.git`/`node_modules`). We add sensible default ignores.").

**Soloist:** a change inside any of `.git`, `node_modules`, `target`, `dist`, `.venv` (matched by
directory name at any depth, relative to the project root) never triggers a restart, **even if a
configured glob would otherwise match it** (the ignore is checked before the glob). The set lives in
one place — `core::filewatch::policy::DEFAULT_IGNORES`.

**Rationale:** these are the version-control, dependency, and build-output trees that churn
constantly (a `cargo build` rewrites all of `target/`, `npm install` rewrites `node_modules/`). Left
unignored, an ordinary build would fire a restart storm — the documented debounce coalesces a burst
but not a steady stream of writes across seconds. Ignoring them by default makes file-watch usable
without every project having to hand-exclude them. Because Solo documents *no* list, this is a
gap-filling decision (it could differ from whatever Solo does internally), so it is recorded here.

**Effect on parity:** D6 (touch a watched file → one debounced restart) and D7 (editing an ignored
path → no restart) verify exactly as the matrix specifies. The only way to observe a difference from
a hypothetical "watch everything" design is to put a `restart_when_changed` glob *inside* an ignored
directory and expect a restart — which we deliberately suppress. Revisit if a user needs to watch
inside one of these directories; the fix is a per-command opt-out, not removing the safe default.

## D-5 — Agent idle-detection thresholds & cues are our own approximation 🟡

**Introduced:** Phase 7 (Agents & idle detection, E5).

**Solo (ref `plan/05` §6):** documents the five activity states (`IDLE`/`PERMISSION`/`THINKING`/
`WORKING`/`ERROR`) and the *signal* each provider family is read from — Claude/OpenCode from visible
output, Codex/Amp from OSC-title stability, Gemini from OSC-title status. It does **not** document the
exact quiet window, the strings that mark a permission prompt, or the title keywords that map to a
status.

**Soloist:** the per-provider heuristic shapes (output-delta, title-stability, title-status) follow
Solo's documented signals, but the thresholds and patterns are our own, in one place each:
- **Quiet window:** idle after `IDLE_AFTER_QUIET_SAMPLES = 3` consecutive unchanged samples at the
  ~1 Hz idle sampler (≈3 s). A brief pause holds the previous state rather than flapping.
- **Permission cues** (`core::agents::idle::permission`): a small set of strong, model-agnostic
  approval idioms (`(y/n)`, "do you want to proceed", "allow this action", …), scanned only over the
  last few rendered lines, and only once the agent's output has **settled** (a terminal still
  producing output reads as `Working`, so a just-printed or just-answered prompt lingering in the tail
  is not misread as a live block). Deliberately conservative — it prefers a **missed** permission to a
  false one, because a wrong `Permission` would tell a fire-when-idle workflow the agent is blocked
  when it is free (or the reverse). The bare word "permission" is intentionally not a cue, so an
  ordinary "permission denied" error line is not mistaken for a prompt.
- **Title-status keywords** (`core::agents::idle::strategy`): generic thinking/working/error
  substrings mapped to activities for the title-status provider.

Copilot/Kimi/Generic have no documented heuristic, so they default to the most universal signal,
visible output.

**Rationale:** the heuristic is inherently fuzzy ("a quiet terminal is not always completed work",
`plan/05` §6), and the precise values Solo uses are unobservable. Isolating each in a single named
constant or module keeps it fixture-tested and easy to tune. The activity signal only *informs*
(notifications now, fire-when-idle timers in Phase 9); it never auto-acts, so an occasional
misclassification degrades gracefully.

**Effect on parity:** E5 ("state tracks a real agent") holds — a real agent transitions to `WORKING`
under output, `IDLE` when quiet, and `PERMISSION` on a recognised prompt. A difference from Solo would
only show as a different quiet-window latency or a permission prompt phrased outside our cue set
(reported as `WORKING`/`IDLE` rather than `PERMISSION`). Revisit the cue set as real agent CLIs are
observed; it is the most likely thing to tune.

---

## D-6 — MCP cross-project scope isolation is authenticated (F13) 🟢 RESOLVED

**Introduced:** Phase 8 (MCP server core), as a build-sequencing deferral. **Resolved:** Phase 8,
**F13** (binding/scope authenticity).

**The deferral (now closed):** the scoped MCP **action** tools (F6 process control, F8 bulk,
`clear_output`, F11 `spawn_agent`) enforce an effective-project scope, but for sessions 1–3 that scope
was *self-asserted* — `bind_session_process` accepted any *existing* process and `select_project` any
*loaded* project, neither verifying the caller ran there. With **≥2 projects open** a client on the
local (same-user, `0700`) socket could scope to a sibling project and stop/restart/clear it
(`stop_all_commands` / `restart_all_commands` / `clear_output` are not trust-gated). The tool fan-out
was sequenced first so the authenticity check could land once, over all of them.

**The check (F13):** the IPC adapter reads the connecting peer's kernel credentials
(`SO_PEERCRED` → pid → its process group) per connection and hands the core the peer's process
**group**; the core matches it to the managed process the caller runs in. `bind_session_process` is
refused (`ForeignProcess`) unless the bound process's group leader is the peer's group, and
`select_project` is refused (`ForeignProject`) unless a process in the caller's own group belongs to
the target project. Because a Soloist-launched agent's `soloist-mcp` child inherits the agent's
process group — the very group the supervisor recorded for that managed process — the legitimate
auto-bind matches, while a forged binding to a sibling project's process does not. The OS credential
detail lives only in the adapter (`crates/app/src/peer_cred.rs`); the core compares plain
process-group ids, so the dependency rule holds.

**Residual (accepted, documented as policy — not a divergence):** when exactly **one** project is
loaded, an **external** caller (`register_agent`, no managed process in its group) still acts on that
sole project via the unambiguous single-project default — identical to the local user's own authority
on the `0700` socket, and with no sibling to cross into. With **≥2** projects open such a caller has
no authenticated scope and the scoped mutating tools refuse. This external-caller policy is recorded
in `plan/05` §12 (MCP session↔process binding authenticity).

**Effect on parity:** F3 (effective project scope) and F13 (a tool cannot touch another project) are
**delivered** — the scope is now authenticated, so the cross-project isolation guarantee holds for the
action tools. Tests prove a forged bind/select to a sibling project is refused.
