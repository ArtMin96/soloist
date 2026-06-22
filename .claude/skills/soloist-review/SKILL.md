---
name: soloist-review
description: Comprehensively research, review, and verify a set of Soloist changes — a phase's work, a commit range, a PR branch, or the current uncommitted diff — against the architecture, security, clean-room, and codebase-discipline contracts. Use when the user wants to review, audit, or sanity-check Soloist code before merge. Read-and-verify only — it grounds itself in the plan docs, pins the exact diff, researches official docs before judging (never assumes), runs the real gates, and reports findings by severity. It does NOT build features and does NOT change code unless explicitly told to after the findings. First-class checks span domain boundaries & separation (hexagonal + bounded contexts), duplication / single-source / DRY, correctness & bugs, security gaps, performance & size, comment/clean-room discipline, tests, and docs — and it consults the official Tauri docs plus the project's tauri-* skills before judging any Tauri surface.
version: 1.0.0
user-invocable: true
argument-hint: "[review target — a commit, a range, a phase name, or 'the current diff'; blank = the latest commit on the current branch]"
---

# Soloist review

You are **reviewing** Soloist — a clean-room, native-Linux rebuild of the macOS app **Solo**
(`soloterm.com`). Your job is to **research, review, verify, and report** on the changes the user named.
Find real problems, prove or disprove the change's claims, and hand back a graded report.

## What to review — resolve the target first

The **review target** is whatever the user typed after `/soloist-review` (the argument passed to this
command). Read that argument now and classify it — do not assume:

- a **commit** (a sha, `HEAD`, `HEAD~2`) → review that commit (`git show <ref>`);
- a **range or branch** (`main..HEAD`, `origin/main..`, a branch name, a PR) → review that range
  (`git log --oneline <base>..<head>` + `git diff <base>..<head>`);
- a **phase** ("Phase 7", "the agents work") → find its commits via `PROGRESS.md` + `git log` and review them;
- **"the current diff" / "uncommitted" / "staged"** → review the working tree (`git diff`, `git diff --staged`);
- **nothing given** → review the most recent commit on the current branch (`git show HEAD`).

If the argument is **ambiguous** — you can't tell which commits or files it means — do **not** guess. Run
`git log --oneline -20`, `git status`, and `git branch --show-current`, state your best interpretation in your
opening line, and confirm it before reviewing. The point is that you always **state precisely what you are
reviewing** (which refs, which files) before judging anything; Step 2 pins it exactly.

## The one hard rule

This is **read-and-verify**. You **propose** fixes; you do **not** apply them, and you do **not** build
features — unless the user explicitly tells you to after seeing your findings. Never weaken or delete a
test to make a gate pass. Never report a gate green you did not run and see. If two docs contradict each
other, **stop and surface it** (CLAUDE.md §2/§12) — never resolve a contradiction by guessing.

---

## Step 1 — Ground yourself (CLAUDE.md §1, in order)

Read, before judging anything:
1. **`PROGRESS.md`** (incl. the "Critical details" and "Decisions" sections) — the state ledger and the
   claims you must verify.
2. **`CLAUDE.md`** in full — the operating rules; they override default behavior.
3. **`plan/05-solo-reference-and-sources.md`** — the behavior contract. Every claim about how Solo behaves
   must trace to here (with its confidence marker ✅🟡❓). If it isn't here, it is a **recorded gap**, not a bug.
4. **`ARCHITECTURE.md` → `plan/04-engineering-architecture-and-patterns.md` → `plan/06-codebase-blueprint-and-cleanup.md`**
   — the design + structural contracts (the §16 invariants).
5. The **phase file** `plan/phases/phase-NN-*.md` for the work under review and its rows in
   **`plan/02-feature-parity-matrix.md`** (the per-feature scope/verify contract — v1 vs `later`).

## Step 2 — Pin the exact diff (no ambiguity)

Resolve the target with git and **state precisely what you are reviewing and every file it touches**:
- a commit → `git show <sha> --stat` then the full `git show <sha>`;
- a range / phase / branch → `git log --oneline <base>..<head>` + `git diff <base>..<head> --stat` + the diff;
- "the current diff" → `git status` + `git diff` (and `git diff --staged`).
List the files grouped by crate/layer (core / store / pty / sys / app / mcp / httpapi / cli / ui / docs), so
the boundary review (Step 4·1) has a map. Cross-check the diff against what `PROGRESS.md` *claims* changed —
flag any drift between the ledger and reality.

## Step 3 — Research first; never assume (CLAUDE.md §4)

For anything touching an external surface, consult the source **before** forming a judgment, and **name the
source** in your finding:
- **Tauri** → `tauri.app/llms.txt` + the **`context7`** MCP (Tauri v2), **and the project's `tauri-*` skills
  (mandatory — see Step 3a).**
