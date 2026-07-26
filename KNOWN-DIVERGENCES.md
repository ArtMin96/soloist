# KNOWN-DIVERGENCES.md — where Soloist intentionally differs from Solo

> Soloist is a clean-room rebuild from Solo's **public behavior** (`plan/05`). Where we
> deliberately differ from a documented Solo behavior — or resolve a documented gap in a way
> that observably differs — it is recorded here with a rationale, so the divergence is a
> *decision*, not a drift. (CLAUDE.md §9; the formal parity walk in Phase 13 reads this file.)
>
> This is **not** the gap log. Undocumented-behavior decisions live in `plan/05 §12`. This file
> is for cases where Solo's behavior *is* documented and we chose to do something different.

Status key: 🟢 settled · 🟡 revisit in a later phase · ⚪ superseded (reversed by a later decision; the entry is kept as history).

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

## D-2 — `solo.yml` live OS watcher lands in Phase 6, not Phase 2 🟢

**Introduced:** Phase 2 (Config & Projects); resolves in Phase 6 (Monitoring & self-healing).
**Resolved (2026-07-16):** Phase 6 shipped the `notify` adapter but wired it only to
`restart_when_changed`; the config-sync trigger stayed unwired (no adapter called
`reload_project` for external edits) until the e2e trust-review walk exposed the gap. The
`ConfigWatchReactor` (projects C1) now holds a **non-recursive** watch per open project root
via a `FileWatcher::watch_dir` port method (one inotify descriptor per project, whatever the
tree's size), debounces a save burst, and drives `ProjectService::reload` — the same
reconcile the HTTP endpoint uses. An invalid mid-edit save fails the reload quietly (the
config keeps its last good state; the next valid save syncs), and a `solo.yml` deletion is
not a sync (the adapter forwards create/modify only) — the loaded config outlives the file,
matching "files on disk are never touched" on removal.

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

**Second authenticated signal — the working directory (2026-07-24):** the process-group check above
answers "which managed process is this peer?", which resolves an agent *Soloist launched*. An agent
Soloist did **not** launch (the documented `register_agent` path) has no managed process in its
group, so with ≥2 projects open it fell through to no scope at all — it could not select the very
project whose directory it was running in. Fixed at root cause by generalizing "the process I run in"
to "the project I run in", proven by **either** of two kernel-read facts about the socket peer: its
process **group** — unforgeable lineage, since a peer cannot join another project's managed-process
group — **or** its working **directory** (`/proc/<pid>/cwd`, read in the same adapter as
`SO_PEERCRED`), matched to the open project whose canonical root contains it (deepest root wins;
component-wise, so a directory under `/p/trackler2` never matches a sibling rooted at `/p/trackler`).
Neither is a tool argument the caller asserts; the group cannot be forged at all, and the directory
is kernel-read but caller-chosen — trusted only under the same-UID local model, where a process
rooted in a project already holds full filesystem access to it. The core is handed a plain path and
does the containment match (`Projects::project_at_path`). `effective_project` gains a cwd step
reached **only for a caller with no managed process in its group** (an agent Soloist did not launch):
selected → bound process → *(groupless only)* cwd project → sole project → none. A caller whose group
owns a managed process is a Soloist-launched agent, scoped by that group (via bind or select), so the
directory never pulls it into a folder it merely sits in — one session stays scoped to one project,
and `effective_project` never disagrees with the `select_project`/`authentic_scope` gate, which
authenticates against the same home project (its group's, else its directory's). This is what lets an
agent working in a project's folder simply *know* its scope (`whoami` reports it) without selecting
anything, even with 100 projects open.

**Residual (accepted, documented as policy — not a divergence):** an **external** caller
(`register_agent`, no managed process in its group) is authentically scoped to the project whose
directory it runs in, or — when its directory is inside none of the open projects **and** exactly
**one** project is loaded — to that sole project via the unambiguous single-project default. Only a
caller whose directory is inside no open project **with ≥2 open** has no authenticated scope, and the
scoped mutating tools refuse. The directory signal grants a caller genuinely rooted in a project the
ability to reach that project's *live* process surface (scrollback/start/stop) over MCP; a same-user,
unsandboxed process rooted there already holds full filesystem access to that project (the D2
local-execution model), and "opened an agent in that repo" is the intent — so the added authority is
narrow and aligned, and far narrower than the *self-asserted* `select_project(id)` the F13 check
closed. This external-caller policy is recorded in `plan/05` §12 (MCP session↔process/directory
authenticity).

**Read tools scoped too (stability audit PRD-06, 2026-07-14):** the original F13 note left the MCP
**read** tools open by design — any session could read any process's output/status/ports by id. On a
shared `0700` socket with ≥2 projects that let an agent in project A read project B's raw scrollback
(which can carry secrets). PRD-06 closes it: `get_process_output` / `get_process_raw_output` /
`search_output` / `search_raw_output` / `get_process_status` / `get_process_ports` now resolve the
caller's effective project and **refuse an out-of-scope process** (`OutOfScope`), exactly as the
action tools do (the rule lives once in `core::facade::scoped`, so every remote adapter inherits it).
`list_processes` stays cross-project — a caller keeps its overview — but **redacts** out-of-scope rows
to identity only (id, project, kind, label, status; no ports, exit code, trust flag, or resumability).
The local UI and the (now token-authenticated, see D-17) HTTP API keep the unscoped reads, since the
local user is not scope-limited.

**Effect on parity:** F3 (effective project scope) and F13 (a tool cannot touch another project) are
**delivered** — the scope is now authenticated, so the cross-project isolation guarantee holds for the
action tools **and** the read tools. Tests prove a forged bind/select and a cross-project read are
both refused.

---

## D-7 — Scratchpads carry an enforced disciplined structure, not free-form Markdown ⚪ SUPERSEDED

> **⚪ SUPERSEDED (owner decision, 2026-07-18).** The owner reversed the enforced-structure
> directive: scratchpads (and todos, [D-8](#d-8--todos-carry-an-enforced-disciplined-structure-and-a-blocker-gate--superseded)) are now **free-form Markdown documents** — `Scratchpad { name, body: String, tags,
> archived, revision }` — edited in a rich, Notion-style TipTap editor (slash commands, autosave,
> undo/redo). This **realigns Soloist with Solo's actual model**, which `plan/05` §6/§7 already record
> as free-form ("a scratchpad is a free-form Markdown note", `plan/05`:233; "a todo is a free-form item
> with a title and an arbitrary body", `plan/05`:282), so the entry below no longer describes a
> divergence — it is history. Size caps (256 KiB scratchpad / 64 KiB todo) and the revision guard are
> unchanged; **blank bodies are now valid** (name/title + caps are the only invariants). Migration v13
> converts every stored structured doc to sectioned Markdown one-way, using the old canonical `render()`
> layout as the faithful converter (proven zero-loss by a seeded-row test).
>
> **New in place of the enforced schema — a deliberate Soloist EXTENSION beyond Solo:** a **unified
> Templates system**. One `Template { kind: TemplateKind::{Prompt, Scratchpad, Todo}, … }` aggregate
> (generalized from the prompt-template vertical — no parallel implementation) lets users author
> scratchpad/todo templates in a Settings surface with the same editor, select a global default per
> kind, and have every creation path (UI **and** MCP) seed a new empty document from the selected
> template through **one core seam**. This keeps the coordination value the enforced structure provided
> — "write it the same way every time" — but as **user policy an author configures**, not a schema the
> core imposes; a template is a suggestion an agent may still ignore (the accepted trade-off). Solo has
> no equivalent templates concept, so this is a Soloist original, recorded here and as a gap decision in
> [`plan/05` §12](plan/05-solo-reference-and-sources.md). Full design + research evidence: Soloist
> scratchpad `rich-editor-design` (revision 3); shipped across build phases A–F, evidenced in
> `PROGRESS.md`.
>
> *The original entry is retained below unchanged for the historical record.*

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

## D-8 — Todos carry an enforced disciplined structure and a blocker gate ⚪ SUPERSEDED

> **⚪ SUPERSEDED (owner decision, 2026-07-18)** — the enforced-structure half only. A todo's
> **document** is now free-form: `TodoDoc { title, body: String, status: TodoStatus }` (256/64 KiB caps
> and the revision guard unchanged; blank bodies valid), edited in the same rich editor as scratchpads,
> seeded on create from the selected Todo template through the one core seam. This is the reversal
> described in [D-7](#d-7--scratchpads-carry-an-enforced-disciplined-structure-not-free-form-markdown--superseded); see it for the Templates extension and the migration (v13 for the todo doc). **The
> blocker gate, the process-owned lock, comment authorship, and the durable identity are NOT superseded**
> — they are live columns around the document, unchanged, and remain the correct clean-room record below.
> Full design: Soloist scratchpad `rich-editor-design`; shipped in build phases A–F (`PROGRESS.md`).
>
> *The original entry is retained below unchanged for the historical record.*

**Introduced:** Phase 9 (Coordination, G3/G4/G5). Same project-owner directive as [D-7](#d-7--scratchpads-carry-an-enforced-disciplined-structure-not-free-form-markdown--superseded): the
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

---

## D-13 — `submit_solo_feedback` stores feedback locally, never transmits it 🟢

**Introduced:** later sweep (F12), 2026-07-02.

**Solo (ref `plan/05` §7):** the Setup/Support MCP tool `submit_solo_feedback` submits feedback to
the Solo team — a vendor service receives the message.

**Soloist:** the tool keeps Solo's name (interop — agents following Solo-era docs still find it) and
the same submit-a-message shape, but the message is **appended to a local `feedback` table** in the
app's own SQLite store (trimmed, non-empty, capped at 4,000 characters per message and 500 entries
overall, wall-clock stamped) and is never transmitted anywhere. The tool's description says exactly
that, so an agent never believes it reached a vendor.

**Rationale:** Soloist is an open, local-only rebuild with no vendor backend — the licensing and
account services were dropped wholesale (D3), and no telemetry endpoint exists by design. Storing
locally keeps the tool honest and useful: the owner reads the collected notes back over the local
HTTP API (`GET /feedback`, backed by `Facade::feedback_list`).

**Effect on parity:** F12 verifies — the tool exists, accepts the documented shape, and acknowledges
with the stored entry. Only the destination differs, and that difference is deliberate and
user-favoring.

## D-14 — The packaged CLI command is `soloist-cli`, not `soloist` 🟢

**Introduced:** packaging fix, 2026-07-03.

**Solo (ref `plan/05` §8):** the companion command-line client is invoked as `solo`
(a thin HTTP client of the local API, v0.7.1+) — the CLI and the desktop app do not share
a name.

**Soloist:** the desktop app's binary already owns the `soloist` name (`/usr/bin/soloist`
from the `.deb`), so the CLI ships beside it under its crate's own binary name:
`/usr/bin/soloist-cli`. Every documented subcommand and behavior is unchanged — only the
executable name differs from the `soloist status` shorthand the plan docs use.

**Rationale:** one artifact cannot install two different programs at the same path, and
renaming the desktop binary would break the `.desktop` entry, the single-instance handoff,
and the updater's installed layout for a cosmetic win. A `soloist` shell alias remains the
user's one-line opt-in.

**Effect on parity:** H4 verifies unchanged (`soloist-cli status` prints the table); the
matrix row carries the note. If a future release wants the short name, a dispatcher or a
rename decision gets its own entry here.

---

## D-15 — `whoami` omits the OS pid, and there is no manual bind tool 🟢

**Introduced:** MCP progressive-disclosure pass, 2026-07-12 (source: Aaron Francis,
`x.com/aarondfrancis/status/2075571055041675691`, 2026-07-10; post-v0.8.2 primary evidence).

**Solo (ref `plan/05` §7 + the tweet's screenshot):** Solo's `whoami` reports the process's
**OS `pid`** (e.g. `9486`) alongside its internal process id, and §7's tool catalog lists
`bind_session_process` as an MCP **tool** an agent calls to bind its session.

**Soloist:**
- `whoami` reports the internal `ProcessId`, the process name/kind/status, the actor (`origin`),
  and the effective project by name — but **not the OS pid**. `ProcessView` (the canonical
  process projection) does not carry the OS pid, and the agent already knows its own; surfacing
  it would mean plumbing a raw pid through the read model for no operational gain.
- There is **no manual bind tool**. A Soloist-launched process's `soloist-mcp` client sends the
  bind **automatically on connect** (authenticated by `SO_PEERCRED`, D-6); an external caller uses
  `register_agent`. The agent guide (and the `AGENTS.md` section it writes) teaches this — the
  earlier guide text told agents to *call* `bind_session_process`, a tool the surface never
  exposed, so an agent following it literally would have errored. That text is fixed.

**Rationale:** keep the read model lean and the agent-facing guide truthful. Auto-bind is the
correct ergonomics (the agent should not have to bind itself) and the authenticity check
(D-6) requires the binding to come from the connecting peer, not a self-asserted tool call. The
OS pid is a detail the agent owns about itself, not a coordination fact other agents need.

**Effect on parity:** F12/identity Verify is unaffected — `whoami` still reports which process and
project a session acts on, now with names. The enriched payload, the auto-bind clarification, and
the related progressive-disclosure additions (topic `help`, init instructions, `mcp_tools_summary`,
featured `tools/list` order, decaying next-tool suggestions, and the group-level-only tool disable)
are recorded as decisions in `plan/05 §12`.

---

## D-16 — Orphan reconciliation verifies process identity and fails closed on ambiguity 🟢

**Introduced:** stability audit PRD-03, 2026-07-14.

**Solo (ref `plan/05` §4 "Orphaned processes"):** Solo v0.9.3's changelog notes a fix so restart
reconciliation no longer risks acting on a PID/PGID the OS **recycled** to an unrelated group. Solo
documents *that* the class is fixed, not *how*.

**Soloist:** each recorded process group is stamped, at record time, with a stable identity — the
kernel `boot_id` (`/proc/sys/kernel/random/boot_id`) plus the group leader's start-time
(`/proc/<pid>/stat` field 22). Reconciliation and the surfaced-orphan Kill path both re-check this
identity through the `OrphanControl` port and treat a group as the recorded orphan **only** when it
matches. This produces two observable fail-closed behaviors a bare-pgid check would not:
- A **legacy record** written before identity stamping (no captured identity) is unverifiable, so it
  is **dropped, not offered for kill** — a one-time effect on the first launch after upgrade. A
  genuine leftover from before the upgrade is left running (leaked) rather than risk SIGKILLing a
  recycled pgid.
- A group whose **leader has exited but whose children linger** reads as gone (its `/proc/<pgid>`
  entry is absent), so it is pruned rather than reaped. The lingering children are leaked, never a
  wrong kill.
- A **failed SIGKILL** on a matched group is surfaced to the user (error banner) and its record is
  kept, so the leftover is re-offered next launch instead of being silently forgotten.

**Rationale:** the audit's locked priority is that Soloist must **never** SIGKILL a process group
whose identity doesn't match the recorded orphan (the exact class Solo v0.9.3 fixed). When identity
cannot be confirmed, leaking a process is strictly safer than killing the wrong one, so every
ambiguous case resolves to "do not kill." `boot_id` + start-time are cheap, Linux-native, and
sufficient to detect PID/PGID reuse across both PID churn and reboots (D2 makes Linux the only
target).

**Effect on parity:** the orphaned-processes behavior (adopt on full match, else Kill/Kill All/Leave)
is unchanged for a legitimate same-boot leftover; only recycled/legacy/leader-gone cases resolve to
prune. No parity row regresses.

---

## D-17 — The HTTP API authenticates every route with a per-launch token, not a constant header 🟢

**Introduced:** stability audit PRD-06, 2026-07-14. **Supersedes** the constant-header note in
`plan/05` §8/§12 (`X-Soloist-Local-Auth: 1`, mutations only).

**Solo (ref `plan/05` §8):** Solo's documented HTTP API gates **mutations** with a fixed header
(`X-Solo-Local-Auth: 1`) and leaves reads open on loopback; a later Solo build (v0.9.3) is noted to
rotate a bearer token. Solo documents the header, not a per-user boundary.

**The gap this closes:** the fixed value `"1"` is CSRF protection, not authentication, and the reads
had no gate at all. But the API binds a **TCP** loopback port, which — unlike the `0700` Unix socket
the MCP server uses — any local user can reach, and CORS never constrains a non-browser client. On
the multi-user Ubuntu target (D2), any local UID could `GET /processes/:id/output` and read another
user's process logs (which can carry secrets).

**Soloist (PRD-06):**
- **A fresh random token per launch** (32 bytes of OS randomness, hex-encoded) is required on
  **every** route — reads and mutations alike — compared in constant time (`subtle`). The token is
  written into the runtime file (`http-api.json`) inside the already-`0700` data directory and the
  file itself is `0600`, so only the user Soloist runs as can read it. The token — not the socket —
  is the boundary between local users; the CLI reads it from the same file it already reads the port
  from. A missing/wrong token is **401**.
- **A `Host`-header guard** rejects (**403**) any request whose `Host` is not loopback, closing the
  DNS-rebinding path where a page the user is viewing resolves its own domain to `127.0.0.1` and
  talks to the server as same-origin (CORS never applies to that).
- Out of scope (kept as `later`, per the ticket): rotating the token mid-session / bearer refresh
  (Solo v0.9.3's fuller scheme). A per-launch token is sufficient for the local boundary.

**Effect on parity:** H1 (HTTP API) and H4 (CLI) are unchanged in surface — the same endpoints, the
same status mapping (403 trust gate, 404 unknown, 401 auth) — but every route now authenticates and
the CLI sends the token on every request. No parity row regresses.

---

## D-18 — Todos may carry an optional link to a scratchpad (a Soloist extension) 🟢

**Introduced:** the `macos-native-ux` initiative, 2026-07-19 (`plan/02` G18; owner decision the same
day). Recorded here alongside the **unified Templates** extension in
[D-7](#d-7--scratchpads-carry-an-enforced-disciplined-structure-not-free-form-markdown--superseded),
which set the precedent for logging a Soloist original in both places.

**Solo — silent, not contradicted.** ⚠️ This entry is a **strict-reading exception** to this file's
scope. `plan/05` records **no** todo↔scratchpad association for Solo: §7's todo catalog (~19 tools)
lists no such parameter, and §10's Scratchpads & Todos panels describe no link. But **no Solo page
states that todos and scratchpads cannot be linked** — the public record is simply silent. Per
`CLAUDE.md` §9 that silence *is* the gap, so the primary record is the gap decision in
[`plan/05` §12](plan/05-solo-reference-and-sources.md); this entry exists only so the extension is
discoverable beside the Templates precedent. **Nothing here asserts what Solo does or does not do.**

**Soloist:** a todo may carry an **optional** link to a scratchpad in the same project.

- **Optional means optional.** A todo is linked only when it was created *from* a scratchpad;
  otherwise it has none, permanently and validly. `validate()` never inspects the field, so every
  path that does not name a scratchpad behaves exactly as it did before. There is no validation
  error, no UI nag, and no default the user must undo — "No scratchpad" is a first-class group on the
  board, not an error bucket.
- **Live column, not a document field.** The link sits beside tags, blockers, and the lock rather
  than inside the revision-guarded `TodoDoc`, because it is coordination state, not the user's prose.
  Migration **v16** adds `todos.scratchpad_id` (`ON DELETE SET NULL`).
- **Only the durable id is stored; the handle is projected on read.** `TodoView`/`TodoSummary` expose
  `Option<ScratchpadRef { id, name }>`, resolved by a `LEFT JOIN`, so a rename still follows the link
  and no adapter ever has to resolve a name itself (`CLAUDE.md` §16).
- **`todo_update` omitted ≠ null.** An omitted `scratchpad` param leaves the link **unchanged**; an
  explicit `null` clears it. This differs on purpose from `body` in the same argument struct, which
  the update replaces — see the `todo_create`/`_update` row in `plan/05` §12.
- **No `todo_set_scratchpad` tool in v1** (owner-resolved, YAGNI): the two params cover the workflow.
- **A `scratchpad_transfer` moves the derived todos with it, link intact** (owner decision,
  2026-07-19 — the case this entry was held open for). The link means "this todo derives from this
  document", so derived work follows its source: every todo in the source project linked to the
  moved scratchpad is re-keyed to the target and **keeps its association**, because both ends move
  and the link therefore stays valid. This is the one place the link is *not* cleared, and
  deliberately unlike `todo_transfer`, which clears it precisely because the scratchpad stays
  behind. Bounded on purpose: only *directly* linked todos move — the blocker graph is not followed
  transitively. A moved todo's blockers naming a todo left behind are **cleared** and its
  process-owned lock is **dropped**, matching what a cross-project `todo_transfer` already does to
  both; a blocker between two todos that both move survives. Todos in the source project linked to
  no scratchpad, or to a different one, are untouched. The whole cascade is one transaction, so a
  todo is never stranded from the document it derives from.

**Why 🟢 (settled):** all three clearing/keeping rules are now decided — deleting the scratchpad
clears the link (`ON DELETE SET NULL`), a cross-project `todo_transfer` clears it, and a
`scratchpad_transfer` keeps it while moving both ends. The asymmetry this entry was previously held
open for (one association straddling two projects) is gone: no path leaves a todo resolving a
scratchpad in another project.

**Effect on parity:** no row regresses. G3/G4 are unchanged for any todo without a link; G18 is the
new row covering the association. Full design: Soloist scratchpad `macos-native-ux-design`.

## D-19 — A rendered prompt is returned to its caller, never applied to a running process 🟢

**Solo's documented behavior:** the prompt-templates view offers "**placeholder** fill-in before a
prompt is **applied**" (changelog v0.8.2, `plan/05` §10, 🟡 changelog-only). The wording implies a
consumer — filling in values and then delivering the finished prompt somewhere.

**What Soloist does (owner decision 2026-07-19):** F15 renders, and stops there. Substituted text is
returned to whoever asked for it — the `prompt_template_render` MCP tool returns a string, MCP
`prompts/get` returns messages the client injects into its own conversation, and the Templates
Settings surface fills and previews for copying. **No path writes a rendered prompt into a running
process**, so nothing in Soloist "applies" a prompt the way Solo's wording suggests.

**Why:** writing into a live process is a different operation with a different risk profile — it
would have to pass the trust gate (CLAUDE.md §3) and needs process targeting that render itself does
not. Folding it into F15 would have made a pure, side-effect-free query into a gated mutation, and
would have shipped a substitution engine and a delivery mechanism as one unreviewable change. Solo's
mechanism is undocumented in any case (`plan/05` §12 records the whole substitution semantics as
ours), so there is no behavior here to match precisely — only a shape to choose deliberately.

**Why 🟢 (settled, owner decision 2026-07-20):** the gap closed on its own once F15's two delivery
paths shipped, so there is nothing left for a push mechanism to add.

An agent reaches a template **by pulling it**, and both routes are live. The MCP **tools** path
(`prompt_template_list`/`_read`/`_render`) is model-controlled and supported by every MCP client
without exception — asked in plain language to use a template, an agent reads it, sees which
placeholders it declares, fills them from the context it already has, renders, and then *follows the
result*. It is the one doing the work, so nothing needs delivering anywhere. The MCP **prompts**
path is user-controlled and adds an explicit slash command on the clients that implement the
primitive. Between them, every agent Soloist hosts can obtain a fully substituted prompt.

What a push would have added is therefore only the case of forcing text into a process the user is
not currently driving — which no workflow here needs, and which would have cost a trust gate, a
process picker, and the risk of landing text in an agent mid-task. Solo's wording ("before a prompt
is applied") describes *its* delivery choice, not a capability Soloist lacks.

**If this is ever revisited** it would be a genuinely new capability, not the completion of this one:
its own parity row, behind the trust gate, and UI-initiated rather than agent-initiated (one agent
injecting text into another agent's terminal is a coordination and security hazard, not a feature).
This entry would then become ⚪ superseded.

**Effect on parity:** F15 is satisfied without it — its Verify clauses cover render, missing-value
reporting, `-32602`, and capability gating, none of which involve delivery. No row regresses.

## D-20 — Diagrams are a first-class coordination document rendering Mermaid (a Soloist extension) 🟢

**Introduced:** the `mermaid-diagrams` initiative, 2026-07-24 (owner-directed; `plan/02` §DG). Recorded
here beside the todo↔scratchpad extension
[D-18](#d-18--todos-may-carry-an-optional-link-to-a-scratchpad-a-soloist-extension-) and the unified
Templates extension [D-7](#d-7--scratchpads-carry-an-enforced-disciplined-structure-not-free-form-markdown--superseded),
the precedents for logging a Soloist original in both this file and `plan/05` §12.

**Solo — silent, not contradicted.** ⚠️ This entry is a **strict-reading exception** to this file's
scope. `plan/05` records **no** diagram or Mermaid capability for Solo: §7's tool catalog lists no
`diagram_*` tool and §10's panels describe no diagram surface. But **no Solo page states that Soloist
may not add one** — the public record is simply silent, and per `CLAUDE.md` §9 that silence *is* the
gap, so the primary decision lives in [`plan/05` §12](plan/05-solo-reference-and-sources.md). This
entry exists only so the extension is discoverable beside the other Soloist originals. **Nothing here
asserts what Solo does or does not do.**

**Soloist:** a **Diagram** is a first-class, project-scoped, durable coordination document — a sibling
of scratchpads and todos — whose body is a raw **Mermaid source string** (not typed JSON, not
free-form Markdown). It mirrors the scratchpad aggregate end to end.

- **Body is source; nothing derived is stored.** `Diagram { name, source: String (≤256 KiB), tags,
  archived, revision }`, defined once in the core; `validate()` enforces only a **non-blank `name`**
  and the size cap — a blank source is valid. The core **never renders or validates Mermaid**
  (rendering is a JS concern), so `DiagramView` carries **no `rendered` field** and a
  `DiagramSummary.gist` is the first non-blank source line, with no heading-skip.
- **Durable identity, survives restart.** A store-assigned `DiagramId` (migration **v18**,
  `SCHEMA_VERSION` 17→18) addressed by the unique-per-project `name`; project-scoped shared content
  (not process-owned), so launch reconciliation never clears it (G11).
- **Revision-guarded writes**, exactly like scratchpads (G2): `expected_revision` omitted = create,
  current = update, mismatch = `DiagramRevisionConflict { expected, actual }`. A
  `DiagramChanged { project, name }` event (ids only) drives the live roster, mirroring
  `ScratchpadChanged`.
- **MCP surface — default-ON group `Diagrams`:** nine clean-room tools
  (`diagram_list`/`_read`/`_write`/`_rename`/`_add_tags`/`_remove_tags`/`_tags_list`/`_archive`/`_delete`),
  project-scoped and ungated by trust (content, not execution); a bound agent reaches only its
  effective project's diagrams. `diagram_write` takes `{name, source, expected_revision}`. No template
  seeding, no `solo://` link, no cross-project transfer and no derived children in v1 (YAGNI).
- **Two rendering surfaces, one renderer.** The same lazy-loaded, theme-following renderer draws a
  standalone **Diagrams tab** (roster + source-editor/live-preview + toolbox) and a ```` ```mermaid ````
  fenced block **inside scratchpad/todo notes** (a TipTap code-block NodeView). Mermaid is dynamically
  imported into its own code-split chunk (`CLAUDE.md` §6) and runs at `securityLevel: 'strict'`
  (DOMPurify-sanitized, no `eval`, no iframe) under the app CSP unchanged.

**Why 🟢 (settled):** the model, storage, MCP surface, gating default, and both UI surfaces were
owner-decided (2026-07-24) and shipped together; no open question straddles the design.

**Effect on parity:** a new Soloist-only section **DG** (`plan/02`) covers it; no existing row
regresses. Full design decision: `plan/05` §12.
