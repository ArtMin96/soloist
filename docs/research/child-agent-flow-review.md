# `spawn_agent` and the child-agent flow — review, reproductions, and gaps

**Date:** 2026-07-31 · **Branch:** `feat/sortable-projects` · **App under test:** installed Soloist
**v0.11.0** (`/usr/bin/soloist`, pid 1381838), matching repo tag `v0.11.0`.
**Nothing in this report is inferred from documentation alone.** Every claim is either a code
reference read in full, or an observed result from a run against your live app. Where a mechanism
does not exist, it is marked **ABSENT** and backed by the searches that prove it — absence needs
different evidence than a failing test, and the two are labelled separately throughout.

---

## 1. Bottom line

The two behaviours you reported are **not broken — they were never built.**

| You expected | Reality |
|---|---|
| Child agent reports its final result to the parent | **ABSENT.** No channel exists by which a worker's *content* reaches its lead. |
| Child agent auto-closes when done | **ABSENT.** No code removes a process from the registry on exit. |

But the more serious finding is a third thing, which **is** built and **is** broken:

> **The delegation loop's "worker finished" signal is three seconds of terminal quiet.**
> A worker that is still booting satisfies it. I watched Soloist tell a lead *"all 1 watched agents
> are idle"* about a worker that was still printing `connecting…` and had never been given a task.

That is why the loop appears not to work end to end. Adding a report-back tool and an auto-close
would **not** fix it on its own — the lead is woken at the wrong moment regardless.

---

## 2. How I tested

`soloist-mcp` driven directly over stdio JSON-RPC against the running app. Two identities were used:

- **Genuine lead.** This Claude Code session *is* Soloist process **9** (`SOLOIST_PROCESS_ID=9`;
  `whoami` → `origin: {kind: "process", value: 9}`). Its `spawn_agent` calls are real
  lead→worker delegation.
- **Synthetic lead.** A shell inside Soloist Terminal **10**, used for the timer experiments so the
  wake delivery could be captured in a file rather than injected into this conversation. `cat` was
  used as the delivery sink — the same technique the repo's own `crates/pty/tests/orchestration.rs`
  uses.

Workers were launched as `Claude` with `extra_args: ["--version"]` (does a job, exits 0) or `[]`
(interactive). No LLM turns were consumed by any worker.

**Cleanup:** every reproduction process (11, 12, 13, 14, 15, 16, 18, 19) was closed and Terminal 10
was restored to its shell prompt. Worker **17** was left running deliberately so you can inspect the
lineage tree; close it when you are done with it.

---

## 3. ABSENT — never built (proof of absence)

### 3.1 A worker cannot report a result to its lead

The only autonomous write into any process's PTY is the timer scheduler:

```rust
// crates/core/src/coordination/scheduler.rs:166-171
let mut input = format!("{header}\n{}", timer.body).into_bytes();
input.push(b'\r');                                     // submits the turn
let _ = supervisor.try_write_stdin(timer.owner, input);
```

The timer's owner is always the **arming** process (`facade/coordination.rs:303`, `:328` — there is
no owner parameter), and `coordination.rs:317-319` states outright that "a timer only ever delivers
to its own owner." So a worker **cannot** arm a timer aimed at its lead. The text the lead receives
is *the lead's own pre-written body* plus a generated header. **The worker contributes timing only,
never content.**

Enumeration is closed: `grep` for `PtyInput::Write` returns three sites, all in
`crates/core/src/supervisor/terminal_io.rs` + `actor.rs:432`. The only three production callers are
the scheduler above, the explicit `send_input` tool (`facade/scoped_process.rs:82`), and the Tauri
keystroke command (`app/src/commands/mod.rs:286`). Neither `crates/httpapi/src` nor `crates/cli/src`
can write to a PTY at all.

Coordination state is **pull-only**. `KvChanged` / `ScratchpadChanged` / `TodoChanged` are published,
but their sole non-test consumer is `app/src/lib.rs:149`, which forwards them to the webview. A lead
must poll.