- **Claude Code / MCP / Agent SDK** → `code.claude.com/llms.txt`.
- **Any other library API** (tokio, rmcp, portable-pty, notify, axum, rusqlite, globset, …) → `context7`
  (`resolve-library-id` → `query-docs`) or the crate's docs. Verify a version/flag/behavior; don't trust memory.
- **Solo behavior** → `plan/05`. A "this should match Solo" claim is only valid if `05` documents it; an
  undocumented behavior is a **gap with our recorded decision**, not a defect.

If you write "this is wrong" about an API or config, you must have looked it up. "Probably" is not a source.

### Step 3a — Consult the Tauri skills for every Tauri surface (mandatory)

Whenever the diff touches the **Tauri shell** (`crates/app`, `tauri.conf.json`, capabilities, a Tauri plugin,
a command/event, the sidecar, packaging), **invoke the matching project-local `tauri-*` skills before judging
that code**, and in each finding name the skill and what it confirmed or flagged. Use this map (invoke a few —
at least the baseline, plus every skill matching a surface the diff actually changes):

| Surface in the diff | Invoke |
|---|---|
| **Baseline — any `crates/app` / `tauri.conf.json` change** | `tauri-architecture`, `tauri-configuration` (confirm the change belongs in the Tauri shell, not `core`; that config keys are real) |
| Commands / IPC / events (`#[tauri::command]`, `invoke`, `emit`, Channel) | `tauri-ipc`, `tauri-calling-rust`, `tauri-calling-frontend`, `tauri-frontend-events` |
| Capabilities / permissions / plugin ACL / scopes / CSP / headers | `tauri-capabilities`, `tauri-permissions`, `tauri-plugin-permissions`, `tauri-runtime-authority`, `tauri-scope`, `tauri-csp`, `tauri-http-headers` (least-privilege) |
| The `soloist-mcp` sidecar / a node helper | `tauri-sidecar`, `tauri-nodejs-sidecar` |
| Packaging / bundle size | `tauri-linux-packaging`, `tauri-binary-size` |
| Window shell / tray / splash / resources | `tauri-window-customization`, `tauri-system-tray`, `tauri-splashscreen`, `tauri-app-resources` |
| Build/release pipeline, signing, deps, migration | `tauri-pipeline-github`, `tauri-code-signing`, `tauri-updating-dependencies`, `tauri-migration` |
| Lifecycle / supply-chain security | `tauri-lifecycle-security`, `tauri-ecosystem-security` |

If the diff genuinely touches **no** Tauri code, say so explicitly and still consult **`tauri-architecture`**
once as a guardrail — to confirm nothing in the change *should* have lived in (or leaked into) the Tauri layer.

## Step 4 — Review across every dimension and collect findings

Go dimension by dimension. For each finding, capture `file:line`, the problem, and a concrete proposed fix.

1. **Domain boundaries & separation (the headline check — `plan/04` §1/§3, `plan/06` §16).**
   - Run **`scripts/check-core-deps.sh`** and confirm `crates/core` imports **no** adapter framework
     (`tauri`/`rmcp`/`axum`/`rusqlite`/`notify-rust`). Dependencies point **one way**: adapters → `core`/`ipc`,
     never the reverse.
   - Business logic lives in its **bounded context** (C1–C8) behind a **port**, exposed through the **one
     `Facade`**. Flag any domain `if`, status comparison, or trust/restart decision living in an **adapter**
     (`app`/`mcp`/`httpapi`/`cli`) or in **React** — those must route to one core command. Flag any behavior
     **reimplemented per adapter** instead of one `Facade` method.
   - Bounded contexts don't reach into each other's internals; **no cycles**; adapters touch **C8 only**.
   - **A context owns its own port** (the newer convention): flag a new **driven** port added to the shared
     `core/ports/mod.rs` god-file instead of living in its context module with a `Noop` default (the R7 drift).
   - React holds **no** business logic: `domain.ts` (types) · `api.ts` (typed IPC only) · `store/` (pure
     reducers) · `components/` (presentational). Flag logic in a component or an `invoke` outside `api.ts`.
   - **Small, single-purpose files**: flag a non-test source file past the ~400-line smell
     (`scripts/check-file-size.sh`); flag tests merged into the implementation file instead of a sibling
     `*_tests.rs` / `tests/`.

2. **Duplication, single source of truth & DRY (CLAUDE.md §15, `plan/06` §6).**
   - Every status / kind / event / command / limit / path is defined **once** — a Rust enum in `core`, the TS
     mirror in **one** `domain.ts`, one command/event-name **constant per side** (the Rust↔TS pair is the only
     sanctioned duplication). Flag a second definition that can drift.
   - **No magic strings or numbers** — a bare status/kind/state string or a bare numeric limit/timeout at a
     comparison or emission site is a finding; it must be the enum or a named `const`.
   - **No copy-pasted behavior** across adapters/contexts; shared logic is one helper; shared **test fakes** live
     once in `core::testing` (not re-rolled per crate). Flag a persisted/wire encoding that re-declares the
     domain shape instead of deriving from it.

