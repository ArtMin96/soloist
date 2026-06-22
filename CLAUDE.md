# CLAUDE.md — Operating Manual for Soloist

> **Read this fully before doing anything. Every session.** This file is the contract that makes
> independent sessions behave as one continuous, disciplined engineer. Each phase is built in a *new*
> session with *no* memory of the last one — this document plus the `plan/` docs and `PROGRESS.md` are
> the only shared brain. If you skip the bootstrap or ignore a rule here, you will "handle things
> differently" than the last session did, and the app will drift. Don't.

---

## 0. What Soloist is (30-second orientation)

Soloist is a **native Linux (Ubuntu, x86_64) process-supervisor + AI-agent-coordination workspace** — a
clean-room, open rebuild of the macOS-only closed-source app **Solo** (`soloterm.com`,
`com.soloterm.solo` v0.8.2). It runs your dev stack and CLI coding agents from one dashboard, keeps them
alive, and gives those agents a shared, project-scoped workspace (logs, status, todos, scratchpads,
locks, timers) over **MCP** — all driven by a committable `solo.yml`.

It is **not** a coding agent, **not** a terminal emulator, **not** a worktree orchestrator. It is a
*metaharness*: it runs the agent CLIs you already use as ordinary processes and layers a coordination
surface on top. Mental model and scope live in `plan/00-vision-and-scope.md`.

**Stack:** Tauri v2 + Rust core + React/TypeScript + xterm.js. **Architecture:** Hexagonal (ports &
adapters); a pure, framework-free domain core with 8 bounded contexts (C1–C8); actor-model supervision;
event-driven + CQRS-lite; SQLite for durable state. The full design is `plan/04-…` and is **mandatory
reading** (§3).

---

## 1. Start-of-session protocol (MANDATORY — do this before any work)

Run these steps **in order, every session**, before writing code, planning, or answering a task:

1. **Read `PROGRESS.md`** (repo root). It tells you the current phase, what's Verified vs in-flight,
   open threads, and the exact "next session should start with" pointer. This is your ground truth for
   *state*.
2. **Read this entire `CLAUDE.md`.** Don't skim the rules sections — they are why sessions stay
   consistent.
3. **Read `plan/05-solo-reference-and-sources.md`** — the **behavior contract** (how Solo actually
   works, with citations and confidence markers). Never invent Solo behavior; if it's not here or in
   the matrix, it's a documented gap with *our* decision attached.
4. **Read the architecture set, in this order, before writing or changing any code:**
   `ARCHITECTURE.md` (repo root — the read-first digest: the hexagonal diagram, the 8 bounded contexts,
   the design-patterns-in-practice table, adapter independence) → `plan/04-engineering-architecture-and-patterns.md`
   (the **design contract** — the rules that keep the app from rotting) → `plan/06-codebase-blueprint-and-cleanup.md`
   (the **concrete blueprint** — where every kind of code lives, the *add-a-X* recipes, the cleanup
   roadmap). **Every change must conform to all three** (`CLAUDE.md` §16 is the must-obey summary). They
   are the load-bearing architecture rules — **do not architect a change differently from them**; if you
   believe one must bend, stop and surface it (§12), don't silently diverge.
5. **Open the phase file for the phase named in `PROGRESS.md`** (`plan/phases/phase-NN-*.md`) and read
   it end to end. Then re-read its rows in `plan/02-feature-parity-matrix.md`. That phase file + its
   matrix rows are your task list and your **definition of done** for this session.
6. **Announce your plan**: state which phase you're working, which tasks/parity-IDs you intend to
   complete this session, and confirm they match `PROGRESS.md`. Then proceed.

**Situational — also do these the moment the task touches them:**
- **Any Tauri code** → invoke the matching project-local `tauri-*` skill first (§5), then confirm
  against the official docs (§4). Don't guess an API or config key.
- **Any Claude-Code/MCP or Agent-SDK code** → consult the official docs first (§4).
- **Any UI/UX work** → drive it through the project-local **`/impeccable`** design skill (§5); never
  hand-roll UI.
- **Any unfamiliar library API** → use the `context7` MCP for version-accurate docs (§4).

If any of those files contradict each other, **stop and surface it** — do not pick silently. The
source-of-truth order is in §2.

> Use normal discovery (Glob/Grep/Read) plus LSP for navigation.

---

## 2. Source-of-truth hierarchy (when docs disagree)

1. **The user** (explicit instructions in-session) — always wins.
2. **Official external docs** (§4) — for Tauri / Claude Code / MCP / library APIs, the official docs
   beat memory. Never fabricate an API, flag, or behavior.
3. **`plan/05-solo-reference-and-sources.md`** — what Solo *does* (behavior contract). A 🟡/❓ marker
   means "not fully confirmed — our decision is recorded; follow that."
4. **`plan/04-engineering-architecture-and-patterns.md`** — how we *build* it (design contract).
4b. **`plan/06-codebase-blueprint-and-cleanup.md`** — the *concrete* structural blueprint + recipes,
   derived from `04`; if the two disagree, `04` wins and `06` is fixed.
