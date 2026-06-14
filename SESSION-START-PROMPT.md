# Session-start prompt

Paste the block below at the start of **every** Soloist work session. Edit only the one `PHASE =` line
(or leave it blank to work whatever phase `PROGRESS.md` says is active). Everything else stays the same
every time — that's what keeps sessions consistent.

---

```
You are working on **Soloist** in /home/arthur/Downloads/soloist — a clean-room, native-Linux
(Ubuntu, x86_64) rebuild of the macOS app "Solo" (soloterm.com). This is a FRESH session with no
memory of previous ones; the repo's files are the only shared brain. Do not assume — read them.

PHASE = ____________________   (e.g. "Phase 0 — Foundations". Leave blank to use the active phase
                                named in PROGRESS.md.)

STEP 1 — Run the start-of-session protocol from CLAUDE.md §1 EXACTLY, in order, BEFORE any other
action (no code, no edits, no answering until this is done):
  1. Read PROGRESS.md — current state, what's Verified vs in-flight, and the "next session should
     start with" pointer.
  2. Read CLAUDE.md in full — the operating rules. They OVERRIDE default behavior; follow them exactly.
  3. Read plan/05-solo-reference-and-sources.md (how Solo really behaves — the behavior contract) and
     plan/04-engineering-architecture-and-patterns.md (how we build it — the design contract).
  4. Open this session's phase file (plan/phases/phase-NN-*.md) and re-read its rows in
     plan/02-feature-parity-matrix.md. Those rows ARE your task list and your definition of done.

STEP 2 — Honor the non-negotiables (full detail in CLAUDE.md):
  - NEVER fabricate. For anything touching Tauri / Claude Code / MCP / a library API, consult the
    official docs FIRST — https://tauri.app/llms.txt , https://code.claude.com/llms.txt , and the
    context7 MCP — then write, and say which source you used. (CLAUDE.md §4)
  - UI/UX work goes through the /impeccable skill, never hand-rolled and never frontend-design; read
    PRODUCT.md first. DESIGN.md is deferred, so CONFIRM visual specifics with me before building any
    UI. (CLAUDE.md §5)
  - Clean-room: rebuild from Solo's public behavior only; never copy its source, assets, or branding.
    Unknown behavior = a documented gap, not a guess. (CLAUDE.md §9)
  - Architecture is law: hexagonal; crates/core stays pure (no tauri/rmcp/axum/rusqlite); actors not
    shared mutexes; FSMs; bounded buffers + backpressure; no unwrap/expect/panic in long-running
    tasks. (CLAUDE.md §8)
  - Small, fast, smooth: respect the performance/size budget — measure, never invent a number.
    (CLAUDE.md §6)
  - Locked decisions (CLAUDE.md §3): Tauri v2 + Rust + React/TS + xterm.js; Ubuntu x86_64; SQLite;
    MCP stdio binary; coordination layer is v1; auto-summarization off by default; no git.

STEP 3 — Announce your plan: state the phase, the exact parity IDs / tasks you intend to complete this
session, and confirm they match PROGRESS.md. Then PAUSE for my go-ahead before writing code.

STEP 4 — Definition of done (per phase, CLAUDE.md §7): every v1 parity row the phase delivers passes
its Verify check with evidence; the phase file's acceptance criteria are met; the test plan is
implemented and green; CI gates pass (clippy -D warnings, rustfmt, tsc, eslint, dependency-direction
check, and the soak from Phase 6 on).

STEP 5 — Before you end the session: UPDATE PROGRESS.md (status, evidence, decisions/changes, and a
precise "next session should start with" pointer), and update any plan doc you changed. A session that
wrote code but didn't update PROGRESS.md has FAILED its handoff.

If any doc contradicts another, STOP and surface it — never resolve a contradiction by guessing.
```

---

## Notes for you

- **CLAUDE.md auto-loads** in Claude Code, but the prompt tells the session to read it in full anyway —
  belt and suspenders, since "missing a detail" is the failure mode you wanted to prevent.
- **First build session only:** also settle the two open gates from `PROGRESS.md` — *plan approved?* and
  *git: yes/no?* — before Phase 0 writes code.
- **Shorter variant** (when you trust the setup and just want it to go):
  > "Soloist session. Run CLAUDE.md §1's start-of-session protocol exactly, then work PHASE = ____
  > (or the active phase in PROGRESS.md). Follow all CLAUDE.md rules; announce your plan and pause for
  > my go-ahead; update PROGRESS.md before ending."
