# Phase 13 — Parity QA, Testing & Longevity Hardening

**Goal:** Prove the clone is faithful **and durable**. Walk every `v1` parity row on a clean machine,
consolidate the test suites, and run the **longevity gate** — the soak/leak tests that satisfy the
brief's "won't break in continuous work." The output is a `parity-report.md` that *is* the definition of
"v1 done."

**Delivers:** verification of all `v1` rows; K1–K7. **Architecture:** validates the rules in
[`04` §8/§14](../04-engineering-architecture-and-patterns.md).

## Scope
**In:** the parity walk; integration/e2e consolidation; the longevity/soak suite; footprint measurement;
cross-distro smoke; accessibility; a bug-fix buffer. **Out:** new features (anything not in Phases 1–12);
`later`-tagged rows (tracked, non-gating).

## Tasks
1. **Parity walk.** For each **`v1`** row in `02-feature-parity-matrix.md`, run its "Verify" check on a
   clean Ubuntu 22.04 packaged install; record pass/fail + evidence (log/screenshot) in
   `parity-report.md`. Any fail → ticket; v1 isn't done until all `v1` rows pass.
2. **Test consolidation.** Ensure coverage per subsystem:
   - core (Rust, mock clock): config/trust/sync; supervisor FSM + orphans; PTY echo/resize/OSC;
     restart 10/60s + file-watch debounce; metrics/ports; idle FSM; coordination (revision/lock/lease/
     timer).
   - adapters: MCP scripted client over stdio (every v1 tool + scope/trust); HTTP API (auth/CORS/
     endpoints); CLI; SQLite `Store` repos.
   - UI: Playwright (dashboard, terminal interactivity, trust + orphan dialogs, palette, theme, project
     switch, scratchpad conflict).
   - packaging: containerized `.deb`/`.AppImage` install + launch smoke.
3. **Longevity / soak gate (K2/K3, `04` §14):** a multi-hour run of a 10-process stack with random
   crashes/restarts/file-touches → assert **flat RSS, flat FD count, flat task count, zero leaked PIDs/
   zombies**, bounded log memory under a chatty producer (backpressure holds).
4. **Footprint (K1).** Measure idle RSS with a small running stack; record the **actual** number vs the
   budget (target < ~150 MB). If it misses, document the gap + a plan — **no fabricated numbers**.
5. **Self-supervision & degradation (K4/K5).** Kill internal tasks (metrics sampler, watcher, HTTP
   server) → assert they self-restart; take the summarizer/ports offline → assert graceful degradation,
   core unaffected.
6. **Crash recovery (K6).** Force-quit mid-run → next launch reconciles orphans (adopt/kill/leave) and
   SQLite state is intact/consistent.
7. **Dependency-direction (K7).** Assert the CI guard is green (`core` has no adapter imports).
8. **Cross-distro smoke.** Launch + a basic flow on Ubuntu 20.04 / 22.04 / 24.04 (+ a Debian); note
   webkit/lib differences.
9. **Accessibility.** Keyboard-only run-through; focus order; contrast on both themes; control labels.
10. **Edge cases.** Missing `solo.yml`; nonexistent command; a process ignoring SIGTERM (escalate to
    SIGKILL); MCP called with app down; `solo.yml` edited mid-run; agent binary not installed; disk full
    while writing logs/SQLite.
11. **Docs.** User README (install, write a `solo.yml`, register MCP with Claude Code, use the CLI) and
    `KNOWN-DIVERGENCES.md` (every intentional difference from Solo: our `auto_start` default, stop-signal
    semantics, lease TTL, clean-room MCP schemas, theme specifics, no licensing).

## Acceptance criteria (the project's v1 gate)
- `parity-report.md` shows **every `v1` row PASS** on a clean Ubuntu 22.04 packaged install.
- All test suites (core, adapters, UI, packaging) run green in CI.
- **Longevity gate passes:** flat RSS/FD/task counts and zero leaked PIDs over the multi-hour soak;
  backpressure holds under a chatty producer.
- Idle RSS measured + recorded (gap documented if over budget).
- Self-supervision, degradation, and crash-recovery checks pass; dependency-direction guard green.
- `KNOWN-DIVERGENCES.md` exists and is accurate.

## Test plan
- This phase **is** the consolidated test plan executed: CI runs the automated portion every push; a
  pre-release checklist runs the manual parity walk + cross-distro smoke + the soak gate.

## Risks & mitigations
- **"Faithful" is subjective** → the matrix makes it objective; divergences documented, not argued.
- **Footprint may miss the Chrome-tab claim on some GPUs** → measure + report honestly; optimize
  (lazy mermaid/webgl, cap buffers); never fabricate the number.
- **Flaky e2e under WebKitGTK** → wait on app events, not sleeps; fix flakes, don't retry-mask.
- **Soak surfaces slow leaks late** → run it nightly in CI from Phase 6 onward, not just here.

## Effort
~6–9 days (+ a bug buffer for what the parity walk and soak surface).

---

## Appendix — phase sequencing & estimate

| Phase | Outcome | Effort |
|------:|---------|--------|
| 0 | Workspace + `.deb` builds | 2–3 d |
| 1 | Walking skeleton & architecture | 4–5 d |
| 2 | Config & projects (real schema, trust, sync, detect) | 4–6 d |
| 3 | Process supervisor (3 subtypes, FSM, orphans) | 5–7 d |
| 4 | PTY & terminal I/O (rendered+raw, input, OSC) | 4–5 d |
| 5 | Dashboard UI | 5–7 d |
| 6 | Monitoring, restart (10/60s), file-watch, notifications | 5–6 d |
| 7 | Agents & idle detection | 5–7 d |
| 8 | MCP server core | 5–7 d |
| 9 | Coordination layer (scratchpads/todos/timers/locks/kv) | 7–9 d |
| 10 | HTTP API & CLI | 3–5 d |
| 11 | UX polish & execution profiles | 6–9 d |
| 12 | Packaging (x86_64) | 3–5 d |
| 13 | Parity QA + longevity gate | 6–9 d |

**v1 critical path (parity rows only):** ~14–18 focused weeks for one experienced Rust+TS developer —
the coordination layer (Phase 9) is **in v1 scope** per your decision, which is the main driver of the
range. The remaining `later` rows (auto-summarization, deep links, command auto-detect, update channel,
signed repo) add a couple of weeks. Phases 3, 8, and 9 carry the most risk and deserve the most test
budget; the **architecture (Phase 1) + longevity gate (Phase 13)** are what keep it from rotting under
continuous use.