5. **`plan/02-feature-parity-matrix.md`** — the per-feature scope/verify contract (v1 vs later).
6. **The phase file** — the per-phase plan derived from the above.
7. **This `CLAUDE.md`** — process rules.

If you discover a phase file or matrix row disagrees with `04`/`05`, the higher doc wins; fix the lower
one and note it in `PROGRESS.md`. Never resolve a contradiction by guessing.

### The canonical docs (your map)

| File | Role |
|------|------|
| `README.md` | Index + confirmed decisions + phase map |
| `plan/00-vision-and-scope.md` | What we are/aren't building; success criteria |
| `plan/01-architecture.md` | Concrete system: binaries, crate layout, data flow |
| `plan/02-feature-parity-matrix.md` | ~80 features → phase → v1/later → how to verify |
| `plan/03-tech-stack-and-decisions.md` | Decisions D1–D10, crate choices, rationale |
| `plan/04-engineering-architecture-and-patterns.md` | **Design contract** — domains, patterns, longevity rules |
| `plan/05-solo-reference-and-sources.md` | **Behavior contract** — cited Solo facts + gap decisions |
| `plan/06-codebase-blueprint-and-cleanup.md` | **Structural blueprint** — where code lives, design-patterns-in-practice, add-a-X recipes, the cleanup roadmap (§16) |
| `ARCHITECTURE.md` | **Read-first architecture digest** (repo root) — diagram, contexts, patterns; defers to `04`/`06` |
| `plan/glossary.md` | Shared vocabulary (use these exact terms) |
| `plan/phases/phase-NN-*.md` | The build, phase by phase (00 → 13) |
| `PROGRESS.md` | **State ledger** — what's done, what's next (update every session) |
| `PRODUCT.md` | **Design source of truth (strategic)** — register, users, purpose, personality, anti-references, design principles, a11y. Generated by `/impeccable init`; read before any UI work (§5) |
| `DESIGN.md` | **Design source of truth (visual)** — color/type/components/motion. *Pending* — seed via `/impeccable document` before the first UI work |

---

## 3. Load-bearing facts that must never be forgotten

