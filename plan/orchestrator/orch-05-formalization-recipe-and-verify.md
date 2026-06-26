# Orchestrator Phase O5 — Formalization, Recipe, Docs & Parity Verify

**Goal:** Turn a proven mechanism + new surfaces into a **documented, first-class, end-to-end-verified
capability**. Write the **orchestrator recipe** (the full lead→workers pattern, in the
[`06` §5](../06-codebase-blueprint-and-cleanup.md) recipe style — a sibling to §5.8's create→delegate→use),
provide **agent-readable setup guidance** so an external agent can discover the primitives, run the
**full loop through the now-visible UI** as an acceptance walk, and **verify every `O`-row** with
evidence — flipping the track to `Verified` in `../../PROGRESS.md` (CLAUDE.md §10).

**Delivers:** O11; the track's parity walk (the orchestrator analogue of Phase 13's per-row check).
**Architecture:** documentation + an end-to-end e2e over the existing core + the orch-01/02/03 surfaces;
no new domain behavior.

## Scope
**In:** an orchestrator recipe doc (clean-room composition of documented primitives); agent-facing
guidance content (the orchestration primitives + the "just talk to the agent" pattern, for
`AGENTS.md`/`CLAUDE.md`); a full-loop UI acceptance walk (Playwright); the `O1–O11` parity walk + ledger
update. **Out:** new tools/UI/behavior (everything was delivered orch-00…orch-04); LLM summarization of
worker output (E6, `later`/OFF — the recipe must work heuristic-only, CLAUDE.md §3); pulling the `later`
`setup_agent_integration` MCP tool (F12) into v1 — the recipe is the v1 deliverable; if/when F12 lands it
*carries* this content.

## Why a recipe, not a framework
The demo's thesis is explicit: **no custom harness, no skills — just talk to the agent, given the
primitives** ([README](README.md) §1). So formalization is **documentation + discoverability**, not a new
orchestration engine (that option was rejected in scoping). The recipe records the clean-room composition
so it's reproducible, and the gap decision (orch-00 Task 1) keeps it honestly ours, not attributed to
Solo (CLAUDE.md §9).

## Tasks
1. **Orchestrator recipe (O11, [`06` §5](../06-codebase-blueprint-and-cleanup.md) style):** document the
   full pattern as a recipe — *charter a scratchpad → derive a blockered todo chain → promote a lead →
   `spawn_agent` workers (bound, lineage) → assign locked todos → `timer_fire_when_idle(All)` and end the
   turn → wake on the delivered `body` → read worker output (`get_process_output`) + verify → complete/
   unblock → dispatch the next slice*. Map each step to its primitive + parity row (G/E/F/O) and its UI
   surface (orch-01/02/03). State that it works **heuristic-only** (no summarizer required).
2. **Agent-facing guidance (O11):** write the orchestration section for `AGENTS.md`/`CLAUDE.md`
   (primitives, identity/binding, fire-when-idle, the "don't busy-poll, set a timer and sleep" rule) so a
   freshly bound agent can discover and use the loop with **no skills loaded** — matching the demo. This
   is content; the `setup_agent_integration` tool (F12, `later`) is its eventual delivery vehicle, not a
   dependency.
3. **Full-loop UI acceptance walk (Playwright):** drive the whole loop and assert it is **visible** end to
   end — workers nest under the lead (orch-01), the todo chain shows blockers/locks and the gate (orch-02),
   the living scratchpad updates (orch-02), the `fire_when_idle` timer shows `waiting_on` + countdown and
   then the wake delivers the `body` to the lead (orch-03). Reuse the `crates/pty/tests/orchestration.rs`
   driver pattern so the worker reaches idle the genuine way (terminal output settling → C4 FSM), not the
   backstop.
4. **Parity walk + evidence (O1–O11):** run each `O`-row's Verify, record pass/fail with evidence (test
   names, the e2e run, screenshots from the `/impeccable` `live` pass) — the orchestrator analogue of the
   Phase 13 walk. Confirm CI gates (`clippy -D warnings`, `rustfmt`, `tsc`, ESLint, dependency-direction)
   and that `crates/core` still imports no adapter.
5. **Ledger + docs convergence (CLAUDE.md §10/§11):** update `../../PROGRESS.md` (per-`O`-row status +
   evidence + a "next" pointer), confirm the `02`/`05 §12` records from orch-00 match what shipped, and
   add any late `KNOWN-DIVERGENCES` entry. Flip `O`-rows to `Verified` only with evidence.

## Acceptance criteria
- The orchestrator recipe exists, maps every step to its primitive + parity row + UI surface, and states
  the heuristic-only guarantee; the agent-facing guidance content is written.
- The Playwright acceptance walk runs the **entire loop** and asserts each stage is **visible** (tree,
  todos+gate, living scratchpad, fire-when-idle countdown, wake delivery) — green, and **mutation-verified**
  (a never-idle worker fails the wake assertion, mirroring the E7 test).
- Every `v1` `O`-row (O1–O11) passes its Verify with recorded evidence; all CI gates green; the
  dependency-direction guard green.
- `../../PROGRESS.md` reflects the track as `Verified` with evidence; `02`/`05 §12`/`KNOWN-DIVERGENCES`
  are consistent with what shipped.

## Test plan
- **e2e (Playwright + the real PTY/idle/timer stack):** the full visible loop above; mutation check.
- **Doc lint / link check:** the recipe's parity-row and section references resolve (no dangling `O`/`G`/
  `§` refs).
- **Regression:** the entire workspace suite (Rust + Vitest + Playwright) green; `crates/pty/tests/
  orchestration.rs` still green (the headless mechanism is unchanged).

## Risks & mitigations
- **Recipe drifting into "build a harness"** → it is documentation of a composition of existing
  primitives; if a step needs new code, it belongs in an earlier `orch` phase, not here (scope guard).
- **Claiming `Verified` without the walk** → a row flips only on recorded evidence; a red is reported red,
  never skipped (CLAUDE.md §12/§15).
- **Guidance implying an LLM is required** → the recipe is explicit that idle detection is heuristic and
  always-on; summarization stays optional/OFF (CLAUDE.md §3).
- **Clean-room slip in the docs** → the recipe cites primitives + our gap decisions, never Solo source/
  assets/strings; the UX credit is the demo's *feel* only (CLAUDE.md §9).

## Effort
~3–4 days (recipe + guidance + the full-loop e2e + the parity walk + ledger convergence).
