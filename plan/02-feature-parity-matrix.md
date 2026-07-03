# 02 — Feature Parity Matrix

"Faithful" made measurable. Every Solo capability (from the cited research in
[`05-solo-reference-and-sources.md`](05-solo-reference-and-sources.md)) → the phase that builds it →
a **v1** (required for success criteria) or **later** target → the acceptance check. Phase 13 walks
every `v1` row and records pass/fail; that report *is* the definition of "v1 done."

Source confidence per `05`: ✅ documented · 🟡 stated elsewhere · ❓ gap (our design).

## A. Projects & config (Phase 2)

## B. Process supervision (Phase 3)

## C. Terminal I/O (Phase 4)

## D. Monitoring & self-healing (Phase 6)

## E. Agents & idle (Phase 7)

## F. MCP server — core (Phase 8)

## G. Coordination layer (Phase 9)

## H. HTTP API & CLI (Phase 10)

## I. UX & shell (Phase 11)

### I7 decomposed — Settings detail (Phase 11a per-project · 11b global)

> I7 above is the umbrella row; these are its concrete sub-features, sourced field-by-field from the Solo
> demo "Your new agentic development environment" (Aaron Francis, `youtube.com/watch?v=kVyFCcP6B28`). Full
> field inventory + design in `plan/phases/phase-11a-project-settings.md` and `…-11b-global-settings.md`.
> Both surfaces share **one settings base** (`plan/06` §5.9): a generic `SettingsStore<K, D>` over a
> serde-default document — adding a setting is one field, not a new store.

## O. Orchestrator (track `orch-00`–`orch-05`)

A standalone build track that makes the multi-agent **orchestrator** experience legible and first-class.
The orchestration *mechanism* (a lead spawns workers, hands out blockered todos, waits token-free on a
fire-when-idle timer, wakes to integrate) is **already built and **`**Verified**` — the passing
`crates/pty/tests/orchestration.rs` (E7). This track is therefore **UX + formalization + deferred tools,
not new primitives**: every row *consumes* the existing C6/C4/C2 behavior through the one `Facade`. Full
charter, dependencies, and per-phase definition of done: [`orchestrator/README.md`](orchestrator/README.md).

> **UX source (**`**🟡**`**):** the public Solo demo "Agent orchestration, simplified" (Aaron Francis,
> `youtube.com/watch?v=WAKGhlzpYgs`), re-verified frame-by-frame 2026-06-28 — matched for *feel* only,
> never assets/strings (clean-room, `CLAUDE.md` §9). "Orchestrator" is not a documented Solo concept; it
> is a Soloist-original composition recorded as a gap decision in [`05` §12](05-solo-reference-and-sources.md).
> `Src`: `✅` documented name · `🟡` stated by the demo · `❓` our design.

> `later` (tracked, non-gating — do **not** gold-plate): a deep cross-project "Activity Monitor" (I12),
> prompt-template UI (I13), and LLM auto-summarization of worker output (E6, OFF by default).

## J. Packaging (Phase 12)

## K. Longevity & quality (Phase 13)

## Deliberately excluded

Licensing/Free-Pro/limits, license validation/analytics, Raycast extension, hosted update manifest/  
account, macOS/Windows/arm64 builds, git worktrees/sandboxes, required cloud summarizer, Solo's  
name/logo/assets. (See `00-vision-and-scope.md`.)