These are the details that, if missed, cause a session to build something subtly wrong. Treat them as
invariants. (The authoritative versions live in the cited docs; this is the can't-miss summary.)

### Confirmed decisions (locked)
- **D1 Stack:** Tauri v2 + Rust + React/TS + xterm.js. **D2 Target:** Ubuntu 20.04+, **x86_64 only**
  (`.deb` + `.AppImage`; no arm64/macOS/Windows). **D3 Licensing:** dropped entirely (no tiers, no
  server, no analytics). **D4 MCP:** separate `soloist-mcp` binary, **stdio** transport. **D5
  `solo.yml`:** byte-compatible with Solo's schema. **D6 Storage:** SQLite durable, in-memory runtime.
- **Coordination layer (Phase 9, context C6) is v1 scope** — *not* post-parity. Matrix rows G1–G11 + E7.
- **Auto-summarization defaults OFF** — opt-in only, via the user's own headless agent CLI. The core
  must never hard-depend on an LLM. Heuristic idle detection is always on.
- **Git:** the project is under **git** with a **private** GitHub remote. Commit per phase.
  `PROGRESS.md` remains the human-readable state ledger and must be kept current — git history
  complements it, it doesn't replace it. (Exact remote slug recorded in `PROGRESS.md`.)

### The real `solo.yml` schema (D5 — keep byte-compatible)
```yaml
name: storefront                 # optional
icon: assets/project-icon.png    # optional
processes:                       # MAP keyed by process name (NOT a list)
  Web:
    command: npm run dev         # required
    working_dir: null            # optional
    auto_start: true             # our default true (Solo's default for this is a documented gap)
    auto_restart: false          # default false
    restart_when_changed: []     # glob list (file-watch restart)
    env: {}                      # per-process env (highest precedence)
```
1 MB file limit. Trust-gated. Synced via hash-diff + debounce. We **never silently rewrite** the user's
`solo.yml`; app-local additions live in app state (Visibility::Local vs Shared).

### The closed domain enums (drive exhaustive `match`; never stringly-typed)
```rust
enum ProcessKind { Command, Agent, Terminal }
enum ProcStatus  { Stopped, Starting, Running, Crashed, Restarting, Stopping, RestartExhausted }
enum AgentActivity { Idle, Permission, Thinking, Working, Error }   // the 5-state idle FSM
enum Trust       { Untrusted, Trusted { variant_hash: Hash } }
enum Visibility  { Shared, Local }
```

### Hard numeric/behavioral invariants (longevity)
- Crash auto-restart capped at **10 restarts / 60 s → `RestartExhausted`**.
- Log ring buffer default **5,000 lines**; raw scrollback cap default **256 KB**; oldest dropped; a
  global cap across all processes too. **No buffer/channel/retry without a ceiling.**
- Graceful stop signals the **process group**: SIGTERM → ~5 s grace → SIGKILL, then **reap** (no
  zombies/orphans). Spawn into a fresh process group; kill the group, never a bare PID.
- File-watch restarts are **debounced** and **trusted-only**.
- HTTP API: loopback `127.0.0.1:24678`, `X-Soloist-Local-Auth` header on mutations, CORS localhost-only.
- MCP identity: `SOLOIST_PROCESS_ID` env + `bind_session_process`/`register_agent`/`whoami`; tools
  honor the **trust gate** and **effective project scope** (a tool cannot touch another project).
- Data dir: `SOLOIST_APP_DATA_DIR` override; default XDG `~/.local/share/soloist/`.

### The non-negotiable architectural rule
**The domain core (`crates/core`) imports NO app frameworks** — no `tauri`, `rmcp`, `axum`, `rusqlite`.
A CI dependency-direction check enforces this. Everything OS/UI/transport/storage is an adapter behind a
port (trait). This is what makes the whole app headless-testable. If you ever feel the urge to `use
tauri` in core, you're doing it wrong — add a port.

---

## 4. Authoritative external sources — consult them, never fabricate (MANDATORY)

Soloist is built on **official, current documentation**, not memory or assumption. The two doc indexes
below are standing entry points. Consult the relevant one **before** relying on recalled knowledge for
anything touching their domain, and **whenever** an API, config key, flag, version, or behavior is even
slightly uncertain. (Verified reachable 2026-06-14.)

| Source | URL | Use it for |
|--------|-----|-----------|
| **Claude Code / Agent SDK / API** | `https://code.claude.com/llms.txt` | MCP server & tool authoring, hooks, subagents, the Agent SDK, CLI behavior, best practices. `WebFetch` the index, then follow the specific doc link. |
| **Tauri (v2)** | `https://tauri.app/llms.txt` | `tauri.conf.json`, commands/IPC, capabilities & permissions, bundling (`.deb`/`.AppImage`), updater, sidecar/external binaries, security model. `WebFetch` the index, follow the v2 link. |

**Rules:**
- **Don't guess an API or config.** Writing a `tauri.conf.json` key, a capability/permission, an MCP
  tool schema, an Agent-SDK call, or a CLI flag and not 100% certain? **Fetch the doc first**, then
  write. Say which source you used.
- **Prefer official docs over training memory** — both products move fast and may have changed since the
  knowledge cutoff. Treat memory as a hypothesis the docs confirm.
- **`context7` MCP is the second channel** for version-accurate *library* docs (`resolve-library-id` →
  `query-docs`): Tauri v2, `tokio`, `rmcp`, `portable-pty`, `xterm.js`, `axum`, `notify`, etc. Use it
  when you need exact API specifics beyond the llms.txt index.
- **Never fabricate** Solo behavior, an API signature, a version number, or a benchmark/footprint
  figure. Unknown → look it up, or mark it a documented gap (clean-room rule, §9). "Probably" is not a
  source.

---

## 5. Required skills — invoke before the work, not after (MANDATORY)

When a dedicated skill exists for the kind of work you're about to do, **invoke it before you start**, so
the output follows best practices the first time. Process skills come first (brainstorming/planning
before building; debugging before fixing), then implementation skills.

- **UI / UX work → drive it through the project-local `/impeccable` skill (MANDATORY).** Before
  building or changing **any** UI (Phase 5 dashboard, Phase 11 polish, themes, palettes, dialogs,
  terminal pane), use **`/impeccable`** — the design-craft toolkit at `.claude/skills/impeccable/` — not
  a hand-rolled approach and **not** `frontend-design`. **`PRODUCT.md` + `DESIGN.md` at the repo root
  are the design source of truth**: they are generated by `/impeccable init` and every impeccable
  command reads them first, which is exactly how the visual system stays consistent across sessions.
  Typical flow: `/impeccable init` (one-time setup — run before the first UI work) → `/impeccable shape
  <surface>` or `craft <surface>` to design + build → `critique` / `audit` / `polish` to refine →
  `live` for in-browser variant iteration. The visual/UX **north star is soloterm.com** — match its
  *feel* (clean, fast, keyboard-first, calm density, light/dark, native-feeling) **without copying** its
  assets, screenshots, logo, or branding (clean-room, §9). impeccable enforces anti-AI-slop rules
  (contrast ≥4.5:1, OKLCH, no gradient text / eyebrow tropes / side-stripe borders, intentional motion
  with reduced-motion fallbacks) — follow them. Pair with **`webapp-testing`** (Playwright e2e, required
  from Phase 5 on). Every UI surface must be **smooth, fast, and responsive** (§6) — no jank, no layout
  thrash when a terminal is firing output.
- **Tauri work → use the project-local `tauri-*` skill suite (MANDATORY), backed by the official docs.**
  This repo ships ~40 dedicated Tauri skills under `.claude/skills/tauri-*`. **Before writing any Tauri
  code, invoke the skill that matches the task**, follow it, and confirm specifics against
  `tauri.app/llms.txt` + the `context7` Tauri v2 docs (§4). Quick map:
  scaffold/config/architecture → `tauri-project-setup`, `tauri-configuration`, `tauri-architecture`,
  `tauri-process-model`; IPC, commands & events → `tauri-ipc`, `tauri-calling-rust`,
  `tauri-calling-frontend`, `tauri-frontend-events`, `tauri-frontend-js`, `tauri-frontend-rust`;
  security (our trust gate, scopes, CSP) → `tauri-capabilities`, `tauri-permissions`,
  `tauri-plugin-permissions`, `tauri-runtime-authority`, `tauri-scope`, `tauri-csp`,
  `tauri-http-headers`, `tauri-lifecycle-security`, `tauri-ecosystem-security`; the `soloist-mcp`
  sidecar → `tauri-sidecar` (`tauri-nodejs-sidecar` for node helpers); window shell →
  `tauri-system-tray`, `tauri-window-customization`, `tauri-splashscreen`, `tauri-app-resources`;
  **packaging `.deb`/`.AppImage` → `tauri-linux-packaging`; the bundle-size budget (§6) →
  `tauri-binary-size`**; dev → `tauri-debugging`, `tauri-testing`; maintenance →
  `tauri-updating-dependencies`, `tauri-migration`; CI/signing → `tauri-pipeline-github`,
  `tauri-code-signing`. (The macOS/iOS/Android/Windows distribution skills exist but are **out of
  scope** per D2 — Linux x86_64 only.)
- **MCP server work → invoke `mcp-builder`.** Phase 8 builds `soloist-mcp`; invoke the `mcp-builder`
  skill and cross-check the MCP docs from `code.claude.com/llms.txt` (§4).
- **Always re-check what's available at session start.** Skills evolve and new ones get installed. If a
  skill clearly fits the task at hand, you must use it (per the global skills rule) — don't reinvent
  what a skill already encodes.

---

## 6. Performance, size & responsiveness budget (first-class, not an afterthought)

The brief is a **small, fast, smooth** app (Solo advertises "less RAM than a Chrome tab"). These are
**gates, measured — never fabricated.** They complement the longevity rules (§8): longevity stops it
rotting; this section keeps it small and quick.

- **App / bundle size:** Tauri's whole point over Electron is a tiny binary — protect it. Target a
  shipped bundle in the **low tens of MB**; **measure** the real `.deb` and `.AppImage` size in Phase 12
  and record it in `PROGRESS.md`. Lazy-load heavy frontend deps (mermaid, the WebGL addon); code-split;
  tree-shake; **add an npm/cargo dependency only when it clearly pays for itself.**
- **Runtime footprint:** idle RSS target **< ~150 MB** with a small running stack. Phase 13 measures the
  real number; if it misses, document the gap + a plan — **never guess a number.**
- **Responsiveness:** the UI stays ~60 fps even under a chatty process — **coalesce terminal output per
  animation frame**, virtualize long lists/scrollback, and never block the main thread or the `tokio`
  runtime. Backpressure (§8) is what protects this.
- **Build for speed:** shipped binaries use a release profile with **LTO + `codegen-units = 1` + stripped
  symbols**; no debug bloat in releases.
- **Always look for optimizations — but measure first.** Prefer the cheaper algorithm/data structure;
  avoid needless clones/allocations in hot paths (the PTY read loop, event fan-out); pool SQLite
  connections; sample metrics on an interval, not per event. **No speculative micro-optimization** that
  costs clarity without evidence — profile, then optimize the proven hot spot.
- **Doing a performance pass — the workflow (MANDATORY whenever you set out to optimize speed, size,
  CPU, memory, or responsiveness).** Treat it like any other discipline: process before edits.
  1. **Skills + valid sources first, never memory.** Invoke the matching Tauri skills *before* you touch
     anything — `tauri-performance-optimization`, `tauri-binary-size`, `tauri-calling-frontend` /
     `tauri-ipc` (the Channel/event hot path), `tauri-process-model`, `tauri-configuration`,
     `tauri-linux-packaging` (§5) — and confirm every API / flag / config against the official docs
     (`tauri.app/llms.txt`, `code.claude.com/llms.txt`) or `context7` (§4). Do real research from
     **valid, current sources** for anything the skills/docs don't cover. **No assumption, no
     fabrication** — an unverified optimization is not an optimization.
  2. **Measure before you change anything, and after.** Profile the *proven* hot spot first: `just
     bloat` (cargo-bloat — what fills the Rust binary), `just bundle-size` (the real `.deb` /
     `.AppImage` + frontend `dist` bytes), the nightly soak (§8) for RSS / FD / task drift, and the
     webview devtools for frontend repaints. A change with no before/after number is speculative and is
     rejected. Record the numbers in `PROGRESS.md`; never guess one.
  3. **Stay inside the architecture and the budget.** Performance lives in **adapters** and the
     composition root, almost never in the pure `core` (§8); keep the hexagonal layering, the bounded
     caps, and backpressure intact. Correctness and clarity outrank speed — never weaken a test, a cap,
     or a typed boundary for a micro-win, and don't pull a `later` / packaging-or-longevity-phase
     measured decision forward to save time.
  4. **Locked non-changes — do NOT "optimize" these; they are deliberate (cross-check §3 +
     `PROGRESS.md` before touching any build/runtime knob).** `panic = "unwind"` stays — the supervisor
     catches task panics for fault isolation, so `panic = "abort"` would break it; `freezePrototype`
     stays `false` — `true` breaks xterm.js (blank window); the `Cargo.lock` brotli pins stay; release
     `opt-level` (size-vs-speed) is a **measured packaging-phase** decision, not a blind flip;
     `removeUnusedCommands` is only safe once every app command is in the ACL **and** a runtime verify
     (user-only) confirms the IPC surface still works. When unsure whether a lever is locked, stop and
     ask (§12) — don't silently change it.

---

## 7. How work is organized

- **One phase ≈ one session.** Phases are ordered 0 → 13 (see `README.md` phase map). Build order is
  deliberate: config → skeleton → supervisor → I/O → UI → self-healing → agents → MCP → coordination →
  API/CLI → polish → package → verify. Don't jump ahead; later phases assume earlier ones are Verified.
- **The walking skeleton (Phase 1) builds the architecture before features.** Every later phase drops
  into the proven ports/adapters structure. Do not introduce a feature that bypasses the core.
- **The parity matrix is the contract.** Each phase's "Delivers" lists parity IDs (e.g. G1–G11). A
  phase is done only when its **v1** rows pass their "Verify" check. `later` rows are tracked,
  non-gating — do not gold-plate them into v1.
- **Definition of done for a phase** (all required):
  1. Every **v1** parity row the phase "Delivers" passes its Verify check, with evidence.
  2. The phase file's **Acceptance criteria** are all met.
  3. The phase's **Test plan** is implemented and green (unit on mock `Clock`, adapter/integration as
     specified).
  4. CI gates pass: `clippy -D warnings`, `rustfmt`, `tsc --noEmit`, ESLint, **dependency-direction
     check**, and (from Phase 6 on) the nightly soak.
  5. `PROGRESS.md` updated (§10) and any new intentional divergence recorded in `KNOWN-DIVERGENCES.md`
     (created in Phase 13; start the list earlier if you introduce one).
  6. **Codebase-discipline gate (§15, `plan/04` §15) passes:** the phase's code keeps clean
     domain/service separation (hexagonal layering + bounded contexts intact, adapters thin), is
     reusable and DRY (single source of truth; no copy-paste), lives in **small single-purpose files**
     (no god-files — a non-test source file pushing past ~400 lines is a split smell to act on, not
     ignore), and carries no dead code or restating comments. A change that regresses this is **not
     done**, even if tests pass.

