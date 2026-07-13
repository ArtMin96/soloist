# PRD-01 — Editing a command must not delete its `env:` from `solo.yml`

Status: ready-for-agent
Blocked by: none

- **Severity:** P0 (silent data loss on version-controlled config)
- **Area:** config round-trip · `crates/core/src/config`, `crates/core/src/facade/commands.rs`,
  `crates/app/ui` project-settings
- **Parity/phase:** Phase 11a ("safe `solo.yml` round-tripping — no silent rewrite"), currently
  `Done — pending verify`. This blocks that verify.
- **Evidence:** root cause VERIFIED in code (main session, 2026-07-13).

## Problem
A user with a per-command `env:` block in `solo.yml` who edits **any** field of that command in
Project Settings → Commands (toggle auto-start, change the command text, add a watch glob) — or
renames it — **silently loses the entire committed `env:` block** on save. This is version-
controlled config: the change lands in the file the user commits to git.

Violates CLAUDE.md §3: *"Never silently rewrite the user's `solo.yml`."*

## Root cause (verified, end to end)
1. The editor read model and form carry no env: `ProjectCommandView` has no `env`
   (`crates/app/ui/src/domain.ts:586-596`); `CommandFields`/`buildSpec` omit it by design
   (`crates/app/ui/src/components/project-settings/spec.ts:4-22`).
2. Edit sends the env-less spec to core (`ProjectSettingsPane.tsx:81`) →
   `edit_shared_command` does `config.processes.insert(name, spec)`
   (`crates/core/src/facade/commands.rs:190`) — a **whole-spec replace**, so the intended
   config's `env` is `{}`.
3. `ProcessSpec.env` is `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]`
   (`crates/core/src/config/model.rs:74-75`) → an empty env serializes to **nothing**.
4. The surgical writer marks the entry `updated` and re-renders its body via `entry_lines`
   (`crates/core/src/config/edit.rs:93-101`), replacing the on-disk lines (which held `env:`)
   with the env-less render. The rename-verbatim fast path (`edit.rs:84-88`) never triggers
   because the env-less spec never equals the on-disk spec.

## Fix approach
Preferred: **carry `env` through the read model and round-trip it untouched**, even though it
stays non-editable on this surface (per the phase-11a field list).
- Add `env: BTreeMap<String,String>` to `ProjectCommandView` (core) + its TS mirror
  (`domain.ts`), populated from the loaded spec.
- Thread it through `CommandFields`/`buildSpec`/`specOf` so the spec the editor persists preserves
  the command's existing env verbatim (the form doesn't render it, but the spec keeps it).
- Result: `edit_shared_command`'s whole-spec replace now carries the real env → `entry_lines`
  re-emits the `env:` block unchanged.

Alternative (defense-in-depth, do in addition if cheap): make `edit_shared_command` /
`edit_local_command` **merge** the incoming spec onto the stored spec's `env` when the caller
supplies none, so a future env-blind caller can't wipe it either. Decide one as the primary; the
round-trip approach is cleaner (single source: the spec is complete).

Do the same audit for `working_dir` and any other `ProcessSpec` field the editor doesn't render —
confirm each round-trips (working_dir IS in `ProjectCommandView`, so likely fine; verify).

## Test plan (must fail before, pass after)
- **Core (`config/edit` or `facade/commands` tests):** load a `solo.yml` with a shared command
  carrying `env: {A: "1", B: "2"}` + a trailing comment; edit an unrelated field (auto_start);
  assert the re-rendered file still contains the `env:` block byte-for-byte (and the comment) and
  only the intended field changed. Add the rename variant (env survives a rename).
- **Core negative:** a command with no env stays env-less after edit (no spurious `env: {}`).
- **UI (`spec.test.ts` / project-settings):** `specOf(view)` preserves `env`; `buildSpec` round-
  trips a non-empty env.
- Covers test-gap #3-ish (config write round-trip) from the audit.

## Acceptance
- Editing/renaming any field of an env-carrying command leaves the `env:` block intact in
  `solo.yml`. No field the user didn't touch changes. `just test` + `just lint` green. Phase-11a
  "no silent rewrite" verify can proceed.

## Out of scope
Making `env` editable in the UI (that's a separate feature). Trust-hash changes.
