# PROGRESS-SETTINGS.md — Settings Stack Review Ledger

> Session-to-session handoff for the **Phase 11 settings stack review** (PRs #31 → #36), kept
> separate from `PROGRESS.md` by request. One PR is reviewed/fixed per session. This file records
> what was done, what was decided, and the precise pointer for the next session. It does **not**
> replace `PROGRESS.md` and must never be used to mark a build phase Verified.

---

## The stack (as of 2026-06-27)

```
main (bcd9ebc, incl. cache PRs #38/#39)
 └─ #31 feat/phase-11-settings-ui      ← REVIEWED + conflict-fixed THIS session
     └─ #32 feat/phase-11-settings-window     (Appearance tab + xterm restyle, I5)
         └─ #33 feat/phase-11-settings-panels (Tools / Integrations / Agents)
             └─ #34 feat/phase-11-settings-sidebar
                 └─ #35 feat/phase-11-settings-hotkeys
                     └─ #36 feat/phase-11a-project-settings
 (merged, now the "existing cache mechanism": #38 read-through ReadCache, #39 frontend SWR)
```

---

## Session log

### Session 1 — PR #31 (`feat/phase-11-settings-ui`): the global-settings backend slice

**Scope of #31:** the I7s generic base + 11b backend. Generalizes `SettingsStore`/`SettingsRepo`
to `SettingsStore<K, D>` / `SettingsRepo<K, D>`; adds six per-tab sub-documents (appearance,
sidebar, hotkeys, agents, tools, integrations); façade getters/setters per tab; the SQLite
`SettingsRepo<(), Settings>` adapter; 18 Tauri commands; the `domain.ts` mirror + `api.ts` IPC.
The Settings **window/panels** are NOT in #31 — they are #32–#36.

**Review verdict: fix-then-ship → done, now ship-ready.** The code conforms tightly to the
architecture contract — among the cleanest slices in the repo:
- Boundaries: `core` framework-free (`check-core-deps.sh` green); SQLite behind a generic port;
  generic `NoopSettingsRepo` default; one composition root wires it (`app/src/lib.rs:117`).
- Single source / DRY: Rust enums in `core`, one `domain.ts` mirror, command names once in
  `api.ts`; one `update(key, mutator)` write primitive every setter routes through; one generic
  `FakeSettingsRepo<K, D>` shared fake.
- No magic strings/numbers: every picker is a closed enum (theme, thresholds, font weight, …).
- Patterns at their trigger: Repository (generic), Null Object (generic Noop), Facade (thin
  pass-throughs), code-defined-default + override-only persistence for the hotkey registry.
- Tests honest and in sibling files: round-trips, defaults, tab independence, cross-scope hotkey
  sharing, within-scope conflict, and serde-default back-compat.

**Findings (all addressed or recorded):**
- *Blockers:* none.
- *Should-fix:* none.
- *Fixed this session (nits, CLAUDE.md §8):* two doc comments carried a bare phase number and a
  plan-section citation — `settings/agents.rs` ("(Phase 7)") and `settings/hotkeys.rs`
  ("(`plan/05` §10)"). Removed; the adjacent `C`-context IDs are sanctioned vocabulary and stay.
  Commit `439e865`.
- *Forward dependency (not a #31 defect):* `Integrations.mcp_enabled` (the MCP master toggle) is
  persisted but not yet consulted by `soloist-mcp` — only the per-group `McpToolGroups` gate (G10)
  is wired. Wiring the master toggle is later-slice work; #31 correctly only stores it.

**Conflict with main — FIXED.** main moved ahead of #31 via the cache PRs #38/#39. The only real
merge conflict was **`PROGRESS.md`** (settings entries vs cache entries); `app/lib.rs`,
`core/facade.rs`, `core/lib.rs`, `plan/06` all auto-merged. Resolved by **merging main into the
branch** (repo convention) and **union-resolving `PROGRESS.md`** (both sides kept verbatim, no new
content authored — +95 lines = main's cache entries added, #31's entries preserved). The merge
also pulled in main's `WindowControls.tsx` prettier fix that #31's branch was missing.
- Merge commit: `6b8518d`. Branch HEAD: `439e865`.
- `git merge-tree main feat/phase-11-settings-ui` → **NO CONFLICTS** after the merge.

**Gates (run on the final branch state):**
- `just lint` → exit 0 (clippy `-D warnings`, fmt, tsc, eslint, prettier, dep-direction
  framework-free; only the 4 pre-existing file-size advisories, none from #31).
- `just test` → Rust **628 passed / 0 failed**, UI **89 passed (18 files)**.

**NOT pushed.** Per the project's "user pushes / no self-merge" rule, the merged branch is local
(`439e865`). `origin/feat/phase-11-settings-ui` still points at `0220731`, so GitHub PR #31 shows
CONFLICTING until the user pushes. **Action for the user:** `git push origin feat/phase-11-settings-ui`.

---

## Decision — where settings caching belongs (researched from #38/#39)

The user asked to "consider caching so each setting is not re-queried," reusing the existing
mechanism. After reading the real mechanism:

- **`core::cache::ReadCache<T>` (#38)** — a `Clock`-driven, single-flight, **TTL-only**, async,
  `pub(crate)` memo for **expensive off-runtime reads** (running `$SHELL -ilc env`; probing CLIs
  with `--version`). `invalidate()` is deliberately **not** built — it is planned for the
  event-invalidated `projects_snapshot` cache and is **DEFERRED until measured** ("do NOT build it
  speculatively", user-confirmed).
- **Frontend `usePersistentSnapshot` (#39)** — a disk-backed (tauri-plugin-store)
  stale-while-revalidate cache for **cold-start render-avoidance**: paint the last snapshot
  instantly, revalidate on a known event, **core always wins**.

**Conclusion: settings caching is a FRONTEND concern and lands with the UI (#32+), not a backend
`ReadCache`.** Grounded reasoning (deliberately considering both directions, not one):
1. A settings read is one small `SELECT doc FROM settings WHERE id=1` + a JSON parse — sub-ms,
   **not** an expensive probe. A backend `ReadCache` for it would be the speculative,
   unmeasured optimization the mechanism's own author warns against.
2. It would not help the `soloist-mcp` process (separate process, own SQLite connection), and a
   TTL there would make group-toggles propagate late — net worse.
3. The real "don't re-query" pressure is the **live UI**: Appearance drives the app + xterm,
   Sidebar drives the live sidebar projection, Hotkeys drives the keyboard handler — all read
   settings during normal use. That is exactly what the frontend SWR cache solves.
4. Settings invalidation is **write-driven, not TTL**: every setter already **returns the stored
   document**, so the cache is seeded from the setter result — no re-query, no staleness, and no
   domain event needed (the write path is local to the same frontend).

**Concrete plan for the next session(s) (#32+), where the consumer lives:**
1. Add a single whole-document read — `Facade::settings() -> Result<Settings, StoreError>`
   (`Ok(self.settings.get(&())?)`) + a `#[tauri::command] settings()` + `api.ts`
   `settings(): Promise<Settings>` — so the window fetches **all tabs in one call**, not seven.
2. Add `CacheKey.settings` in `store/cache/persistentCache.ts` and a `useSettings()` hook over
   `usePersistentSnapshot(CacheKey.settings, () => settings())` — instant cold-open, revalidate on
   settings-window open.
3. Write-through on save: after a per-tab setter resolves, merge its returned value into the cached
   `Settings` snapshot (a pure reducer) + `writeSnapshot` — **no re-query** (the setters return the
   stored value precisely to enable this).
4. Do **not** add `core::cache::ReadCache` for settings. If a future in-process settings read is
   ever *measured* hot, the pattern is the planned `ReadCache::invalidate` + invalidation on
   `SettingsStore::update` — but only when measured.

---

## Next session should start with

**Push #31, then review PR #32 (`feat/phase-11-settings-window`).**
1. `git push origin feat/phase-11-settings-ui` (publishes the merge so GitHub PR #31 is mergeable).
2. **Re-stack #32 onto the updated #31:** #32's branch still bases on the pre-merge #31 head
   (`0220731`); merge the new #31 (`439e865`) into `feat/phase-11-settings-window` and resolve any
   conflict the same way (union for `PROGRESS.md`).
3. Review #32 (Appearance tab + the I5 xterm restyle) per the soloist-review dimensions; #32 is the
   first Settings **UI** PR, so it goes through `/impeccable` and the `tauri-*` skills, and it is
   the natural home for the **caching plan above** (steps 1–3).

**Reference:** #31 review evidence is in this file; the settings design contract is `plan/06` §5.9,
the parity rows `plan/02` I7s/I7f–I7k, and the Solo facts `plan/05` §12; the phase plan is
`plan/phases/phase-11b-global-settings.md`.