---

## 8. Engineering rules you must follow (the anti-rot contract)

These come from `plan/04`. They are **rigid** — adapt the *feature*, never these. Full rationale is in
`04`; the must-obey shortlist:

- **Hexagonal:** core is pure; OS/UI/MCP/HTTP/CLI/SQLite/PTY are adapters behind ports. Core is the only
  source of truth; adapters hold no business state; React renders a pushed read-model projection and
  holds **no business logic**.
- **One behavior, many frontends:** Tauri UI, MCP, and HTTP/CLI all route to the **same** core command.
  Never reimplement an action (e.g. "restart") per adapter.
- **Actors, not shared mutable state:** each managed process is one supervised `tokio` task that solely
  owns its child/PTY/stdin/exit-watcher. Interact via bounded `mpsc` + emitted events. No big `Mutex`
  over domain state. Single-writer per aggregate.
- **FSMs are contracts:** state changes are explicit functions returning `Result<NewState,
  IllegalTransition>`. No ad-hoc field mutation.
- **Errors are values:** typed errors (`thiserror`) at boundaries; **no `unwrap()`/`expect()`/`panic!`
  in long-running tasks** (clippy-denied in `core`). A dying child, bad `solo.yml`, full disk, missing
  agent binary are all *expected* and handled.
- **Fault isolation & self-supervision:** each actor/sampler runs under a supervisor that catches
  panics, marks the unit `Error`, and keeps the app alive. Internal tasks (metrics, file-watch, event
  pump) are themselves supervised and auto-restart with backoff.