**One unguarded path does exist.** `ScopedFacade::send_input` (`facade/scoped_process.rs:75-92`) is
gated only by `require_in_scope` — same project, no lead/worker distinction — so a worker *can*
write a submitted turn into its lead's terminal today. It is reachable but not addressable: **no MCP
tool exposes lineage.** `grep -rn "lineage|parent" crates/mcp/src/` (non-test) → **zero hits**;
`whoami`'s reply schema has no parent/lead field. A worker cannot learn which process its lead is.

### 3.2 Nothing closes a finished worker

`grep -rn "remove_returning_handle" crates/` → 3 hits: the definition, `supervisor/lifecycle.rs:174`,
one test. **Zero in `supervisor/actor.rs`** — the exit path. `ProcessRemoved` is published only from
`lifecycle.rs:157` and `:190`, neither reachable from an exit. Every removal is explicit
(`close_process`, project removal, config reload dropping a command).

**Reproduced twice, including on a genuine lead→worker pair:**

```
# this session (process 9, a real bound lead) spawns worker 16
spawn_agent {"tool":"Claude","extra_args":["--version"]}  ->  {"process": 16}
# worker prints its result and exits cleanly
get_process_status {"process":16}
  {"exit_code": 0, "id": 16, "kind": "Agent", "status": "Stopped", ...}
```

The worker did its job, exited `0`, and **remains in the registry as a `Stopped` row** with its
terminal scrollback retained. Its MCP session binding is also never closed — `identity.close` is
called only when the MCP connection ends (`facade/session.rs:104`), never from a process exit.

### 3.3 A spawned worker is told nothing

The complete set of Soloist-supplied inputs to a worker is **one environment variable**,
`SOLOIST_PROCESS_ID` (`supervisor/actor.rs:162-164`). No prompt, no preamble, no identity of its
lead, no instruction to report back or close itself.

- `grep -rni preamble` over `crates/` and `e2e/` → **zero hits.**
- `SpawnAgentArg` is exactly `{ tool, extra_args }` (`mcp/src/args.rs:57-65`). **There is no `prompt`
  parameter anywhere on the path.**
- This is tracked but unbuilt: `plan/02-feature-parity-matrix.md:257` (row **O13**, the
  `[SOLO ORCHESTRATION CONTEXT]` preamble) and `PROGRESS.md:2715-2716` — *"today only
  `SOLOIST_PROCESS_ID` is injected."*
- `setup_agent_integration` writes `AGENTS.md`/`CLAUDE.md` containing `agent_guide()`
  (`support/guide.rs:264-271`). I read all 13 topics: nothing tells a worker it has a lead, must
  report, or should close itself. The nearest text is lease hygiene — *"release what you hold when
  you are done"* (`guide.rs:172`).

The repo's own contract agrees. `crates/pty/tests/orchestration.rs` spawns its worker with
`extra_args: Vec::new()` and the worker stub is `printf 'WORKER STARTED\n'; exec sleep 600` — it
never reports and never closes itself. Completion is inferred from terminal quiet.

---

## 4. BROKEN — built and defective (reproduced)

### 4.1 🔴 Critical — "worker is idle" fires on a worker that has not started working

`AgentActivity::Idle` means *no new bytes for 3 samples* — `IDLE_AFTER_QUIET_SAMPLES = 3`
(`agents/idle/strategy.rs:17`) × `SAMPLE_INTERVAL = 1s` (`agents/idle/sampler.rs:26`), over a raw
byte-count delta (`strategy.rs:72`). `watched_is_idle` (`coordination/timer.rs:112-118`) then treats
`Idle` as *done*. There is no `Finished` state — `AgentActivity` has exactly five variants
(`core/src/idle.rs:22-36`).

**Reproduction (verbatim, live app).** Lead spawns an interactive worker and immediately arms the
timer the agent guide tells it to arm:

```
### interactive WORKER=15 spawned at 1785488804
get_process_status {"process":15}   ->  {"status": "Running", ...}
### arming fire_when_idle_all on the just-spawned worker at 1785488804
timer_fire_when_idle_all {"body":"WAKE4","processes":[15],"max_wait_ms":600000}
  ->  {"already_idle": false, "waiting_on": [15], "timer": {"id": 2, "owner": 10, ...}}
```

The arm was **correct** — worker not idle, `waiting_on: [15]`. Then, ~4–8 s later:

```
get_process_status {"process":15}  ->  {"status": "Running", "exit_code": null, ...}
get_process_output {"process":15}  ->  {"output": ["... 5% ⏱ ... │ ◑ xhigh/rc connecting…──────"]}
timer_list {}                      ->  {"timers": []}          <-- already fired
```

and the delivered wake, captured in the sink:

```
[Soloist timer #2] all 1 watched agents are idle
WAKE4
```

**The worker was still `Running`, still rendering `connecting…`, and had never been given a task.**
Soloist reported it as idle and woke the lead. This is a false-positive completion signal, not a
race at arm time.

**The mechanism, measured directly.** A rendered spinner suggests bytes are always flowing, which
would prevent the quiet count from ever reaching 3 — so I measured it rather than assuming. Freshly
spawned worker 19, polling the SHA-256 of its raw output once a second:

```
t=1785489922.32 sha=c261f360fe8f     t=1785489928.65 sha=0ce3ce0de959
t=1785489923.37 sha=c261f360fe8f     t=1785489929.71 sha=0ce3ce0de959
t=1785489924.43 sha=c261f360fe8f     t=1785489930.78 sha=0ce3ce0de959
t=1785489925.49 sha=c261f360fe8f     t=1785489931.84 sha=0ce3ce0de959
t=1785489926.55 sha=c261f360fe8f     ...unchanged through...
t=1785489927.60 sha=c261f360fe8f     t=1785489936.06 sha=0ce3ce0de959
```

The output is **byte-identical for ~6 s, changes once, then byte-identical for a further ~8 s.** The
spinner is a static frame, not an animation. So a booting agent really does produce quiet windows far
past the 3-sample threshold, and `OutputDelta` classifies it `Idle`. An independent run polling raw
output length reproduced the same shape (constant for ~7 s, one jump, constant for ~5 s).

A second timer run showed the degenerate case: arming on a worker spawned in the **same second**
returned `"already_idle": true` and fired instantly — a not-yet-sampled agent defaults to `Idle`
(`agents/idle/classifier.rs:35`, pinned by the test `a_quiet_agent_first_emits_idle`). So there are
two independent routes to a false "done": never sampled, and sampled during a boot-time lull.

**The symmetric failure also exists in code** (not separately reproduced): the scheduler folds only
`AgentActivityChanged` and `ProcessRemoved` (`scheduler.rs:118-135`) and **ignores
`ProcessStatusChanged`**. A worker that exits while last classified `Working` leaves
`watched_is_idle(Some(Working), true) == false` **forever**, so a `fire_when_idle_all` waits out
`DEFAULT_IDLE_MAX_WAIT = 3600 s` (`timer.rs:42`). No test covers exit-while-working
(`grep "ProcessStatusChanged" crates/core/src/coordination/scheduler_tests.rs` → only a generic
helper). So the same mechanism can fire an hour early *or* an hour late.

**Contract divergence.** `timer.rs:110-111` claims the scheduler and the façade share one definition
"so what is reported matches what fires." They read different sources: the façade reads the live
`IdleTracker` (reset to `None` on exit, `sampler.rs:106`); the scheduler reads its own stale event
fold. For an exited-but-registered agent the two disagree.

**Provider coverage is uneven** (code-read, not reproduced): `TitleStability` (Codex, Amp) and
`TitleStatus` (Gemini) never call `looks_like_permission_prompt` — it has exactly one call site,
`strategy.rs:85`, inside `OutputDelta`. A Codex/Amp worker **blocked on a permission prompt reads as
`Idle`**, and `AgentPermission` attention can never fire for those providers. `strategy.rs:99-100`
concedes the converse: *"Providers that never set a title read as idle."*

### 4.2 🟠 High — a bind failure is silent and unrecoverable

```rust
// crates/mcp/src/client.rs:108-112
if let Some(process) = self.bound {
    // A bind failure must not fail the connection — whoami simply reports unbound.
    let _ = exchange(&mut stream, &IpcRequest::BindSessionProcess { process }).await;
}
```

The result is discarded. `Origin` has three variants (`identity.rs:27-35`), so **"tried and was
refused" is indistinguishable from "never tried"** — both read `Unbound`. There is **no
`bind_session_process` MCP tool** (`crates/mcp/src/tools/identity.rs`, whole file: only `whoami`,
`register_agent`, `select_project`, `select_process`), so an agent cannot retry or diagnose.

