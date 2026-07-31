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

**Amended 2026-07-31 (branch `feat/child-agent-lifecycle`, `Done — pending verify`) — what the quiet
window governs, and what it no longer counts for.** The window is unchanged as the *rendering* rule: a
quiet terminal still reads `IDLE` on every surface, because that is what the terminal shows. What
changed is the **fire-when-idle quorum**, which used to read the same value and treat it as *done*. An
agent's activity is now tracked only from the point it demonstrably began, so the quiet that
**precedes** an agent's first observed activity is unclassified rather than `IDLE`, and a
fire-when-idle wait (which already treats unclassified as not idle) no longer fires on a
silently-booting worker. Separately, a watched process that **exits** now ends the wait however it was
last classified — one that exited while `WORKING` used to wait out the 3600 s max-wait backstop. The
tracker and the timer scheduler fold the same rule, so what is reported and what fires stay one
answer. **The heuristic's limit is unchanged and is now stated rather than papered over:** a *noisy*
boot (a TUI banner, then quiet) still satisfies the quorum, because the signal is a cumulative output
byte count that cannot distinguish quiet-before-work from quiet-after-work. **No settle-time constant
was introduced** — see **D-35**, which records the decision that completion is explicit and terminal
quiet is not a completion signal.

**Effect on parity:** E5 ("state tracks a real agent") holds — a real agent transitions to `WORKING`
under output, `IDLE` when quiet, and `PERMISSION` on a recognised prompt. A difference from Solo would
only show as a different quiet-window latency or a permission prompt phrased outside our cue set
(reported as `WORKING`/`IDLE` rather than `PERMISSION`). Revisit the cue set as real agent CLIs are
observed; it is the most likely thing to tune. One known unevenness, code-read and not reproduced:
`looks_like_permission_prompt` has a single call site, inside the output-delta strategy, so the
title-based providers (Codex/Amp, Gemini) cannot report `PERMISSION` at all — a worker of theirs
blocked on a prompt reads `IDLE`.

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

## D-15 — `whoami` omits the OS pid 🟢 — the "no manual bind tool" half is REVERSED (2026-07-31)

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
- ~~There is **no manual bind tool**.~~ **REVERSED 2026-07-31** (branch `feat/child-agent-lifecycle`,
  `Done — pending verify`). Binding is still sent **automatically on connect** by a Soloist-launched
  process's `soloist-mcp` client (authenticated by `SO_PEERCRED`, D-6), and an external caller still
  uses `register_agent` — but the client used to **discard the result**, so a *refused* bind was
  indistinguishable from never having attempted one: both reported `origin: unbound`, with no error,
  no log, and no way back, while the session silently lost every coordination surface that needs an
  owning process (lineage on spawn, timers, leases, todo locks). This is reachable in ordinary use —
  an agent the user starts by hand inside a Soloist terminal inherits that terminal's
  `SOLOIST_PROCESS_ID` but connects from a *new* process group (shell job control), fails the peer
  check, and operates unbound. So `bind_session_process` **is now exposed as an MCP tool**, matching
  the name `plan/05` §7 records Solo documenting, routed through the same façade gate — a bind to a
  process the caller does not run in is refused exactly as before, so the tool adds a retry, not an
  authority. The refusal itself is recorded on the session and reported by `whoami` as a field beside
  `origin` (a refusal is not an identity, and the two facts move independently), and the client writes
  one deduplicated line to stderr for its MCP host to show. **The rationale below still holds for
  auto-bind** — the agent should not *have* to bind itself, and the authenticity check still requires
  the binding to come from the connecting peer; what changed is that a caller whose automatic bind was
  refused can now see why and try again.

**Rationale:** keep the read model lean and the agent-facing guide truthful. Auto-bind is the
correct ergonomics (the agent should not have to bind itself) and the authenticity check
(D-6) requires the binding to come from the connecting peer, not a self-asserted tool call. The
OS pid is a detail the agent owns about itself, not a coordination fact other agents need.

**Effect on parity:** F12/identity Verify is unaffected — `whoami` still reports which process and
project a session acts on, now with names. **`plan/02` F4 already *named* `bind_session_process`, so
the 2026-07-31 reversal also closes a pre-existing doc-vs-doc contradiction, in F4's favour.** The
enriched payload, the auto-bind clarification, and
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

---

## D-21 — Agents and terminals can be removed from the sidebar; commands cannot 🟢