- **Bounded everything + backpressure:** caps on buffers/channels/retries; coalesce chatty output per
  frame; debounce file events; rate-limit restarts (10/60s) and summaries. Reclaim OS resources
  deterministically (close PTYs/FDs in Drop/cancel; reap process groups). A start/stop loop of N
  processes ends at the **same** PID/FD/task count it started with.
- **Graceful degradation:** no optional subsystem (summarizer, port discovery, notifications) can crash
  the core.
- **Deterministic shutdown:** stop accepting commands → cancel watchers/timers/samplers → `stop_all()`
  (reap) → flush SQLite → exit. No orphans on quit.
- **Persistence split by lifetime:** ephemeral (registry/PIDs/metrics/PTY buffers) in memory; durable
  (trust, projects, settings, todos, scratchpads, kv, locks/leases) in SQLite (WAL, transactions,
  versioned migrations) via the repository pattern. Optimistic concurrency (revision guards) for
  scratchpads/todos. Leases carry TTL + owner `ProcessId`, auto-release on expiry/owner-close.
- **Security in the core:** the trust gate is enforced in core (not UI) for `start*`/`restart*`/auto-*,
  per (project, command-variant hash). MCP/HTTP honor scope + auth.
- **Comment & naming discipline (source is not a notebook):** code carries **doc comments** (what an item
  does / how to use it / what it depends on) and the **rare** comment that explains a *non-obvious*
  decision — nothing else. **Never** write phase numbers, build/cleanup-phase tags (e.g. `R1`, `Phase 5`),
  plan/doc citations (e.g. `plan/04 §6`), changelog or progress narration, `placeholder`/`TBD` notes, or
  comments that merely restate the code. **The same prohibition applies to every name, not just comment
  text** — file names, module names, function/test names, identifiers, and any other label must not encode
  a phase/R-phase number or a plan citation (no `r1_reach.rs`, no `phase5_test`, no `t_R2_*`); name things
  for *what they are*, permanently true regardless of when they were added. This governs all source
  (`.rs`/`.ts`/`.tsx`/`.css`/config/`justfile`/scripts; `*.md` docs are exempt) **including throwaway or
  temporary files** while they exist. That session/phase context lives in `PROGRESS.md` and git history.
  Any temporary note or file added mid-phase is removed before the phase ends.

