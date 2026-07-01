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

---

## D-7 — Scratchpads carry an enforced disciplined structure, not free-form Markdown 🟢

**Introduced:** Phase 9 (Coordination, G1/G2). **Per the project owner's directive** (2026-06-24):
scratchpads and todos must have *disciplined, informative schemas* — "I don't want to let AI write
different ways every time."

**Solo (ref `plan/05` §6/§7/§10):** a scratchpad is a **free-form Markdown** note whose **leading H1
is the title**; the tools (`scratchpad_write`/`_read`/`_append`/`_edit`/…) read and write that arbitrary
Markdown body, with read modes full/headings/section over whatever the author wrote.

**Soloist:** a scratchpad is a **typed, structured document** — `ScratchpadDoc { objective, context,
plan[], acceptance_criteria[], risks[], status, notes? }` — defined once in
`core::coordination::scratchpad`. The MCP `scratchpad_write` tool's parameters *are* those fields, so
the schema itself presents the required structure; the core **validates** it (no required field blank;
`plan`/`acceptance_criteria`/`risks` each need ≥1 non-blank entry) and rejects a malformed write
(`InvalidScratchpad`). The core **renders** the document to one canonical Markdown layout (H1 = the
scratchpad's `name`; `## Objective` / `## Context` / `## Plan` (numbered) / `## Acceptance criteria`
(checkboxes) / `## Risks` / `## Status` / optional `## Notes`), returned alongside the structured doc.
`notes` is the single free-Markdown field for anything the structure does not cover. Identity is a
durable, store-assigned `ScratchpadId` (stable across a rename and across restarts) addressed by a
unique `name` handle per project; writes are revision-guarded (G2).

**Rationale:** the owner's product decision — coordination artifacts that multiple agents read and
extend stay consistent and informative only if their shape is enforced, not merely suggested. A typed
structure rendered to one canonical layout makes "write it the same way every time" a property of the
schema rather than a convention an agent may ignore. The free `notes` field preserves an escape hatch
so the discipline does not block legitimate ad-hoc content.