This matters because an unbound session **silently loses the whole coordination layer**: no lineage
recorded on spawn, no timers, no leases, no todo locks.

**Reproduced.** A script run inside Soloist Terminal 10 inherited `SOLOIST_PROCESS_ID=10` correctly,
yet `whoami` returned `origin: {kind: "unbound"}`. Cause: binding is authenticated by the connecting
peer's **process group** (`facade/session.rs:82-86`; `app/src/peer_cred.rs:46`), and an interactive
shell's job control puts every command in a *new* process group. Re-running the identical script
after `set +m` bound successfully (`bound_process: {id: 10, ...}`).

**Practical consequence:** an agent the user starts *by hand inside a Soloist terminal* inherits the
terminal's `SOLOIST_PROCESS_ID`, fails the pgid check, and operates unbound — with no error anywhere.
Its `spawn_agent` calls then produce **root processes, i.e. siblings, not children.**

Related: the registered pgid is **asserted, not read** — `supervisor/actor.rs:258` reuses the child
pid as a pgid, justified by a doc comment (`pty/src/lib.rs:11-13`) rather than a `getpgid` call,
while the peer side genuinely kernel-reads. The two agree only while the assertion holds.

### 4.3 🟠 High — gate/record asymmetry drops known lineage

`spawn_agent` resolves the caller from **two** signals to decide whether to *refuse* it:

```rust
// crates/core/src/facade/scoped_process.rs:120-126
let caller_is_worker = [ self.home_process(),
                         self.inner.identity.origin(self.session).process() ]
    .into_iter().flatten()
    .any(|caller| self.inner.lineage.parent_of(caller).is_some());
```

…but records lineage from **`origin` only**:

```rust
// crates/core/src/facade/scoped_process.rs:133-135
if let Some(lead) = self.inner.identity.origin(self.session).process() {
    self.inner.lineage.record(worker, lead);
}
```

So a caller whose peer group resolves to a managed process but which never bound is recognised well
enough to be **gated as a worker**, yet its own spawns record **no parent** and render as roots. The
app knows who the caller is and throws the information away. Adding `home_process()` as a fallback at
line 133 closes it.

### 4.4 🟡 Medium — the sidebar flattens a worker whose lead is not an Agent

The two surfaces nest by different rules. `crates/app/ui/src/store/grouping.ts:86-90` resolves
nesting **only within one kind bucket**:

```ts
const roots = nestByLineage(processes.filter((p) => p.kind === kind), parents);
```

The Orchestration pane has no such filter (`facade/orchestration.rs:65-87`,
`store/orchestrationTree.ts:15-31`), and core's `lineage_edges()` emits the edge regardless of kind.

So when the lead is an **Agent** (the normal case) both surfaces agree. When the lead is a
**Terminal** — someone running `claude` inside a Soloist terminal — the worker is `ProcessKind::Agent`
and lands in a different bucket: **the sidebar shows it flat while the Orchestration pane nests it.**
Also `Sidebar.tsx:86` filters before grouping, so a filter query matching a worker but not its lead
re-roots the worker.

> **This is almost certainly what you saw.** My earlier reproduction bound to Terminal 10, so those
> workers were children of a *Terminal* — flat in the sidebar by rule 4.4 — and two others (11, 12)
> were genuine roots because they came from `soloist-cli spawn` and an unbound session. Workers 16
> and 17 were spawned by this session (process 9, an Agent) and should nest in **both** surfaces.

### 4.5 🟡 Medium — lineage is per-run and invisible outside the Tauri UI

`AgentLineage` is one `Mutex<HashMap<ProcessId, ProcessId>>` (`agents/lineage.rs:26`), never
persisted — `grep -rn "lineage" crates/store/` → zero hits. This is *not* a restart-flattening bug
(process ids are per-run, so a restart loses the processes and their edges together), but it does
mean lineage cannot inform anything durable.

More importantly it is exposed **only** through the Tauri command `lineage_edges`
(`app/src/commands/orchestration.rs:32`). No MCP tool, no HTTP route. **An agent can never see the
tree it is part of.**

---

## 5. Adjacent — real, but outside this feature