Performance/size budget is §6; see `plan/04` §13 for the explicit forbidden anti-patterns and §14 for
the longevity checklist.

---

## 9. Clean-room discipline (legal + ethical — non-negotiable)

- This is a **clean-room rebuild from public docs and observable behavior.** Do **not** copy Solo's
  source, assets, icons, screenshots, strings, or branding. Do not extract anything from the original
  `.dmg`/app bundle into this project.
- **Names mirror, schemas are our own:** MCP **tool names** may mirror Solo (for interop), but their
  parameter JSON Schemas are **clean-room** and documented per tool. `solo.yml` is byte-compatible by
  spec, not by copied code.
- Use the working name **"Soloist"** / app id `dev.soloist.app` (placeholder; not a trademark claim).
  Don't ship Solo's name/logo. The UI may match soloterm.com's *feel* (§5) but not its assets.
- Every fact about Solo's behavior must trace to `plan/05` (a citation). If you need a behavior that
  isn't documented there, treat it as a **gap**: make an explicit decision, record it in `05` §12 and
  `KNOWN-DIVERGENCES.md`, and move on. **Never fabricate Solo behavior or invent numbers** (esp.
  footprint claims — measure, don't guess).

---

## 10. Progress tracking & the ledger (this is how state survives between sessions)

**`PROGRESS.md` is the canonical, human-readable state ledger** — git history complements it, but this
is where a new session reads what's done and what's next. It must always reflect reality.

**Update `PROGRESS.md` at the end of every session** (and whenever you finish a phase task worth
checkpointing). Keep entries factual and evidence-backed — never mark something Verified you didn't
verify.

Status vocabulary (use exactly these):
- `Not started` — no code yet.
- `In progress` — being built; note what's left.
- `Done — pending verify` — code complete, acceptance/Verify checks not yet all green.
- `Verified` — all v1 rows + acceptance criteria + tests green, with evidence recorded.

What a `PROGRESS.md` update must capture:
- Current overall state + the **active phase**.
- Per-phase status (table).
- What you completed this session (with evidence: test names, parity IDs, what you ran).
- Open threads / unresolved questions / decisions awaiting the user.
- **"Next session should start with…"** — a precise pointer so the next session resumes cleanly.

If you make or discover a decision that changes scope or contradicts a plan doc, **fix the doc** and log
the change in `PROGRESS.md` under "Decisions/Changes this session." Don't let the plan and reality drift.

---

## 11. End-of-session / handoff protocol (MANDATORY)

Before you end a session:
1. Make sure the working tree is in a **coherent state** (it compiles / tests you added pass, or you
   clearly note what's red and why).
2. **Update `PROGRESS.md`** per §10 — this is the single most important thing for continuity.
3. If you changed scope or a contract, update the relevant `plan/` doc(s) and note it.
4. Leave the "Next session should start with…" pointer specific and actionable.
5. Summarize for the user: what got done, what's verified vs not, what's next.

A session that wrote code but didn't update `PROGRESS.md` has **failed its handoff** — the next session
will not know what happened.

---

## 12. When you're blocked, unsure, or scope seems to change

- **Ambiguity about an API/config** (Tauri, MCP, a library) → consult the official docs / `context7`
  (§4). Don't guess.
- **Ambiguity about Solo's behavior** → check `plan/05`. Not there → it's a gap; propose a decision,
  don't invent. If it materially affects v1, ask the user.
- **Ambiguity about *our* design** → check `plan/04`. The patterns are opinionated on purpose; follow
  them rather than introducing a new style.
- **A task seems bigger than the phase** → it may belong to a later phase. Check the matrix; don't pull
  `later` work into v1, and don't bypass the architecture to save time.
- **You want to change a locked decision** (§3) → stop and ask the user. Don't silently re-decide
  D1–D6, coordination=v1, or summarization-off.
- **Tests are failing / something's red** → say so plainly with the output. Never report green you
  didn't see. Never delete/weaken a test to make a phase "pass."

---

## 13. Red flags — stop if you catch yourself doing these

| Thought | Reality |
|---------|---------|
| "I'll skip reading `PROGRESS.md`/`04`/`05`, I remember it" | You don't — this is a fresh session. Read them. |
| "I know the Tauri API, no need for the skill" | Invoke the matching `tauri-*` skill (§5) + confirm via §4. |
| "I'll build the UI without the design skill" | Drive it through `/impeccable` first (§5). |
| "I'll just `use tauri` in core to save a step" | Forbidden. Add a port. CI will fail you anyway. |
| "I'll reimplement restart in the MCP adapter quickly" | One core command; route to it. |
| "An unbounded buffer is fine for now" | Every unbounded thing is a future crash. Cap it. |
| "One more npm dep won't hurt the size" | It might. Justify it against the size budget (§6). |
| "Solo probably does X" | Probably ≠ documented. Check `05`; if absent, it's a recorded gap. |
| "I'll mark this phase done; the soak test can come later" | Done means v1 rows + acceptance + tests green. |
| "I'll pull this nice `later` feature into v1" | Don't gold-plate. v1 is the matrix's v1 rows. |
| "I'll finish and skip the `PROGRESS.md` update" | Handoff failed. The next session is now blind. |
| "I'll guess the RAM/size footprint number" | Measure or say unknown. Never fabricate. |
| "I'll note the phase / plan-ref in a code comment for traceability" | Source isn't a ledger. Docblocks + important comments only — no phase numbers or `plan/§` citations (§8). |

---

## 14. Quick reference — toolchain & commands

**Toolchain:** Rust stable (rustup) · Node 20+ · pnpm · `cargo install tauri-cli` ·
`cargo install just`. System libraries (Ubuntu 22.04+) are listed in `CONTRIBUTING.md`.

**Task runner (`just`):**
- `just dev` — run the app (Vite + Tauri) with hot reload.
- `just test` — `cargo test --workspace` + UI unit tests (`vitest`).
- `just lint` — `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  UI `tsc`/ESLint/Prettier, and the dependency-direction guard.
- `just fmt` — auto-format Rust + UI.
- `just bundle` — build the `.deb` (and `.AppImage`).
- `just setup` — install UI dependencies.

**Dependency-direction guard:** `scripts/check-core-deps.sh` (run by `just lint` and CI) fails if
`crates/core` depends on `tauri`/`rmcp`/`axum`/`rusqlite`/`notify-rust`.

**CI:** `.github/workflows/ci.yml` on `ubuntu-22.04` — the `check` job runs every gate; the `bundle`
job builds and uploads the `.deb`.

**Without `just`:** `cargo build --workspace` · `cargo test --workspace` ·
`pnpm -C crates/app/ui build`; run the app from `crates/app` with `cargo tauri dev`.

Playwright e2e arrives in Phase 5; soak/longevity in Phase 13. The build host must be Ubuntu 22.04+
(Tauri v2 needs WebKitGTK 4.1); 20.04 is a runtime target via the AppImage.

---

## 15. Codebase discipline (clean, reusable, single-source — MANDATORY)

The brief is a **clean, disciplined, reusable codebase that stays easy to read and won't break when a
later phase changes it** — not "tons of code that passes." These are hard rules, checked in every review
and every phase; a change that violates one is **not done**.

- **Single source of truth.** Every concept is defined **once** and referenced everywhere else — a
  status, a process kind, an event/command name, a limit, a path. No second definition that can drift.
  Rust domain enums live in `core`; the TS side mirrors them in **one** `domain.ts` and nowhere else.
  Cross-boundary constants (e.g. the Tauri event name) are a single named constant on each side.
- **No magic strings or numbers.** Never compare against or emit a bare status/kind/state string or a
  bare numeric limit/timeout. Use the enum or a named `const`. Stringly-typed domain state is forbidden
  (the closed enums in §3 lock this) — keep it that way in **adapters and frontend** too.
- **DRY — one place to change.** A requirement changes in exactly one place. Extract shared logic to a
  single helper/module; never copy-paste a behaviour. Editing "the same thing" in two files is a signal
  to refactor, not to make two edits.
- **Small, single-purpose files.** A file does one thing; when it starts doing two, split it (§10). No
  god-files — many small focused modules over one large one, in Rust **and** the frontend.
- **Clear domain separation.** Respect the bounded contexts (§3) and hexagonal layering (§8): logic in
  its context, adapters thin, frontend renders projections. Don't smear one concern across layers.
- **Reusable, component-based frontend.** Keep the structure: `domain.ts` (types) · `api.ts` (typed IPC
  only) · `store/` (read-model: pure reducers + hooks) · `components/` (small, reusable, presentational).
  **No** business logic in components, **no** huge `App.tsx`, **no** duplicated markup — extract a
  component. Pure reducers are unit-tested; components stay declarative.
- **Tests test behaviour, not vanity.** Every test exercises real business logic or a real flow and can
  fail for a real reason. **No** placeholder/empty tests, **no** tautological asserts, **no** test
  written only to turn a check green or that pretends to cover something it doesn't. If a module has
  nothing meaningful to test yet, it has **no** test yet — that's honest. Delete pretend tests on sight.
- **No unnecessary code or comments.** Doc comments on public items (§8) and the rare comment explaining
  a *non-obvious* decision — nothing else. No dead code, no speculative abstraction (YAGNI), no comment
  that restates the code. Less code that works beats more code that impresses.
- **Built to change safely.** Prefer the design that survives the next phase touching it: typed
  boundaries, exhaustive `match`, ports over concretions, names that say what they mean. Optimize for the
  reader six months from now.

---

## 16. Architecture & structure rules — how to build *any* change (MANDATORY)

The detailed blueprint is **`plan/06-codebase-blueprint-and-cleanup.md`** (where every kind of code lives,
the design-patterns-in-practice catalog, the step-by-step *add-a-X* recipes, and the cleanup roadmap). Read
it before any structural change. These are the load-bearing invariants it expands — **do not diverge**; if
you think a change needs to break one, stop and surface it (§12):

- **Behavior → context → port → one façade.** All business logic lives in a **bounded context** in
  `crates/core` (`04` §3 map), behind **ports** (traits in `core::ports`), exposed through the **single
  `Facade` (C8)**. Adapters and React hold **no** business logic — they marshal a wire format to one
  `Facade` call and project the read-model back. Never reimplement an action (restart, trust-check) per
  adapter; route to the core. Never add a domain `if` to an adapter.
- **Adapters are independent crates; the dependency points one way.** Each external surface (Tauri UI,
  MCP, HTTP, CLI) is its **own crate** depending only on `core`/`ipc`. `core` depends on **nothing
  app-specific** (CI-enforced, K7). This is the mechanical guarantee that **removing an adapter (e.g. MCP)
  leaves the app building and running** — drop the crate from the workspace + the composition root; nothing
  else references it. Don't put a new integration's logic in `core` or in another adapter.
- **Optional subsystems are ports with `Noop` defaults (Null Object).** A subsystem the core *calls* but
  that may be absent (lock releaser, runtime-state, file watcher, notifier, summarizer) is a trait with a
  `Noop*` default. The core always holds *a* port and never branches on "is it present?". Add new optional
  subsystems the same way (`plan/06` §5.2).
- **One composition root per binary.** `crates/app/src/lib.rs::build_facade` is the **only** place real-vs-
  `Noop` adapters are chosen; it assembles a **`core::ports::CorePorts`** (via its builder, which defaults
  the optional driven subsystems to their `Noop` port) and hands it to `Facade::new`. No other code
  constructs adapters. A future port is **one field on `CorePorts`**, not another constructor argument.
  Tests are alternate composition roots that build a `CorePorts` from `core::testing` fakes.
- **Single source of truth, everywhere.** Every status/kind/event/command/limit/path is defined **once**
  (Rust enum in `core`; the TS mirror in **one** `domain.ts`; one command/event-name constant per side).
  Shared **test fakes** live once in `core::testing` (reused cross-crate via its `testing` feature — see the
  roadmap), never re-rolled per crate.
- **Small, single-purpose files; tests in separate files, honest.** Split a non-test source file at the
  ~400-line smell (`scripts/check-file-size.sh` signals it). New tests live in their **own file**, not merged
  with the implementation (user directive 2026-06-20, reversing the earlier inline rule): unit tests of
  private items via `#[cfg(test)] #[path = "x_tests.rs"] mod tests;` (the module stays a child of its parent,
  so it still reaches private items); adapter integration tests in `tests/`. Inline only when there is no
  other way. Every test must exercise real behavior and be deletable-on-sight if it doesn't.
- **Reach for a pattern when its trigger fires, not before.** Use the `plan/06` §4 table: FSM for legal
  state transitions, Registry for a growing set of handlers (MCP tools, agent providers — never a giant
  `match`), Strategy for per-provider behavior, Repository per durable aggregate, Parameter-Object/Builder
  when a constructor passes >4 collaborators. No speculative abstraction (YAGNI).
- **Use the recipes.** Adding a context behavior, a port+adapter, an MCP tool, an HTTP/CLI/Tauri command, a
  `DomainEvent`, or a UI surface each has a closed checklist in `plan/06` §5. Follow it so the change lands
  in the right layer with the dependency rule, single-source, and DRY intact.