**Introduced:** parity row **B11**, 2026-07-27 (owner decision). Recorded here for the same reason as
[D-20](#d-20--diagrams-are-a-first-class-coordination-document-rendering-mermaid-a-soloist-extension-):
the public record is silent, and per `CLAUDE.md` §9 that silence is the gap.

**Solo — silent, not contradicted.** `plan/05` records Solo's `close_process` MCP tool (§7) and the
ordinary Stop affordance, but **no Solo page describes a UI control that forgets a process**, nor what
such a control would do to a `solo.yml` command. Nothing is fabricated about Solo's behavior here.

**What Soloist does.** A row for an **agent** or a **terminal** offers **Remove**, which routes to the
existing `Supervisor::close` — stop, reap the process group, drop the registry entry, publish
`ProcessRemoved` — the same core behavior MCP `close_process` and the HTTP/CLI dispatch already reach.
A **command** never offers it.

**Why the split.** A command's identity lives in its `solo.yml` (or app-local overlay) declaration, not
in its process. Forgetting the process would drop the row only until the next config sync or app launch
re-registered it from the declaration that still exists, so the control would silently undo itself.
Deleting a command is therefore the command editor's job. An agent or terminal is declared nowhere: the
process *is* the thing, so removal is the only way to clear a finished one out of a sidebar that
otherwise grows for the whole app session.

**Scope of the divergence.** UI-only, and additive. The core's `close` is unchanged and still accepts
any kind, so **MCP `close_process` behavior is untouched** — a scoped agent may still close a command in
its project exactly as before. The kind restriction is presentation policy in the frontend's
`processActions`, beside `canStart`/`canStop`, per the existing split between what is *offered* and what
is *legal*.

**Why 🟢 (settled):** kinds, statuses, and the confirmation rule were owner-decided together
(2026-07-27) and shipped in one change; no open question straddles it.

**Effect on parity:** new row **B11** (`plan/02`); no existing row regresses. Full decision:
`plan/05` §12.

---

## D-22 — MCP callers may read the seed template for a kind, but never author one 🟢

**Introduced:** proposed by the implementing session and **decided by the owner on 2026-07-27**, who
reviewed the reversal of `plan/05` §12's resolved decision 4 and approved widening it from *no access*
to *no authoring*. The reversal is the owner's, not the implementer's; this entry records it rather
than authorizing it. Recorded here beside the unified Templates extension
[D-7](#d-7--scratchpads-carry-an-enforced-disciplined-structure-not-free-form-markdown--superseded),
which it refines.

**Solo — silent, not contradicted.** `plan/05` records **no** user-authored-templates concept for Solo
beyond the v0.8.2 prompt-templates view, so scratchpad and todo templates are a Soloist original and
their agent-facing surface is ours to decide. Nothing here asserts what Solo does.

**The gap this closes.** Scratchpad and todo templates seed the body of a document created **empty**,
and the create reply's `seeded_from` names the template used. An MCP agent was therefore *affected by* a
template it could not *observe*: asked to write a scratchpad "following the project's template", it had
to guess the shape or reverse-engineer it from existing documents, and both drift from the template.
`ScopedFacade` pins every `prompt_template_*` action to `TemplateKind::Prompt`, so the two seed kinds
were invisible to it.

**What Soloist does.** `ScopedFacade::seed_template(kind)` is a **read-only** peek returning
`SeedTemplate { name, body }` — what seeding a new empty document of that kind would apply — or
nothing when the local user has selected no default. Two MCP tools expose it —
`scratchpad_template` and `todo_template`, neither taking arguments.

**Why read-only access rather than none.** Both fields were **already reachable** by any scoped
caller: an empty `scratchpad_write` seeds the body and returns the written view — `IpcResponse`
carries the core `ScratchpadView`, body included, verbatim — and the same reply names the template in
`seeded_from`. The peek therefore discloses nothing new — it removes the junk document the disclosure
used to cost. Confidentiality of the template body was never a property of this design, so
withholding the read bought no security and charged the user a stray note per lookup. It also matches the pull model
[D-19](#d-19--a-rendered-prompt-is-returned-to-its-caller-never-applied-to-a-running-process-) records
for prompts: an agent reaches a template by pulling it.

**What stays the local user's authority.** Creating, editing, deleting, and **selecting the default**
for a seed kind remain on `Facade`, driven by Settings → Templates. `ScopedFacade` gains one read and
no writes, so `CLAUDE.md` §16's "scope is a type" holds: the scope-limited caller still cannot reach an
ungated door.

**Three deliberate narrowings.**

- **The selection, not the library.** The peek answers "what would seed a create", so it returns the
  *selected default* and takes no name. A template the user authored but did not select is not shown,
  and no scoped caller can enumerate the template library.
- **Gated with its own kind.** Each tool lives in its kind's existing feature group (`Scratchpads`,
  `Todos`), not in `PromptTemplates`. Turn a group off and both the create *and* its peek disappear —
  so the peek is available exactly where the write that already exposed the body is available, and
  never widens reachability in any settings configuration.
- **What a create applies, not what the template is.** The answer is `SeedTemplate { name, body }`,
  the two fields the seeding path consumes — never the full `TemplateView`. The template's authoring
  metadata stays off the wire: its `description` above all, which is prose the user wrote for the
  Settings manager and which no create has ever disclosed. Narrowing at `Facade::seed_template`
  rather than at either caller means the peek cannot drift wider than the create it describes, and
  makes "discloses nothing new" true field by field rather than approximately.

All three resolve through the same `Facade::seed_template` the create path uses, so a caller is never
shown a shape a create would not actually apply.

**Why 🟢 (settled):** owner-decided and shipped together. The one open question it surfaced — that the
default *selection* was global-only, which left the owner's own project-scoped templates unselectable and
so inert — was decided and built the same day: seed defaults are now **per-project** with no global
fallback (`plan/05` §12). The peek needed no change to follow it, because both resolve through the one
`Facade::seed_template`.

**Effect on parity:** refines the "Unified Templates" row in `plan/05` §12, whose "no agent-facing
template-CRUD MCP tools for the Scratchpad/Todo kinds in v1" now reads as *no authoring* rather than
*no access*. `plan/02` **F15** carries the same amendment — its "resolved decision 4 … stands" clause
named the wider restriction — and **I13** records the per-project move. No row regresses.

---

## D-23 — The terminal's full ANSI palette is authored per theme, with a runtime readability floor 🟢

**Introduced:** Phase 4 surface, built on the branch that themes the emulator.

**Solo — silent, not contradicted.** `plan/05` records nothing about Solo's terminal colours beyond
OSC title and bell handling. There is no documented Solo palette to match or to differ from, so this
is a **clean-room addition** rather than a divergence from observed behavior, recorded here because
`plan/05` §12 owns the decision and this file is where the parity walk reads it. Nothing below
asserts what Solo does.

**The defect this closes.** `terminalColors()` set 5 of xterm 6's ~26 `ITheme` fields. All 16 ANSI
slots were therefore left to xterm's built-in defaults, which are tuned for a dark terminal — so in
the Light theme program colour rendered against a near-white `#fbfbfd` surface with no relationship
to the app at all.

**What Soloist does.** Both themes carry the full ANSI set plus the unfocused-selection tone and the
three scrollbar-slider colours. Each hue is the app's own signal hue, so the terminal reads as one of
the instruments rather than a foreign surface: red is DESIGN.md's crashed red (27), amber the
transition amber (70), green the running green (150), blue the azure accent (245); cyan bridges green
to azure at 200 and magenta sits at 328, deliberately clear of the 264-300 violet band DESIGN.md
rejects as the "purple tell". Black and white ride the cool-slate neutral. The palette is authored in
OKLCH and **emitted as hex** — xterm.js cannot parse `oklch()`.

**This is the one exemption from DESIGN.md's Spent-on-Status Rule** ("saturated color is forbidden
except on a status indicator"), and the rule now records it. Sixteen saturated slots are not the app
reporting `ProcStatus` — they are program output the emulator is obliged to render, and reusing the
status hues for them is what keeps the terminal reading as one of the instruments. The Settings
swatch row is covered by the same exemption: its whole subject is the palette. Nothing else in the
app may borrow a status hue on this precedent.

**Bright is the more emphatic set, not merely the lighter one.** `drawBoldTextInBrightColors` defaults
to `true`, so bold output renders in the bright half. On the light theme bright is therefore *darker*
and more saturated than its normal twin; on dark it is lighter. It is never less legible than the
normal slot — bold that reads worse than plain text would be a defect the ANSI convention hides.

**The contrast rule, and its two honest exemptions.** Every slot clears 4.5:1 against its own
background **except the one whose ANSI role is the surface end of that theme** — `white` and
`brightWhite` on light, `black` on dark. Demanding 4.5:1 of those would invert what the slot means:
`\e[47m` has to paint a pale panel and `\e[40m` a dark one, and a "white" that is really a mid-grey
breaks every `\e[3x;4ym` pairing that uses it as a background. `brightBlack` is *not* exempt in either
theme: it is the dim-text slot, not a surface tone, so DESIGN.md's `slate-muted` rule ("verified
≥ 4.5:1 on Cool White; never lighter, no 'elegant' pale gray") applies to it unchanged.

**Why the floor and the palette are complementary, not redundant.** `minimumContrastRatio: 4.5` is set
alongside the palette. It is a **top-level terminal option, not an `ITheme` field**, so it only reaches
the emulator through `terminalOptions()`. It exists for the colour we do *not* choose: the 256-colour
and truecolor foregrounds a program picks for itself, and the surface-end slots above when a program
uses one as text. On the rest of the palette it never fires — but that is a property the palette had to
*earn* on three backgrounds, not one. A selected cell sits on a 30% wash of the selection colour over
the surface (xterm forces an opaque `selectionBackground` to that alpha and blends it), which costs
every slot a little contrast. `brightBlack` — the dim slot, and the one CLI output leans on hardest —
is the only one without the margin to absorb it, so it carries extra headroom against the bare
background (4.99:1 light, 5.24:1 dark) in order to still clear 4.5:1 over both the active and the
unfocused selection. Without that, dim text visibly changed colour the moment it was selected.

**How narrowly it clears — read this before retuning any of the three.** Behind the selection
`brightBlack` measures 4.600:1 light / 4.635:1 dark on the active selection and 4.605:1 light /
4.627:1 dark on the unfocused one. That is ~0.10 of margin over the 4.5:1 floor — roughly one 8-bit
quantization step. `brightBlack`, `selectionBackground`, `selectionInactiveBackground` and the
terminal `background` are therefore a coupled set: nudging any one of them by a single hex step can
put the dim slot under the bar, at which point the runtime floor starts recolouring selected dim
text. `terminalPalette.test.ts` fails loudly if it does; retune until it passes rather than adding
the slot to that file's `SURFACE_END` exemptions, which exist for a different reason entirely.

**Why `selectionForeground` stays unset.** Reading xterm 6.0.0's shipped renderers, the minimum-contrast
adjustment resolves against the cell's **real** background — for a selected cell that is the selection
background (the DOM row factory passes it as the background override; the WebGL cell resolver writes it
into the cell's background before the atlas lookup). Selected text is therefore already guaranteed
readable, so pinning `selectionForeground` would only flatten a coloured selection to one tone and lose
information, and would depart from the emulators a user already knows.

**Effect on parity:** adds `plan/02` **C10**. **C2** ("Full ANSI / color") is unchanged and still ✅ —
it covers whether ANSI renders at all; C10 covers whether the 16 colours are ours and follow the theme.
No row regresses.

## D-24 — The terminal cursor's shape and blink are user settings, not constants 🟢

**Introduced:** Phase 4 surface, on the branch that promotes the cursor to Settings.

**Solo — silent, not contradicted.** `plan/05` records nothing about Solo's terminal cursor: not its
shape, not whether it blinks, not whether either is configurable. There is no documented Solo behavior
to match or to differ from, so this is a **clean-room addition** rather than a divergence from observed
behavior, recorded here because `plan/05` §12 owns the decision and the parity walk reads it from this
file. Nothing below asserts what Solo does.

**The defect this closes.** `cursorBlink: true` was hardcoded at the emulator's construction and
`cursorStyle` / `cursorInactiveStyle` sat at xterm's defaults, with no way for a user to change any of
the three.

**What Soloist does.** The appearance document carries two closed enums and a boolean —
`CursorStyle { Block, Underline, Bar }`, `CursorInactiveStyle { Outline, Block, Bar, Underline, None }`
and `cursor_blink` — mirrored once in `domain.ts` and handed to xterm unchanged. The permitted sets are
xterm's own, read from the installed typings rather than from memory: `@xterm/xterm@6.0.0`'s
`xterm.d.ts` declares `cursorStyle?: 'block' | 'underline' | 'bar'` and
`cursorInactiveStyle?: 'outline' | 'block' | 'bar' | 'underline' | 'none'`. Because the serialized enum
strings already *are* those values, nothing translates between the two — unlike the font weight and
line height beside them in `lib/appearance.ts`, whose domain steps carry no xterm meaning of their own.
The two sets are still held to each other at compile time: the value flows into `new Terminal({…})` and
into `term.options.cursorStyle`, both typed by xterm, so a domain variant the emulator does not accept
fails to build. The pickers that offer the three are derived from label records keyed by the same
enums, so a variant xterm *would* accept still cannot ship without a label to show for it.

**Defaults `Block` / `Outline` / `true`, and why blink departs from xterm.** The first two are xterm's
own defaults. `cursor_blink` does not follow xterm's `false`: the app has always run a blinking cursor,
so `true` is what keeps an upgrade from silently changing the terminal under an existing user. That is
the whole reason the default is stated rather than inherited. `Outline` is kept as the unfocused
default in preference to `None` — hiding the cursor is a legitimate choice, offered in the picker as
"Hidden", but a poor default, because an unfocused pane then reads as having no cursor position at all.

**Why there is no schema migration.** `SCHEMA_VERSION` stays **18**. The settings row persists as a
single JSON document (`settings.doc`) parsed straight into `Settings`, whose containers carry
`#[serde(default)]` and set no `deny_unknown_fields` — so three new struct fields need no DDL, and a
record written before they existed reads back with the defaults above. This is the same "add a field,
not a store" recipe the per-project seed-template defaults already follow. A bump would be worse than
merely redundant: `migrate()` refuses any database whose `user_version` exceeds the running build's, so
bumping to 19 would make a database this build has touched unopenable by an older one, in exchange for
no DDL at all. A store test covers the behavior the bump would have been ceremony for.

**Why the live-restyle path is the point.** Each option is assigned to the mounted emulator when the
setting changes, not only when a pane is created — a change applies to the terminal the user is looking
at, with no remount and so no scrollback loss or re-attach. This is the failure mode `focus_on_click`
already demonstrates: a setting that persists, moves its switch, and is read by nothing. The vitest
covering these three asserts against the emulator instance that was mounted *before* the edit, so an
option wired only into construction reddens it.

**Effect on parity:** adds `plan/02` **C11**. No row regresses.

---

## D-25 — Keyboard copy/paste, copy-on-select, and a focus setting that finally does something 🟢

**Introduced:** Phase 4 surface, on the branch that adds terminal copy/paste.

**Solo — silent, not contradicted.** `plan/05` records nothing about copying or pasting in Solo's
terminal, nor about how a pane takes keyboard focus. There is no documented Solo behavior to match or
to differ from, so this is a **clean-room addition** rather than a divergence from observed behavior,
recorded here because `plan/05` §12 owns the decision and the parity walk reads it from this file.
Nothing below asserts what Solo does.

**The defect this closes.** xterm ships a `copy` listener, `paste` listeners with bracketed-paste
handling, a `contextmenu` handler that pre-fills its hidden textarea, and — on Linux — middle-click
primary-selection paste. It ships **no keyboard binding for copy**: that is the embedder's job, and
Soloist had not done it, so there was no way to copy terminal output from the keyboard at all.
Separately, `focus_on_click` was a **dead setting** — declared in the appearance document, defaulted,
mirrored in `domain.ts`, with a live switch in the Appearance panel, and read by no code anywhere. It
persisted, its switch moved, and it changed nothing.

**What Soloist does.** Two actions join the closed `HotkeyAction` set in the Terminal scope,
`CopySelection` and `PasteClipboard`, defaulting to **Ctrl+Shift+C** and **Ctrl+Shift+V**. The Shift is
load-bearing: bare Ctrl+C and Ctrl+V belong to the program on the PTY (an interrupt, and a literal
`^V`), and the terminal's capture-phase key handler claims only the Shift chords, so both bare chords
still reach the emulator untouched. The keymap holds **one binding per action**, so the traditional
Ctrl+Insert / Shift+Insert aliases are **not** shipped; adding them would mean reshaping the keymap to
carry alternates, which is a larger change than the aliases are worth. Copy is a no-op without a
selection — an empty write would replace whatever the user had on the clipboard with a blank. Paste
goes through `term.paste`, which normalizes newlines and applies bracketed-paste markers only when the
running program enabled that mode, then emits the result as ordinary input, so no new IPC is involved.

**`copy_on_select`, default off.** A new boolean on the terminal appearance document, driven from
xterm's `onSelectionChange`. Off by default (owner decision): the explicit hotkey stays the primary
path, and Linux middle-click primary-selection paste already works natively either way. The event
fires as a selection is *cleared* as well as made, so an emptiness guard is what keeps a deselect from
wiping the clipboard.

**`focus_on_click` now governs programmatic focus, and its default flips to `true`.** The setting
decides whether selecting a process hands its terminal the keyboard focus; off, the pane is shown and
focus stays where it was, so a click into the terminal is what starts typing. xterm focuses its own
textarea on `mousedown` unconditionally, so clicking the terminal surface always focuses it regardless
— the setting governs the only focus Soloist itself performs. **Both** of the app's focus calls are
gated: the one when a pane is created and the one when a pooled pane becomes visible again. Gating
only the first would leave the setting exactly as dead as it was, because the visible path also runs on
mount. The default moves from `false` to `true` on the same reasoning that kept `cursor_blink` at
`true` in [D-24](#d-24--the-terminal-cursors-shape-and-blink-are-user-settings-not-constants-): the app
has always focused a terminal as its pane was selected, and a fresh install must not silently lose
that. An existing record that already carries `focus_on_click: false` now takes effect, which is the
intended consequence of fixing a setting that was being ignored.

**`rightClickSelectsWord` is turned on.** xterm derives this option's default from "are we on macOS",
read from the installed bundle rather than from memory, so it arrives **off** on our only target. A
right click would then open the context menu over an empty selection, which is the one thing that menu
exists to act on.

**Clipboard access sits behind one seam, backed by the native plugin.** `lib/clipboard.ts` is the
single place the terminal's clipboard is read and written. It goes through
`tauri-plugin-clipboard-manager` rather than the webview's async Clipboard API: WebKitGTK gates
`navigator.clipboard.readText()` behind a user gesture it does not credit a capture-phase key handler
with, so the paste chord could not rely on a webview read. The plugin's commands run in the app
process, where no such gate applies. Neither function rejects: a refused clipboard degrades — a write
is dropped, a read yields no text — so the key handler and the selection listener can never take an
exception.

The grant is the two text commands and nothing else. `capabilities/default.json` carries
`clipboard-manager:allow-read-text` and `clipboard-manager:allow-write-text`, which the generated
`gen/schemas/acl-manifests.json` shows mapping to exactly `read_text` and `write_text`. Image, HTML,
and clear stay ungranted, and `clipboard-manager:default` is deliberately not used — that set is
empty, because the plugin ships no capability enabled by default.

**What this costs and what is still unverified.** The plugin pulls `arboard`, whose `image-data`
feature is on by default and is not switchable from the plugin, so the `image` decode tree compiles
for a text-only use — accounted for in the dependency note below. And the plugin's **runtime
behavior in a real window is still unwalked**: the frontend tests mock the plugin module, so they
prove the seam's wiring and its degradation contract, not that WebKitGTK and the runtime authority
let the call through. That remains the user-only display walk C12 records. Separately, the app's
other copy buttons (scratchpads, project settings, code blocks, Mermaid export) still use
`navigator.clipboard.writeText` — writing is the direction WebKitGTK does permit, and migrating them
was out of scope for the terminal branch.

**What it adds to the dependency graph.** `Cargo.lock` goes from **731 to 757 entries — 26 added,
none removed**: 24 crates that were not present before, plus second versions of `nom` and
`quick-xml`. By what pulls them in, the 26 split into **16 for the Linux clipboard backend**
(`wl-clipboard-rs` with its six `wayland-*` crates, `tree_magic_mini`, `quick-xml`, `nom`,
`os_pipe`, `petgraph`, `fixedbitset`; and `x11rb`, `x11rb-protocol`, `gethostname`), **6 for the
`image-data` decode tree** (`tiff`, `weezl`, `fax`, `half`, `crunchy`, `quick-error`), **2 that
never build on this target** (`clipboard-win` and `error-code`, Windows-only), and **2 for
`arboard` and the plugin themselves**. The bulk of the lockfile cost is therefore the X11 and
Wayland backend — which is the feature, not overhead — and only the 6-crate decode tree is paid
for nothing.

That decode tree is also the only part that is newly *compiled*. `image`, `moxcms`, and `pxfm` were
already lockfile entries at the base, but reachable only through `tauri-plugin-mcp-bridge`, which is
`optional` and absent from `default = ["mcp", "http"]`, so a default build did not compile them and
now does. `png` and `bytemuck` are **not** new cost — a default build already reached `png` through
`tauri` → `tray-icon`/`muda` and `bytemuck` through `tauri-runtime-wry` → `softbuffer`. The
**compiled-size delta is unmeasured**: bundle size is measured against the real `.deb`/`.AppImage` in
the packaging phase, and a number that was not taken is not recorded here.

On the frontend side the plugin is small. `@tauri-apps/plugin-clipboard-manager` is **14.6 kB on
disk** (`dist-js` 10.4 kB, of which the ESM entry the bundler actually pulls is 3,605 B), and its one
dependency, `@tauri-apps/api`, is already a direct dependency of the UI.

**Why there is no schema migration.** `SCHEMA_VERSION` stays **18**, on the same "add a field, not a
store" recipe D-24 records: the settings row persists as one JSON document parsed straight into
`Settings`, whose containers carry `#[serde(default)]` and set no `deny_unknown_fields`, so a new
boolean needs no DDL and a record written before it existed reads back with the default. A bump would
only make `migrate()` refuse the database for an older build, in exchange for no DDL. A store test
covers the behavior the bump would have been ceremony for.

**Effect on parity:** adds `plan/02` **C12**. No row regresses.

---

## D-26 — Terminal links open in the system browser, behind a two-scheme gate 🟢

**Introduced:** Phase 4 surface, on the branch that adds terminal links.

**Solo — silent, not contradicted.** `plan/05` records nothing about links in Solo's terminal, in
either direction. There is no documented Solo behavior to match or to differ from, so this is a
**clean-room addition** rather than a divergence from observed behavior, recorded here because
`plan/05` §12 owns the decision and the parity walk reads it from this file. Nothing below asserts
what Solo does.

**The defect this closes.** A URL in terminal output was inert text. The OSC 8 case was worse than
inert: xterm parses those hyperlinks natively, and with no `linkHandler` set it falls back to a
blocking `confirm()` followed by `window.open()` — read from the installed `@xterm/xterm@6.0.0`
bundle, not from memory. Under the app's CSP (`default-src 'self'`) that cannot reach a remote
origin, so the user got a scary modal and then nothing. Claude Code and other agents emit OSC 8, so
this was on a path the app's own supervised processes take. The app had no way to open a URL
externally at all: no opener plugin, no shell plugin, no `xdg-open` anywhere in `crates/`.

**What Soloist does.** `tauri-plugin-opener` is added to `crates/app` (never `core` — opening a URL
is a UI-shell concern, and CI enforces that core links no app framework). Both link routes end at one
function, `lib/opener.ts::openExternal`: the plain-text route through `@xterm/addon-web-links`, whose
addon is constructed with our handler because **its default handler is `window.open`**, and the OSC 8
route through `linkHandler`. One scheme guard, one call site, so the rule changes in one place.

**The permission is `opener:allow-open-url` carrying its own scope — not `opener:allow-default-urls`.**
Read from the generated `crates/app/gen/schemas/acl-manifests.json`, the two are not competing
spellings of one thing and neither works alone. `allow-open-url` enables the `open_url` command and
ships **no** scope; `allow-default-urls` ships a scope (`mailto:*`, `tel:*`, `http://*`, `https://*`)
and an **empty** `commands.allow`, so it grants no command at all. The capability therefore names
`allow-open-url` and supplies the scope inline, restricted to `http://*` and `https://*` — narrower
than the plugin's default set, which would also admit `mailto:` and `tel:`. `open_path` and
`reveal_item_in_dir` are deliberately left ungranted, so the webview cannot reach them. The pattern
shape matters: the plugin compiles each `url` with the `glob` crate and matches via
`Pattern::matches`, whose `MatchOptions::new()` sets `require_literal_separator: false` — so `*`
crosses `/` and `https://*` matches a full URL with a path and query. Enforcement is in the Rust
process; the webview-side guard is convenience, not the boundary.

**Two schemes, and the reason for each exclusion.** `http:` and `https:` only. `file:` would hand a
local path to the desktop on nothing more than a line of output; `javascript:` and `data:` are script
chosen by whatever wrote that line. A URL printed by a supervised process is untrusted input.
`allowNonHttpProtocols` is left unset — the typings warn that enabling it "may cause security issues
such as XSS", and while it is falsy the emulator drops non-http OSC 8 links before they become
clickable, parsing each URI and refusing to offer any whose protocol is not `http:`/`https:` (read
from the installed `@xterm/xterm@6.0.0` bundle). The web-links addon's own regex matches http(s)
only, so a plain-text `file://` path is never linkified either.

**So neither route can currently reach `openExternal` with a scheme it should not** — on both, the
guard is defence in depth rather than the sole gate. It is kept because the alternative is a rule
that lives entirely in two upstream defaults, in a library the app upgrades: widening the addon's
`urlRegex`, or setting `allowNonHttpProtocols` for a case that seems to warrant it, would each
silently remove a filter with nothing behind it. One guard at the single call site keeps the app's
own answer to "which schemes do we open" in the app, where it can be read and tested.

**Hover reveals the destination, because OSC 8 lets the two disagree.** An OSC 8 hyperlink may
display one string and point somewhere else entirely, which is the classic phishing shape. xterm hands
the handler `getLinkData(id).uri` — the destination, not the displayed cells — and the pane's readout
is fed from that value. It renders in the app's own chrome at a fixed corner of the pane rather than
following the pointer: a program can print anything it likes into the terminal's cells, but it cannot
paint over app chrome. The readout is `pointer-events-none`, so it never takes a click meant for the
terminal. The pointer-cursor affordance needs no work — xterm's own stylesheet already carries
`.xterm-cursor-pointer`.

**Why `linkHandler` is not part of `terminalOptions()`.** That function projects the appearance
document, and every option it returns must also be re-assigned in the live-restyle effect or the
setting silently fails to reach a mounted pane. A link route is not appearance and has nothing to
restyle, so it is passed at construction alongside `scrollback`, which is already handled that way.

**No proposed API, and no CSP change.** `registerLinkProvider` is not behind
`_checkProposedApi()` in xterm 6, so `allowProposedApi` stays off. The opener goes over IPC rather
than a navigation, so `tauri.conf.json` is untouched.

**The ACL is verified as written and parsed, never as enforced.** This is the caveat to carry out of
this branch. What has evidence behind it is that the capability is *well-formed*: it compiles, and
the generated `acl-manifests.json` resolves `opener:allow-open-url` to the `open_url` command. A
scope that is well-formed but matches nothing compiles exactly the same way. Nothing here has
observed the runtime authority admit a single real URL, because the frontend tests mock
`@tauri-apps/plugin-opener` — they drive our handler and record what it asked for, and the plugin's
Rust side never runs. So the failure mode to be aware of is a scope that silently refuses
*everything*: `open_url` compares the URL against the resolved allow-list and returns
`Error::ForbiddenUrl` when it does not match (`tauri-plugin-opener` 2.5.4, `src/commands.rs`), and a
mis-specified pattern would take that branch for every link.

That refusal is invisible twice over. No test exercises the real plugin, and `openExternal` ends in a
`catch` that deliberately swallows the rejection so a hostile link in a process's output can never
throw into the terminal — which means a systematically broken scope is indistinguishable, from
inside the app, from the guard correctly dropping a link. Every link would simply do nothing. **No
headless gate can catch this**: `just lint`, `cargo test` and the vitest suite would all stay green
with the scope pattern wrong. Only the display walk C13 records answers it, and until that walk is
run this capability is wiring that has not been demonstrated to work end to end.

**Effect on parity:** adds `plan/02` **C13**. No row regresses.

---

## D-27 — A file dropped on the terminal inserts its path, quoted, and runs nothing 🟢

**Introduced:** Phase 4 surface, on the branch that adds terminal file drag-and-drop.

**Solo — silent, not contradicted.** `plan/05` records nothing about dragging a file onto Solo's
terminal, in either direction: not whether a drop is accepted, not what a drop does, not whether
anything is inserted. There is no documented Solo behavior to match or to differ from, so this is a
**clean-room addition** rather than a divergence from observed behavior, recorded here because
`plan/05` §12 owns the decision and the parity walk reads it from this file. Nothing below asserts
what Solo does.

**The defect this closes.** Dragging a file onto a pane did nothing at all — there was no
drag-and-drop listener anywhere in the app. Every desktop terminal (GNOME Terminal, iTerm2, Kitty)
answers a drop by writing the file's path at the cursor, and it is the gesture that makes handing a
screenshot to a coding agent a drag rather than a `find`. Soloist's whole purpose is running those
agents, so the pane most likely to be dropped on was the one that ignored it.

**The drop is taken from the OS, not from the DOM.** The window's `drag_drop_enabled` is left at
Tauri's default of `true` — read from the pinned `tauri-utils-2.9.2` `src/config.rs`, "Whether the
drag and drop is enabled or not on the webview. By default it is enabled." With it enabled the
webview handles the drop natively and `getCurrentWebview().onDragDropEvent` hands back **real
filesystem paths**. HTML5 drag-and-drop is not an alternative here and is deliberately not used: a
file dropped through the DOM arrives as a `File` object carrying no path at all, so recovering one
would mean reading the bytes back out to a temporary file to learn where the original already was.

**No new IPC command and no capability change — verified, not assumed.** `onDragDropEvent` is
implemented purely over `this.listen(TauriEvent.DRAG_ENTER | DRAG_OVER | DRAG_DROP | DRAG_LEAVE)`,
read from the installed `@tauri-apps/api@2.11.0` `webview.js` rather than from the docs site. The
capability already grants `core:default`, which resolves through `core:event:default` to
`allow-listen` and `allow-unlisten` in the generated `crates/app/gen/schemas/acl-manifests.json`.
Delivery is `term.paste`, the same route the paste hotkey takes: it emits the text as ordinary input
through `onData` → the existing `pty_write`, so bracketed-paste mode is honored and no separate write
path exists to keep in step.

**One window-wide subscription, not one per pane.** The event belongs to the window, not to an
element — it carries the position it happened at and nothing else identifying a target. Subscribing
per pane would mean six listeners each filtering the same stream, so the app shell owns the single
subscription and routes each event by hit-testing its position against the registered hosts. The
subscription is disposed on unmount; undisposed it would outlive the app's whole session.

**The position is physical, the box is CSS.** `PhysicalPosition` is in real screen pixels while
`getBoundingClientRect()` is in CSS pixels, so the two are converted through
`PhysicalPosition.toLogical(window.devicePixelRatio)` before being compared. Unconverted, the routing
is silently wrong on every HiDPI display — the class of bug that never appears on the developer's
machine. Their **origins** need no correction: the app's title bar is drawn inside the webview
(`decorations: false`) and the shell fills it with no page scroll, so the webview's top-left is the
viewport's.

**A box is half-open, and that is what protects the hidden panes.** Up to six terminals stay mounted
in the keep-alive pool with five of them `display: none`. Containment excludes a box's right and
bottom edges, so a zero-size box — exactly what a `display: none` pane reports — contains no point at
all, and a drop can only ever reach the pane the user can see. This is the mechanism rather than a
guard: an explicit "skip empty boxes" test could not be made to fail on its own, so it is not there.

**Nothing is executed.** No newline is appended. Dragging a file is a request to *refer* to it, not a
decision to run a command with it; auto-submitting a command line the user assembled by accident is
not recoverable, and the alternative costs them one keystroke. Several files insert as several
arguments, separated by a single space.

**Quoting is POSIX single-quoting, in one place.** Each path is wrapped in single quotes, inside
which a shell performs no expansion and honours no escape character — so a space, a newline, a
backslash, a double quote, `$`, a backtick, a glob and a `;` all survive as literal bytes. The single quote is the one
character a quoted run cannot contain, and is spelled by closing the run, emitting `\'`, and
reopening: `'` becomes `'\''`.

**The affordance is a tint and an inset ring, and no label.** While a drag is over the pane it is
marked, and the mark clears on both `leave` and `drop`. Which pane will receive the drop is the only
thing in doubt during the drag; the result — a quoted path at the cursor — explains itself the moment
it lands, and a label would sit over the very output the file is being dropped into. It is
`pointer-events-none`: the drop is handled by the OS rather than by DOM pointer events, so the
overlay never needs to receive one and must never take a click meant for the terminal.

**A pane also gives the mark up as it stops being shown**, which is not the same event as the drag
ending. A pooled pane stays mounted while hidden, so nothing re-runs on the way back to re-derive the
mark; and a drag it was under can end anywhere — over another window, cancelled, dropped somewhere
else — with none of those events addressed to a pane that is no longer on screen. Without giving the
mark up on the way out, the pane comes back marked for a drag that ended long ago. This is the same
shape as the stale hover readout the link work closed, arrived at independently on the drop side.

**Effect on parity:** adds `plan/02` **C14**. No row regresses.

## D-28 — The terminal decorates and counts search matches, clusters graphemes, draws inline images, and honors OSC 52 both ways 🟢

**Introduced:** Phase 4 surface, on the branch that adds the search decorations, unicode and image addons.

**Solo — silent, not contradicted.** `plan/05` records nothing about how Solo's terminal searches
beyond the existence of a find affordance, and nothing whatever about character widths, inline
images, or the clipboard escape sequence. There is no documented Solo behavior to match or to differ
from, so this is a **clean-room addition** rather than a divergence from observed behavior, recorded
here because `plan/05` §12 owns the decision and the parity walk reads it from this file. Nothing
below asserts what Solo does.

**The gate everything else hangs off.** `allowProposedApi` defaults to `false` in xterm 6, and it is
not advisory: reading `terminal.unicode` or calling `registerDecoration` **throws** while it is
unset. Read from the installed `@xterm/xterm@6.0.0` bundle rather than from memory, and asserted
through the emulator rather than the option object — a terminal built from the app's options must be
able to reach those APIs, or the addons silently do nothing. Turning it on widens the surface the app
depends on; only three gated APIs are used, all long-shipped upstream, and this is the recorded
decision to accept that.

**Highlight-all and the match counter are one feature, not two.** The search addon's `_fireResults`
calls `fireResultsChanged(!!searchOptions.decorations)`, and that method returns early on a falsy
argument — so `onDidChangeResults` never fires for a search that decorates nothing. The find bar was
never missing a listener; it was missing the option that makes the event exist. This was verified by
measurement, not inference: with the `decorations` object removed the counter stops updating in every
case that exercises it.

**Clearing the decorations does not report that the matches are gone.** `clearDecorations` drops the
highlights and the tracked results without firing the event, so the count is reset explicitly
alongside it. Without that, closing the find bar and reopening it would show the tally from the
previous query over an empty input.

**Decorating also arms a debounced re-search that was previously dead.** The addon's `_updateMatches`
is guarded on the last search having asked for decorations, so before this change it never ran. With
the find bar open it now re-runs the query 200 ms after the buffer changes, which is what keeps the
tally honest as a live process writes — the count would otherwise describe output that has since
scrolled. It is bounded twice over: debounced, and capped by the explicit `highlightLimit`. What it
costs under a genuinely chatty process is a **real-window question that has not been measured**; it
cannot be characterized headlessly.

**The active match is told apart by its border, not its fill.** Both washes are deliberately quiet —
each is the faintest tint that still reads against the terminal surface — because they tint live
output rather than replacing it. Distinguishing active from inactive by fill alone would mean making
one of them heavy, or leaning on hue, which a colour-blind reader and a grayscale screenshot both
lose. So the accent border carries it, clearing its own fill by 3:1 in both themes. The colours reuse
the app's two existing roles (slate for a found thing, azure for the selected one), which keeps
saturated colour meaning process status and nothing else. A decoration replaces the cell's background
*before* the renderer's contrast pass, so `minimumContrastRatio` still governs program colour drawn
over a match; the fills are nonetheless chosen so the ordinary foreground clears 4.5:1 unaided.

**A theme flip repaints the matches already on screen.** The addon takes its decoration colours as
an argument to a search and offers no way to restyle what it has drawn, so highlights would otherwise
keep the palette of whichever theme was current when the user last typed — the find bar open over a
Light/Dark toggle, or a "System" theme following the OS. The repaint reissues the last query, which
needs the decorations dropped first: given the same query and the same matching options the addon
treats its highlights as current and re-creates only the active one. It reissues through
`findPrevious`, which with that comparison cleared resumes from the *start* of the current selection
and so lands back on the match the user was standing on rather than stepping past it. The addon also
scrolls a match back into view, which a repaint nobody asked for must not do, so the viewport row is
captured and restored around the reissue. **That last guarantee is display-walk-only**: xterm's
viewport does not scroll under jsdom at all — `scrollLines`, `scrollToLine` and the addon's own
scroll are each inert without a measurable surface — so a headless test of it could never fail and
none is written. What *is* asserted headlessly is the repaint itself and the user's place: the border
colour of every decoration on the pane flips to the other palette while the reported match index and
the emulator's selection both stay put.

**The appearance projection and the fixed options were separated so the restyle rule could be checked.** `terminalOptions` had grown five options that never follow the appearance document — the proposed-API gate, the contrast floor, the ruler width, right-click-selects-word and the e2e screen-reader flag — while the comment above the live-restyle effect claimed every option it returns is re-assigned to the mounted emulator. Nothing was dead, because all five are constants, but the rule was false exactly where the next appearance-derived option would quietly become a setting that works on the next pane opened and does nothing to the one in front of the user, which is the defect [D-25](#d-25)'s `focus_on_click` was. They move to `TERMINAL_FIXED_OPTIONS`, spread at construction, and the rule holds again — and is now asserted rather than asserted-in-prose: the test iterates the projection's own keys, so a key added later is covered without anyone remembering to extend a list, over a pair of appearances that is first asserted to disagree about every one of them (including the theme, which is why the pair flips `dark` too) so no option can pass by already holding the right value from construction.

**The overview ruler is given the width the scrollbar already held.** The emulator renders no ruler
at all until a width is set. The fit calculation subtracts the ruler's width *instead of* the
scrollbar's — `overviewRuler?.width || 14` in the installed `@xterm/addon-fit@0.11.0` — so setting it
to exactly 14 leaves the pane's column count, and therefore the PTY winsize, unchanged. Any other
value would silently reflow every pane. `overviewRulerBorder` is set in both themes because xterm
leaves it **black** when unset, which on the light surface draws a hard rule down the pane's edge.

**The unicode addon activates itself; the embedder does not.**
`@xterm/addon-unicode-graphemes@0.4.0` sets `unicode.activeVersion` inside its own `activate()` and
restores the previous version on `dispose()` — read from its shipped source. The widely-assumed extra
assignment by the embedder is therefore not written, because it would be dead code duplicating the
addon's own constant. What is guarded instead is the observable outcome: a ZWJ sequence occupies one
double-width cell rather than three, which is the failure that shears a TUI's columns.

**The image addon's own limits would not fit the budget.** Its defaults are `storageLimit: 128` MB and
`pixelLimit: 16777216`, read from the shipped bundle — its typings' prose claims "2^16", which
contradicts the value the code actually uses and is simply wrong. Both are **per terminal instance**,
and up to six panes stay mounted in the keep-alive pool, so inherited they would permit far more than
the app's whole runtime footprint. Ours are `storageLimit: 16` MB and `pixelLimit: 2048 × 2048`. The
storage figure is not written as a bare 16: it is a 96 MB budget for the whole pool divided by the
pool cap, taken from the constant that sets the cap, so widening the pool tightens each pane instead
of silently raising the app's ceiling. The pixel limit has no accessor to read back, but it is not
unobservable either — a program asking the terminal for the largest graphics geometry it accepts is
answered with the largest square inside the limit, which is 2048 × 2048 for ours against the addon's
own 4096 × 4096.
**Both are proposals, not measurements.** Confirming them needs `storageUsage` sampled in the nightly
soak with a full pool and images loaded, which requires a real display and **has not been run** — so
no figure for actual usage is recorded here. The addon reaches into ten private `_core.*` internals,
so it is pinned exactly; that those internals still line up with xterm 6.0.0 is confirmed by
activating the real addon against a real terminal under test rather than assumed, and must be
re-confirmed on every xterm upgrade.

**Addon loads degrade; they do not throw.** Both heavy addons are fetched with a dynamic `import()`
so each lands in its own bundle chunk, following the renderer addon's existing shape. A chunk that
cannot be fetched, or an addon whose activation throws, leaves a terminal without that one capability
and nothing else — the two are independent, and the pane still renders its output. Their disposers
run before the emulator's, because both reach back into it as they let go.

**The clipboard addon is the deliberate exception to that shape.** It is imported statically, so it
is eager where the grapheme and image addons are code-split. The reason is ordering, not size: it has
to be parsing before the first bytes reach the emulator, and the raw scrollback a pane replays as it
opens can already carry an OSC 52 sequence — a chunk still in flight would miss it, and the miss would
be silent. It is also the smallest of the three. The deviation is recorded here so a later session
reading the two lazy loads beside it does not take the static import for an oversight and "fix" it.

**OSC 52 is granted in both directions, deliberately.** The clipboard addon ships with its default
`BrowserClipboardProvider`, so a program running in a pane can both set the system clipboard and
**read** it — including something the user copied for an entirely unrelated purpose, such as a
password. The emulator offers no way to allow writes while refusing reads short of replacing the
provider outright. This is an **owner decision** (2026-07-27), taken because the panes run commands
the user configured and trusted and the capability is what makes a remote editor or multiplexer yank
into the desktop clipboard at all. It is recorded here specifically so a later session does not read
it as an oversight and quietly narrow it; reversing it is one custom `IClipboardProvider`. This is
separate from the keyboard copy/paste path of [D-25](#d-25), which acts for the user at the keyboard —
this acts for the program at the other end of the PTY. Both reach the same system clipboard.

**The round trip is unverified.** Nothing in the test suite exercises OSC 52: the shared terminal
fake's `loadAddon` is a no-op, so the clipboard addon never activates under test, and no case drives a
read or a write through the escape sequence. Its *release* is safe by construction — xterm registers
its `AddonManager` as a disposable of the terminal, so `term.dispose()` disposes every loaded addon —
but that is the only part of this capability with evidence behind it. The rest is wiring that has not
been demonstrated to work, and it is the one capability here carrying an accepted security cost.

**Effect on parity:** adds `plan/02` **C15**. No row regresses.

---

## D-29 — The terminal names the fonts Ubuntu installs, and the picker offers only those 🟢

**Introduced:** Phase 4 surface, on the branch that corrects the terminal font stack.

**Solo — one recorded string, no recorded fallback.** `plan/05` records that Solo's Appearance tab
has a font-family control (I7f–I7k, read from the demo); the phase inventory notes its description
reads "Monospace fonts installed on your system". What Solo's terminal falls back to when no family
is chosen is not recorded anywhere. So the offered set diverges from a described Solo behavior, while
the fallback stack is a clean-room decision with nothing to differ from.

**The stack named three fonts that do not exist on the target.** It was
`"SF Mono", Menlo, Monaco, ui-monospace, monospace`. Soloist ships Linux-only (D2), and none of SF
Mono, Menlo or Monaco is on a stock Ubuntu box; `ui-monospace` is not implemented by the webview
either. Every entry was therefore skipped and the terminal rendered whatever that particular machine
resolved the bare generic `monospace` to — which is not a decision the app made, and not one it could
predict. It is now `"Ubuntu Mono", "DejaVu Sans Mono", monospace`.

**The evidence is containers, not this machine.** The development host has SF Mono, JetBrains Mono,
Hack and a wall of Powerline fonts installed by hand, so `fc-list` here proves nothing about a user's
box. Three clean images were probed instead:

- On `ubuntu:24.04` carrying **only the app's own runtime closure** — `libwebkit2gtk-4.1-0`,
  `libgtk-3-0t64` and `fontconfig`, installed `--no-install-recommends` — the sole monospace family
  present is **DejaVu Sans Mono**. It arrives because `fontconfig-config` itself depends on
  `fonts-dejavu-core | ttf-bitstream-vera | fonts-liberation | …` and apt takes the first
  alternative. The last *named* family in the stack therefore resolves anywhere the app can run at
  all — a promise the bare generic does not make.
- `fonts-ubuntu` (Ubuntu Mono) and `fonts-liberation` (Liberation Mono) are reachable from
  `ubuntu-desktop` through `Depends` alone on **20.04, 22.04 and 24.04**, the whole D2 range, and are
  listed by the kubuntu / xubuntu / lubuntu metas too.
- With that desktop font set installed, `fc-match` resolves Ubuntu Mono, DejaVu Sans Mono and
  Liberation Mono to themselves, and resolves JetBrains Mono, Fira Code, Source Code Pro, Hack, SF
  Mono, Menlo and Monaco to **Noto Sans** — a proportional face.

**Ubuntu Mono leads, DejaVu Sans Mono follows.** Ubuntu Mono is the Ubuntu desktop's own monospace
face, so on the primary target the terminal wears the platform's own typography; anything else that
can run the app has DejaVu Sans Mono. The generic tail stays as a floor and is never expected to be
the answer.

**The picker offers only what packaging guarantees.** System default, Ubuntu Mono, DejaVu Sans Mono,
Liberation Mono. JetBrains Mono, Fira Code, Source Code Pro and Hack are dropped: nothing is bundled,
and on a stock desktop each resolved to a proportional face, so picking one changed nothing the user
could see — the same shape of failure as a setting no code reads. **Noto Sans Mono is not offered
either**, despite being the obvious fifth: it resolves on 20.04 and 24.04 but falls back to DejaVu
Sans on 22.04, and a family that is only sometimes there is exactly the defect being removed.

**No availability marker is shipped, and the probe that would have driven one is disproven.** The
two honest options for keeping an aspirational family were a static "requires installation" label or
a `document.fonts.check()` probe. The first is false for the developer who *does* have the font. The
second was measured in a real WebKitGTK 2.52.3 webview and **returns `true` for a family that does
not exist** — `document.fonts.check('12px "Totally Not A Real Font 12345"')` is `true`, exactly as it
is for DejaVu Sans Mono. On this port it cannot distinguish an installed family from an absent one,
so the marker it would have driven would have been a lie, which is worse than no marker at all.

**Enumerating the machine's fonts, as Solo's control describes, is not available to the webview.**
Two observations, because either alone would be weaker than it looks: the shipped WebKitGTK library
contains no occurrence of `queryLocalFonts`, and `typeof window.queryLocalFonts` reads `undefined` in
a live webview — that probe page has an opaque origin, so on its own it would also fit an API that is
implemented but gated. Listing a user's real families would therefore mean reading fontconfig in the
Rust process behind a core port. That is a new subsystem, not a picker change, and it is the reversal
path if the fixed list proves too narrow.

**`ui-monospace` is dropped with the macOS families, on measurement rather than inference.** In the
same webview, text set in `ui-monospace` renders at *exactly* the width of text set in a nonsense
family name, while `monospace` renders at DejaVu Sans Mono's width. The port does not implement
`ui-monospace` as a generic — it fell through like any unknown family, which makes it dead weight
in the stack. (`CSS.supports('font-family', 'ui-monospace')` answers `true`, but so does any
arbitrary identifier: the CSS grammar accepts it as a custom family name, so that API cannot answer
this question and the rendered width is what settles it.)

**A family stored before the prune stays selectable.** The core keeps `font_family` as a free string,
so a record written when the list was longer still holds e.g. `"Fira Code"`. A select handed a value
that no item carries renders **empty** — observed under test rather than assumed — which would show
the user's setting as unset while the terminal kept rendering it. The stored name is therefore
appended to the offered options. It is shown plainly, with no claim either way about whether it
resolves on that machine.

**A blank stored family is read as no family.** The field is a free string, so a hand-written record
can hold `""`. The stack already resolved that to the default, but the picker tested the stored name
for `null` rather than for emptiness, so it appended `""` as an option — and a select item with an
empty value is one Radix refuses by throwing, taking the whole settings panel down rather than the
single row it could not draw. Both readers of the field now agree that a blank name is no choice.

**The app shell's `--font-mono` carries the same stack.** The token that inline code, the editor and
Mermaid read held `"SF Mono", Menlo, Monaco, ui-monospace, monospace` — the identical defect on a
different surface. At `main` it and the terminal's stack were byte-identical, so correcting only the
terminal would leave one requirement with two answers and nothing recording which was intended.
**Owner decision (2026-07-28):** correct both together. The consequence is accepted rather than
incidental — code across the whole app now renders in Ubuntu Mono where it rendered in whatever that
machine resolved the bare generic to. `DESIGN.md`, the visual source of truth, names that face as
well; it had named **Geist Mono**, which is not a dependency of this app and has never shipped in it.
The stack stays one named constant per side — `--font-mono` in `index.css`, `DEFAULT_MONO_STACK` in
`lib/appearance.ts` — because xterm is handed a concrete family string, not a CSS variable it could
resolve.

**Effect on parity:** adds `plan/02` **C16**. No row regresses.

---

## D-30 — An agent waiting on you is Important-class, which widens Solo's documented Important set 🟢

**Introduced:** the notifications initiative, recorded ahead of the code that implements it.

**Solo (ref [notifications/indicators](https://soloterm.com/docs/notifications/indicators)):** alerts
are gated by a three-level model — **All / Important / None** — set per-project *and* per-process and
combined by taking the **more restrictive**. The levels are documented as: `All` admits "terminal
alerts plus important process alerts"; **`Important` admits "crashes and auto-restart exhaustion, but
not terminal BEL/OSC alerts"**; `None` "suppresses both terminal alerts and important process alerts
for that scope". The owner approved adopting this model on **2026-07-27**.

**The gap this closes.** Solo's published record never says where an agent *asking for permission*
sits. It names crashes and auto-restart exhaustion as Important and terminal BEL/OSC as All, and stops
there — so an agent blocked on a permission prompt is classified by no documented rule.

**Soloist (owner decision 2026-07-28).** `AgentActivity::Permission` and `AgentActivity::Error` both
map to `Severity::Important`. Both therefore survive at level `Important` and are silenced only by
`None`. Because the documented `Important` set reads as a closed pair — crashes and auto-restart
exhaustion — admitting a third, agent-sourced kind is an **observable widening of that set**, which is
why this sits here and not only in the gap log. That it is *also* a gap in Solo's record is recorded in
`plan/05` §12; this entry owns the divergence half.

**This changes Soloist's own current behaviour.** Today both kinds are gated by the per-project
`terminal_alerts` boolean (`crates/core/src/notify/reactor.rs`, `Attention::permitted_by`) — that is,
terminal-class. Under the level model they move up a class, so a user on `Important` starts receiving
alerts that a user on today's "terminal alerts off" would not have received.

**Rationale.** A blocked agent is the alert whose loss costs most: it is a state a human must clear
before anything proceeds, so a missed one stalls work indefinitely rather than merely going unobserved.
Crashes are self-evidencing after the fact; a silent permission prompt is not.

**What was rejected, and the argument that lost.** Terminal-class was rejected because a user on
`Important` would silently stop learning that an agent is waiting on them — the exact failure the
setting's name promises to avoid. A split making `Error` Important but `Permission` terminal was
rejected as more precise but unsupported by any soloterm.com page, adding a third divergence for no
proven gain. **The counter-argument that lost is alert fatigue** — agents ask for permission far more
often than processes crash, so an Important-class permission prompt is the notification most likely to
push a user to disable notifications wholesale. It is recorded rather than dismissed: if fatigue shows
up in real use, this entry is the thing to reopen, and the cheap remedy is the rejected split.

**Effect on parity:** revises what `plan/02` **D8** delivers (native desktop notifications, crash and
attention). No row regresses. **Implemented** in `AttentionKind::severity`
(`crates/core/src/attention.rs`), pinned by `level_important_keeps_an_agent_asking_for_attention` and
`level_none_silences_an_agent_asking_for_attention`. Delivery itself is still unobserved — D8's
outstanding runtime walk covers that, not this entry.

---

## D-31 — The legacy alert booleans upgrade to a level, and one combination cannot survive it intact 🟢

**Introduced:** the notifications initiative, recorded ahead of the code that implements it.

**Solo — silent, and necessarily so.** The per-project `crash_exit_alerts` / `terminal_alerts` booleans
are **Soloist's own construct**, not Solo's; Solo has only the three-level model. No Solo page describes
this upgrade because Solo never had the thing being upgraded. Nothing below asserts what Solo does. This
entry exists because the migration silently rewrites a user's persisted, deliberately-chosen settings,
which is an observable behaviour change that a later session must not quietly re-decide.

**The problem.** Two booleans express four states; three levels express three. The pair
`crash_exit_alerts: off` + `terminal_alerts: on` means "bells but not crashes", and **no level expresses
it** — `Important` admits crashes and drops bells, `All` admits both, `None` admits neither. Whichever
level is chosen, one of the user's two explicit choices is inverted.

**Soloist (owner decision 2026-07-28).** The serde upgrade shim
(`legacy_booleans_upgrade_to_a_level`) pins the full mapping:

| `crash_exit_alerts` | `terminal_alerts` | level |
| --- | --- | --- |
| on | on | `All` |
| on | off | `Important` |
| off | off | `None` |
| off | on | **`All`** — the lossy case |

**Rationale for the lossy case.** Over-notifying is one click for the user to fix, and it is
self-announcing — the unwanted alert arrives and the user changes the setting. Silently dropping crash
alerts is neither: it is not recoverable, and it is not noticed until the moment it matters, when a
process has already died unobserved. Given a forced error, the decision takes the one the user can see
and undo.

**What was rejected.** `Important` was rejected because it inverts *both* of the user's choices at once
— it admits the crashes they turned off *and* drops the bells they turned on — which is strictly worse
than inverting one. `None` was rejected because it silences a user who deliberately turned bells on,
and is the same unrecoverable-silence failure in a broader form.

**A second loss, in the per-command overrides.** The table maps a project's pair; each stored
`command_terminal_alerts` entry is mapped by the same rule, pairing that command's boolean with the
*project's* crash setting. Under the booleans a per-command override **won outright**, so a command
could be louder than its project; under the level model project and command combine to the more
restrictive of the two, so an override can only ever tighten. A user who had project alerts off and one
command's bells explicitly on therefore loses that command's bells: project `None` clamps it. That is
the level model working as approved, not a bug — but it is a distinct user-visible loss from the
project-level lossy case above, and it is why
`a_per_command_terminal_override_can_re_enable_a_silenced_project` was replaced by
`command_override_tightens_but_cannot_loosen`, which asserts the opposite.

**Effect on parity:** revises `plan/02` **I7c** (project notifications) and `plan/05`'s per-project
settings row, both rewritten to the level model. **Implemented** as the serde upgrade shim on
`ProjectSettings` (`crates/core/src/settings/project.rs`), with the full mapping pinned by
`legacy_booleans_upgrade_to_a_level`. The upgrade has been proven against hand-written legacy JSON
only; it has not been run against a real pre-upgrade data directory.

---

## D-32 — OSC 99 notifications are one-shot only; a multipart payload is ignored, never half-assembled 🟡

**Introduced:** the notifications initiative, recorded ahead of the code that implements it.

**Solo (ref [notifications/triggering-from-scripts](https://soloterm.com/docs/notifications/triggering-from-scripts)):**
Solo accepts three escape sequences for script-triggered notifications — **OSC 9** (iTerm2-compatible),
**OSC 777** (libnotify-compatible), and **OSC 99** (Kitty-compatible). For OSC 99 the page states that
"Solo accepts one-shot payloads and multipart payloads with an `i=<id>` parameter", and that "payloads
can be plain text or base64 when `e=1`". Multipart support is therefore **documented Solo behaviour**,
not merely a capability of the underlying protocol.

**Soloist — a constraint on the parser, now implemented under it.**
`crates/core/src/terminal/parser.rs` accepts all three sequences, and OSC 99 **one-shot payloads in
both encodings** — plain text and base64 under `e=1`. **Multipart is out of scope**: a payload carrying
an `i=<id>` chunk is **ignored outright** rather than partially assembled, and so is one carrying `d=0`,
which the Kitty protocol defines as "incomplete, more chunks follow" — the same half-a-notification this
entry exists to prevent, reached without an identifier. There is no reassembly buffer, no per-id table,
and no timeout reaping half-finished notifications.

**Two further readings this constraint forced, recorded here so they are not silently inherited.**

_An unlabelled one-shot payload is read as the message, not the title._ Kitty defaults `p` to `title`, and
composing a notification with both parts requires the multipart form this entry rules out. Under the
literal default, Kitty's canonical one-shot `printf '\e]99;;Hello\a'` would yield a title with no message
and be dropped, since a notification with nothing to say is not raised. Soloist reads a one-shot payload
as the message either way and lets the surface supply the title from the process's own label — so both
Solo's documented `p=body` example and Kitty's bare form reach the user. Nothing is lost by this: the
alternative silently discards one of the two.

_A payload that is not the notification's text is ignored._ `p=icon` is base64 image bytes, and `p=close`
and `p=buttons` carry no message at all; rendering any of them as notification text would put binary or
control data on screen. Metadata keys Soloist does not recognise are ignored rather than treated as a
reason to drop the message, as the protocol prescribes, so a newer sequence still gets through.

**Rationale.** Chunking exists for payloads too large for a single escape sequence — long bodies and
embedded icons. Nothing in the alerting Soloist actually delivers needs it: a notification is a title
and a short body, and both fit one sequence. Against that, reassembly is exactly the shape the
longevity rules bound most tightly — a keyed buffer, fed by **arbitrary process output**, needing a
cap on both chunk count and total size plus a timeout to reap abandoned ids, or it is an unbounded
buffer any chatty process could grow. Ignoring a chunk is the honest bounded behaviour; assembling
half a notification and showing it would be worse than showing none.

**Why 🟡 rather than settled.** This is a scope decision, not a judgement that multipart is wrong. If a
real payload turns up that needs it, the entry reopens — the work is a bounded per-id buffer with the
caps named above.

**Effect on parity:** `plan/02` row C17 covers the sequences that are implemented; C7 keeps its narrower
meaning (title and bell) rather than being widened to imply this. No existing row regresses.

---

## D-33 — The unread badge rides on `libunity` and silently shows nothing where it is absent 🟡

**Introduced:** the notifications initiative, recorded ahead of the code that implements it.

**Solo (ref [notifications/indicators](https://soloterm.com/docs/notifications/indicators), `plan/05` §10):**
unread attention surfaces in four places — an unread marker on process rows, a dot on the project
header, a title-bar attention count, and an **app-icon badge count**, "capped at `99+`". Solo qualifies
the badge itself as available "on supported platforms"; on its own macOS target it is simply always
there, since the Dock badge is a first-party API.

**Soloist — a constraint on indicators that are not built yet.** The first three are ours to render and
will work everywhere. The fourth will not: Linux has no first-party taskbar-badge API. A badge must be
set through the **Unity launcher D-Bus protocol**, which requires `libunity` present and a shell that
consumes it (GNOME does so via `dash-to-dock` or a comparable extension). Where either is missing the
badge call is a **silent no-op** — no error, no fallback, nothing drawn — and that is accepted rather
than worked around.

**Rationale, and what keeps this honest.** The badge is treated as **additive, never the primary cue**.
The **title-bar count is the always-works indicator**, and every piece of information the badge carries
is reachable from the in-window indicators, so a user on a shell without `libunity` loses redundancy
rather than the signal itself. The alternative — a tray-icon fallback drawing our own badge — was not
taken: it introduces a second attention surface with its own state to keep in sync, for an environment
that already shows the count in the window.

**Why 🟡.** The dependency is environmental, so the behaviour differs across otherwise-identical
installs of our one supported target. It stays flagged until the Phase 13 walk records what the badge
actually does on a stock GNOME session; `libunity.so.9` and `com.canonical.Unity` were both found
present on the development machine, which is **not** evidence about a default Ubuntu install.

**Effect on parity:** constrains `plan/02` **D10** (attention bell + unified unread), promoted to `v1`
alongside this entry. No row regresses. Unimplemented as of this entry — the badge has never been
observed rendering.

## D-34 — The alert sound works on Linux, where Solo's is macOS-only 🟢

**Introduced:** the notifications initiative, with the global Notifications tab.

**Solo (ref [notifications/bell-sounds](https://soloterm.com/docs/notifications/bell-sounds), `plan/05` §10):**
the bell-sound picker is documented as **macOS-only** — it selects from the system alert sounds that
platform exposes, and the feature has no counterpart on Solo's other targets because Solo has none.

**Soloist — the same affordance, working on our one supported target.** The picker offers names from the
**freedesktop Sound Naming Specification** and stores one on the global Notifications document
(`bell: Option<String>`, `None` = silent). The name travels as the `sound-name` hint on the D-Bus
notification, and the desktop's own notification service resolves it against the user's sound theme.
**No audio ships**, and the domain never validates the name against what the backend advertises.

**Why this is a divergence in our favour, and not parity.** Solo's picker cannot work on Linux and ours
cannot work the way Solo's does: there is no system-alert-sound enumeration to offer, and no API that
plays a chosen file for a notification. The freedesktop hint is the platform's own mechanism for the
same intent, so this is the Linux-native form of a macOS-only feature rather than a port of it.

**What the name is, and is not.** It is a **hint**, not a file reference. The spec's lookup truncates an
unfound name at its last `-` and tries again, falling through the theme's parents to `freedesktop`, so
`bell-terminal` reaches a plain `bell` on a theme carrying only that. A theme that resolves none of it
plays nothing **and still shows the notification** — degradation, never failure. The offered set is
therefore grounded in the specification rather than in one machine's installed files, which would make
the list that machine's accident.

**One thing it deliberately does not claim.** A daemon advertising the `sound` capability is not a
promise that a sound is produced: volume, sink state and the user's theme all sit past the point where
Soloist can observe anything. The status row reports what the daemon advertises and nothing beyond it.

**Effect on parity:** satisfies `plan/02` **I7l** (global Notifications tab). No row regresses.

---

## D-35 — A worker's completion is explicit; terminal quiet is not a completion signal 🟢

**Introduced:** the child-agent lifecycle work, 2026-07-31 (branch `feat/child-agent-lifecycle`,
`Done — pending verify` — not yet exercised against a running build).

**Solo (ref `plan/05` §6, §7 + the orchestration demo):** the documented delegation mechanism is
`timer_fire_when_idle_any` / `_all` — a lead arms a timer on the workers it spawned and is woken with
its own pre-written `body` when they go idle. `plan/05` §6 concedes in passing that "a quiet terminal
is not always completed work", but no other completion mechanism is documented: there is no tool by
which a worker reports a result, and none that closes a worker when it finishes.

**Soloist — going quiet is not finishing, and the loop says so.** A worker that has finished and a
worker that is still thinking are indistinguishable from outside, so **nothing derived from terminal
output stands in for the worker saying it is done.** A worker's task is over when it **exits**, calls
**`report_to_lead`**, or **completes its todo** — and each of those is an event, not an inference.
Three additions carry it:

- **`report_to_lead`** (new MCP tool, ours) delivers a worker's final result — success or failure — to
  its lead as a fresh **submitted** turn, in the same header-then-body shape a fired timer wakes an
  agent with, so a wake reads the same whatever produced it. The target is **resolved from recorded
  spawn lineage and cannot be named by the caller**, so the tool reaches only the one agent that
  spawned it, and a caller with no lead is refused rather than defaulted onto someone's terminal. That
  is strictly narrower than `send_input`, which the same worker may already call on any process in its
  project — this adds an *address*, not reach, so `send_input`'s gate is unchanged. The body is capped
  like a timer body and delivered non-blocking, so one deaf lead cannot stall a worker.
- **`close_when_done`** on `spawn_agent`, **off unless asked for.** Nothing previously removed a
  process from the registry on its own, so every worker a lead spawned stayed listed as a `Stopped`
  row for the rest of the session. That is right for a dev server, whose output the user still wants,
  and wrong for the short-lived workers an orchestrating lead runs — so the caller chooses. A failed
  run is still a finished one, so a crashed armed worker is closed too.
- **The spawn preamble and the timer surfaces now say what is true** (`plan/02` **O13**, **O7**). A
  spawned worker is told, as the load-bearing instruction of its opening turn, that it must report its
  final result and that going quiet says nothing. The fire-when-idle tool descriptions and the agent
  guide's timers topic previously read "wait until every worker you spawned is done" — a lead
  following that acts on work nobody did. They now say the watched processes went quiet, which is
  where to *look*, and name the three things that actually end a worker's task.

**Rationale — why not simply make the idle heuristic better.** The heuristic reads a cumulative output
byte count, so it cannot distinguish quiet *before* work from quiet *after* it. Measured against the
live app: a freshly spawned `claude` worker's raw output was byte-identical by SHA-256 for ~6 s,
changed once, then byte-identical for a further ~8 s — the spinner is a static frame, not an
animation. Two independent routes to a false "done" therefore exist (never sampled, and sampled during
a boot-time lull), and **a settle-time constant would only encode a guess about boot duration**;
inventing one was explicitly declined by the owner. Two real defects *were* fixed (D-5): quiet
preceding an agent's first observed activity no longer counts toward the quorum, and a watched process
that exits now ends the wait instead of hanging to the 3600 s backstop. **The noisy-boot case still
fires early**, which is survivable precisely because nothing now treats a fired timer as proof of
completion.

**Effect on parity:** extends `plan/02` **E7** (the delegation loop gains its completion channel),
**F11** (`report_to_lead`, `close_when_done`), **O8** (a lead can now be woken by a worker's own
result, not only by its own pre-written timer body), and constrains **O7** (fire-when-idle is a
"they went quiet" convenience, not a completion guarantee). No row regresses. The residual heuristic
gap is tracked in **D-5** 🟡.