- **`prompt_mode` is dead state.** `AgentTool::prompt_mode` (`agents/tool.rs:68`) is written,
  mirrored into `domain.ts:178`, surfaced in the Settings panel, and returned by the
  `list_agent_tools` MCP reply — but `grep -rn "\.prompt_mode" crates/` (excluding `node_modules`)
  returns **zero read sites** workspace-wide. `PromptMode::Stdin` has no implementation.
  `launch_agent` never consults it. This violates `CLAUDE.md` §15 (no dead code).
- **Inconsistent `already_idle` in one reply.** `timer_fire_when_idle_all` returned outer
  `"already_idle": true` alongside nested `"timer": {"already_idle": false}` — the nested aggregate
  is un-enriched (`facade/orchestration.rs:105-108` enriches only on the list path).
- **Ports go stale after exit, contradicting the documented contract.**

On exit, `actor.rs:290` calls `registry.set_pgid(id, None)`, which clears `ports` and resets `ready`
**silently** — `monitoring.rs:52` states "clearing the gate on stop happens in the registry and is
silent." But `events.rs:76-77` documents that ports are "emptied when it stops." No `PortsChanged` is
emitted, and the port scanner iterates `live_groups()` only, so a delta-folding consumer keeps stale
ports until a full snapshot re-read. **Code and doc-comment disagree; the doc is wrong.**
- **Five `soloist-mcp` processes were resident** at session start. Not investigated; may be normal
  per-client sessions, may be leaked stdio servers. Flagging only.
- **No regression guard on the grandchild bind path.** `e2e/fixtures/lead-agent/src/main.rs:4-8`
  connects as the group leader itself; its comment claiming it binds "exactly as `soloist-mcp`
  binds" is not asserted under test. The path I measured by hand is untested.

---

## 6. What is worth having (not implemented — your call)

Ordered by how much each unblocks. **I have not written any of this.**

1. **A real completion signal.** `Idle` conflates *quiet* with *done*, and that needs **two**
   separate changes — one does not cover the other:
   - *The hour-late hang:* fold `ProcessStatusChanged` into the scheduler
     (`scheduler.rs:118-135`) so a worker that exits counts as finished. Small and contained. It does
     **nothing** for the false positive — a booting worker never emits a status change.
   - *The false positive:* a worker must not read as "done" before it has demonstrably started. That
     means not treating a never-sampled or never-yet-active agent as `Idle` for quorum purposes
     (e.g. require an observed `Working` before `Idle` can mean finished). This is the change that
     actually makes delegation work.
2. **`report_to_lead` (or `worker_result`) MCP tool** — a scoped tool letting a worker hand a
   payload back, delivered to its lead as a fresh turn via the existing `try_write_stdin` path.
   Requires exposing lineage server-side (the data is already in `AgentLineage`).
3. **Auto-close policy** — most naturally an opt-in `close_when_done` flag on `spawn_agent`, so the
   default stays "the user can read the worker's scrollback." A blanket auto-close would destroy
   output the user may want.
4. **The O13 spawn preamble** — already scoped in the matrix; it is what would tell a worker it has
   a lead and what is expected of it. Points 2 and 3 are much less useful without it.
5. **Surface bind failures** — return the bind error instead of `let _ =`, and add an `Origin`
   variant (or a `bind_error` field on `whoami`) so "refused" is distinguishable from "never tried".
6. **Close the gate/record asymmetry** (§4.3) — a one-line fallback to `home_process()`.
7. **Nest across kind buckets in the sidebar** (§4.4), or state the flattening as intended.

---

## 7. Confidence and limits

- Everything in §3 is **proof of absence** (grep patterns + missing consumers), not a failing test —
  there is nothing to make fail.
- §4.1 and §4.2 are **reproduced against the live app**, transcripts quoted verbatim.
- §4.1's exit-while-working hang, and the Codex/Amp permission-vs-idle collapse, are **read from
  code and not separately reproduced.** They are marked as such above.
- I did **not** verify what a *live* worker's `whoami` returns from inside the worker's own process
  group; the claim that a worker cannot identify its lead rests on the reply schema having no such
  field and on zero lineage hits in `crates/mcp/src`.
- No claim here rests on `plan/` docs. Where docs are cited (O13, `PROGRESS.md`) it is to show the
  gap was already *known*, not to establish behaviour.