**Effect on parity:** G1 ("read/write a scratchpad") and G2 ("stale write → conflict") are
**delivered** — read/write/list/rename/tags/archive/delete over the disciplined document, with
revision-guarded writes. The observable difference from Solo is that a scratchpad cannot hold an
arbitrary free-form body: a write must supply the structured fields (and pass validation), and a read
returns both the structured doc and its canonical rendering rather than an author-formatted blob. The
Solo tools that presuppose a free-form buffer are resolved (decided 2026-07-01), not left open: the
free-form-oriented verbs (`_append`/`_edit`/`_append_section`/`_tail`/`_find`/`_clear`) are an
**intentional divergence — not implemented**, because they have no clean mapping onto the disciplined
document and some would violate its invariants (`_clear` against the non-blank rule; `_append_section`
against the fixed sections); the revision-guarded whole-document `scratchpad_write` is the deliberate
replacement. The host file-io tools (`_save_to_file`/`_load_from_file`) are **formally declined** — no
MCP tool reads or writes an arbitrary host path until a dedicated project-root FS-sandbox security
pass, which is not planned. Cross-project `_transfer` is delivered by the **O10** transfer slice
(authenticated to both project scopes; see
[D-6](#d-6--mcp-cross-project-scope-isolation-is-authenticated-f13--resolved)); its reachable success
path is the local-user loopback endpoint `POST /projects/:id/transfer-scratchpad`, since an MCP
session scoped to one project cannot authorize a genuine cross-project move. Todos carry the same
discipline ([D-8](#d-8--todos-carry-an-enforced-disciplined-structure-and-a-blocker-gate-)). The
clean-room per-tool semantics are recorded in `plan/05` §12.

---

## D-8 — Todos carry an enforced disciplined structure and a blocker gate 🟢

**Introduced:** Phase 9 (Coordination, G3/G4/G5). Same project-owner directive as [D-7](#d-7--scratchpads-carry-an-enforced-disciplined-structure-not-free-form-markdown-): the
shared coordination artifacts must have *disciplined, informative schemas*, not free-form bodies.

**Solo (ref `plan/05` §7):** a todo is a free-form item with a title and an arbitrary body, tags,
blockers, comments, a transfer, and a process-owned lock; Solo documents the tool *names*
(`todo_create`/`_update`/`_complete`/`_set_blockers`/…) but not their parameter schemas.

**Soloist:** a todo carries a **typed document** — `TodoDoc { title, description, acceptance_criteria[],
risks[], status }` — defined once in `core::coordination::todo`. The MCP `todo_create`/`todo_update`
tool parameters *are* those fields, so the schema presents the required structure; the core
**validates** it (title and description non-blank; `acceptance_criteria`/`risks` each need ≥1 non-blank
entry) and rejects a malformed write (`InvalidTodo`). Around the revision-guarded document sit live
columns each mutated by its own atomic operation — **tags**, **blockers**, **comments**, and a
process-owned **lock**.

Two semantics are clean-room decisions worth flagging:
- **The blocker gate.** `status` (`Open`/`Blocked`/`InProgress`/`Done`) is the label an agent
  *declares*; what *mechanically* prevents completion is the todo's unmet **blockers**. `todo_complete`
  (and `todo_update` setting status to `Done`) is refused with `TodoBlocked { by }` while any blocker
  still exists and is not itself done. A blocker that has been **deleted counts as met**, so dropping a
  dependency never deadlocks the graph. Keeping the gate in the blocker set (not the `status` label)
  avoids a single-source-of-truth conflict where "blocked" would be both stored and derived.
- **The lock is process-owned and per-run; the todo is durable.** `todo_lock`/`todo_unlock` set a
  `locked_by` owner ("signals, not ownership" — a lock another process holds is reported, not stolen),
  which **auto-releases when the owning process closes** (the supervisor's `LockReleaser` hook, shared
  with leases via a `CompositeLockReleaser`, G5) and is **cleared for every todo on launch** (per-run
  process ids are recycled). The **todo itself survives an app restart** (G11) — only its stale lock is
  reconciled away, never the content.

**Rationale:** identical to D-7 — enforced shape makes "consistent, informative coordination artifacts"
a property of the schema rather than a convention. The blocker gate gives G4 a real, testable meaning
("a blocker gates a todo") without a second source of truth for blocked-ness.

**Effect on parity:** G3 (create/list/get/update/complete/delete), G4 (tags, blockers, comments — a
blocker gates a todo), and G5 (process-owned lock, auto-releases on close) are **delivered**. The
observable difference from Solo is that a todo cannot hold an arbitrary free-form body (a write must
supply and pass the structured fields), and completion is gated on blockers. Cross-project
`todo_transfer` is **delivered (2026-07-01, O10)**: it moves the todo to the target project keeping
its comments and completion and clearing its blockers and lock (both reference the source project),
authorized only when the caller is authenticated to **both** projects — a single MCP session
authenticates to one project (D-6), so a genuine cross-project transfer over MCP is refused by
design and the reachable success path is the local-user loopback endpoint
`POST /projects/:id/transfer-todo` (the target must be loaded, else `UnknownProject`, so a bad id
never orphans the todo). The clean-room per-tool semantics and the cross-scope authorization are
recorded in `plan/05` §12.

## D-9 — A stopped resumable agent offers both Start and Resume 🟢

**Introduced:** B9 ("Resume last session"), delivered ahead of schedule 2026-06-29 (a `later` row
pulled forward at the owner's request).

**Solo (ref `plan/05` §10):** a stopped process's main pane shows an in-pane **Start** *or*, for an
agent, **"Resume last session"** — the documentation presents them as alternatives ("Start (or Resume
last session)").

**Soloist:** for a stopped agent whose provider supports resume, we offer **both** controls — Start
(begins a fresh session) and Resume last session (relaunches with the provider's resume-last
invocation, reopening the most recent conversation). Resume is a one-off relaunch that does **not**
overwrite the process's stored fresh command, so the two affordances stay independent across
stop/start cycles. The controls render in the existing ghost-icon `ProcessControls` cluster (sidebar
row + terminal header), gated on `ProcessView.resumable && canStart(status)`; a non-resumable process
(command, terminal, or unsupported-provider agent such as Amp or Generic) shows only Start.

**Rationale:** the two actions are genuinely distinct — continue the prior conversation vs. start clean
— and a user wants both available without having to launch a second agent to get a fresh session.
Offering both is a faithful **superset** of the documented behavior, not a contradiction: the Resume
affordance still appears exactly where Solo documents it (a stopped agent), and Start is never removed.
`resumable` is a static per-process property, so the control set never reflows as the agent cycles
(DESIGN.md: disable, don't remove).

**Effect on parity:** B9 ("stopped agent offers resume") verifies as the matrix specifies — a stopped
resumable agent offers Resume. The only observable difference from a literal "Start *xor* Resume"
reading is that Start remains present beside Resume. The undocumented resume **mechanism** (the
per-provider invocation, and the Amp/Generic gaps) is recorded in `plan/05` §12.

## D-10 — GPU terminal renderer falls back to the DOM renderer, not canvas 🟢

**Introduced:** C8 ("GPU/smooth rendering"), delivered ahead of schedule (a `later` row pulled forward
at the owner's request).

**Solo (ref `plan/05` §10/§11):** the main-pane PTY uses a **GPU renderer** (added in Solo v0.6.0).
The matrix C8 row records the contemporaneous xterm.js model as *"webgl renderer; canvas fallback"*
(`plan/02`, `plan/03` D1) — at the time, xterm.js offered a WebGL renderer with a 2-D **canvas**
renderer as the middle fallback tier.

**Soloist:** we render with the **WebGL** addon (`@xterm/addon-webgl`) and fall back to xterm's
built-in **DOM** renderer when WebGL is unavailable — there is **no canvas tier**. The reason is a
library reality, not a behavior choice: Soloist pins **xterm.js v6** (`@xterm/xterm@6.0.0`), which
**removed the canvas renderer** (`@xterm/addon-canvas@0.7.0` peer-depends `@xterm/xterm@^5.0.0` and was
not carried to v6). So v6's only renderers are WebGL (addon) and DOM (built-in), and DOM is the sole
fallback. Two failure modes degrade to DOM: WebGL2 unavailable at activation (no GPU/driver/blocked
context), and a GPU context lost at runtime (driver reset, sleep/resume) — handled via the addon's
`onContextLoss`. The addon is **lazy-loaded** (a dynamic-import chunk, ~123 kB / ~35 kB gzip) so it is
fetched only when a terminal first mounts (`CLAUDE.md` §6).

**Rationale:** WebGL is the GPU path Solo's behavior calls for; DOM is the only available fallback in
xterm v6 and is the renderer the terminal already opens with, so the upgrade-or-degrade is seamless and
visually identical. A canvas tier cannot be offered without downgrading xterm to v5.

**Effect on parity:** C8's Verify ("webgl renderer; canvas fallback") is met in substance — a GPU
(WebGL) renderer with an automatic non-GPU fallback — with the fallback tier being DOM rather than the
since-removed canvas. The runtime visual/FPS check is a user-only step (no display in CI). The
undocumented renderer-selection **mechanism** is recorded in `plan/05` §12.

## D-11 — The distributable floor is Ubuntu 22.04, not 20.04 (J1/J2) 🟡

**Introduced:** Phase 12 (packaging). **Decision (D2):** *"Ubuntu 20.04+, x86_64; `.deb` targets 22.04;
`.AppImage` (self-contained webkit) covers 20.04."*

**The plan's assumption:** the `.deb` links the system WebKitGTK 4.1 (so it targets 22.04+), and a
self-contained `.AppImage` would bundle its own WebKit and therefore run on a clean **20.04**.

**What Phase-12 testing proved (containerized smokes, glibc 2.31 image):** the `.AppImage` does **not**
run on Ubuntu 20.04. The chain is unavoidable: Tauri v2 requires **WebKitGTK 4.1**, which 20.04 does not
ship and cannot be built against there, so the bundle must be built on **22.04** (glibc 2.35). The
AppImage correctly bundles WebKit, but the libraries `linuxdeploy` pulls from the 22.04 host
(`libudev`, `libbsd`, `libelf`, `libmd`, …) reference **GLIBC_2.33/2.34**, which 20.04's **glibc 2.31**
lacks → `version 'GLIBC_2.34' not found`. Force-bundling more would not help: the GPU/display libraries
(`libGL`/`libEGL`/`libgbm`/`libdrm`/`libX11`) are deliberately left to the host so they match its
driver, and they too would drag newer glibc. There is no 20.04-compatible build path for a Tauri-v2 app
short of backporting WebKitGTK 4.1 onto a 20.04 build host (out of scope, fragile).

**Soloist (clean-room decision):** the supported floor for **both** the `.deb` and the `.AppImage` is
**Ubuntu 22.04+, x86_64**. The `.AppImage`'s value stands — it is portable and carries its own WebKit, so
it needs no `apt` install of WebKit on 22.04+ desktops (the J2 promise, scoped to 22.04+).

**Effect on parity:** **J1** (`.deb` on 22.04) and **J3** (desktop entry + icon + `solo.yml` MIME) pass
on a clean 22.04 container. **J2** passes as *"the `.AppImage` runs on a clean 22.04+ desktop without a
manual WebKit install"* — its literal *"20.04"* wording is not achievable and is revised to 22.04+ here.
Recorded in `README.md` (Platform support), `plan/02` J2, `plan/03` D2, and `plan/05` §12.

---

## D-12 — Quick Jump palette (I3): processes + projects only, not todos/scratchpads

**What Solo does:** `Cmd+E` jumps to any destination — processes, projects, todos, scratchpads.

**What we do:** the palette searches processes and projects only. Todos and scratchpads require a
per-project `orchestration_snapshot` call that is not pre-loaded at the App shell level; fetching
them on each palette open would add noticeable async latency. The I3 "later" marker reflected
missing infrastructure; now that the data exists it can be lifted by promoting the orchestration
snapshot to the App-level store and extending the palette's search targets.

**Effect on parity:** I3 is partial parity — navigation to process/project destinations works; the
todo/scratchpad jump targets are a tracked follow-up.

## D-13 — Auto-summarization covers the providers with a documented headless one-shot; the rest degrade 🟡

**What Solo does:** documents native-headless auto-summarization for Claude/Codex/Gemini (default
models `sonnet`/`gpt-5-codex`/`flash-lite`, 15s/30s/1min cadence).

**What we do:** the per-provider headless invocation is a clean-room Strategy grounded per arm in each
provider's own published CLI reference — Claude `-p`, Codex `exec`, Gemini `-p`, OpenCode `run` (model
via `--model`/`-m`), and a user-configured Generic tool via its `PromptMode` (appended arg or piped
stdin). Amp, Copilot, and Kimi document no id-less headless one-shot we could ground, so they produce
**no summary** rather than a fabricated flag — the same honesty as the resume Strategy's `NoResume`.
Summarization is OFF by default (opt-in tool+model in settings), per-agent rate-limited (30 s cooldown,
within Solo's cadence band), and fully degradable: no runner wired, an unsupported provider, a missing
CLI, or any failure yields no summary and never blocks the core (K5). The summary shows as a muted
one-line caption under the agent's sidebar row, kept as the agent's last-known activity until replaced
or the agent leaves Running.

**Effect on parity:** E6 is implemented (built ahead of its `later` marker at the owner's request).
"Disabled OK" is structural (default off, `NoopSummaryRunner`); "summary when enabled" is code-complete
and tested with fakes + a real shell, pending an owner runtime walk with a live summarizer CLI.