3. **Correctness & bugs.** Logic errors, off-by-ones, unhandled edge cases, error handling (typed errors at
   boundaries, no `unwrap`/`expect`/`panic` in long-running tasks — clippy-denied in `core`), concurrency races
   (exit-vs-stop, double-kill, stale read-model writes after a group ends), FSMs as explicit
   `Result<New, IllegalTransition>` transitions (no ad-hoc field mutation), lifecycle/cleanup
   (PTYs/FDs/process-groups reaped; subscriptions/timers/watchers dropped on close; a start/stop loop ends at the
   same PID/FD/task count). Verify the change's own claims with a test or a command where you can.

4. **Security gaps (`plan/04` §12).**
   - **Trust gate enforced in `core`** (not the UI) for `start*`/`restart*`/auto-*, per `(project,
     command-variant hash)`. Confirm no path (UI, MCP, HTTP, auto-start, file-watch, crash-restart) can run an
     untrusted variant.
   - **Scope isolation**: no adapter or MCP/HTTP tool can reach **another project's** state; effective project
     scope + identity (`SOLOIST_PROCESS_ID` / `bind_session_process` / `register_agent` / `whoami`) honored.
   - **Tauri ACL least-privilege**: capabilities/permissions grant only what's used (cross-check with the
     Step 3a skills); CSP present; `freezePrototype` stays `false` (breaks xterm). HTTP API loopback-only +
     `X-Soloist-Local-Auth` on mutations + localhost CORS.
   - **Env & credentials**: child env sanitized (only `SOLOIST_PROCESS_ID` injected; documented precedence);
     Soloist stores/injects **no** agent API key or OAuth token (agents use their own auth — E8).
   - **Subprocess / input handling**: any spawned command (probes, agents, the shell) — is the input trusted,
     bounded, timed-out, and **reaped**? Any path-from-config or path-from-IPC validated? No command injection,
     no unbounded buffer/channel/retry, no panic-as-control-flow at a boundary.

5. **Performance & size (CLAUDE.md §6).** Bounded buffers/channels/retries everywhere; chatty output coalesced
   per frame; no needless clones/allocs in hot paths (the PTY read loop, event fan-out); metrics sampled on an
   interval not per event; release profile (LTO + `codegen-units=1` + strip) intact; bundle/RSS budget respected.
   **Measure, never invent a number** — if a footprint/size claim is made, it must be measured or marked unknown.

6. **Comment & clean-room discipline (CLAUDE.md §8/§9).** Doc comments on public items + the rare *why*-note
   only. **Flag every** phase number, `R`-phase tag, `plan/§` citation, changelog/progress narration,
   `placeholder`/`TBD`, or comment that merely restates the code — **in comments AND in names** (no `r1_*`,
   `phase5_test`). Note: the C1–C8 bounded-context IDs are sanctioned vocabulary, not phase numbers. Clean-room:
   no copied Solo source/assets/strings/branding; MCP tool *names* may mirror Solo but their schemas are ours;
   no Solo asset committed (e.g. `processes.webp`, a reference `solo.yml`).

7. **Tests & gates (actually run them).** Run **`just lint`** (rustfmt, clippy `-D warnings`, tsc, ESLint,
   Prettier, dependency-direction guard, file-size guard) and **`just test`** (cargo + vitest) and **report the
   real output** — pass/fail counts, not a claim. Confirm new behavior has **honest** tests (each can fail for a
   real reason; no tautological/placeholder tests), tests are **deterministic** (mock `Clock`; no real-time/race
   flakiness — re-run a suspicious suite a few times), and the soak gate (Phase 6+) is intact. A flaky or
   pretend test is a finding.

8. **Docs & ledger (CLAUDE.md §10/§11).** `PROGRESS.md` updated with **evidence** (test names, counts, commit
   shas) and an accurate "next session" pointer; plan docs consistent (a lower doc must not contradict a higher
   one — `04`>`06`, `05`/`02` over a phase file); `KNOWN-DIVERGENCES.md` carries any new intentional Solo
   divergence; `CLAUDE.md` §14 commands current. Flag stale or contradictory docs.

## Step 5 — Report

Group findings by severity, each with `file:line`, the problem, and a concrete proposed fix:
- **Blocker** — must fix before merge (breaks a boundary/security/correctness invariant, or a gate is red).
- **Should-fix** — real issue, not merge-blocking on its own.
- **Nit** — polish.

Then, separately and explicitly:
- **Verified** — what you proved, each with the **command and its actual output** (gates, `check-core-deps.sh`,
  a re-run flaky suite, a doc you fetched and the Tauri skills you consulted).
- **Read-only** — what you assessed by reading but did not execute.

End with a one-line **verdict**: **ship** / **fix-then-ship** / **needs-rework**, and the single most important
next action. Propose the fixes; **apply them only if the user says so.**
