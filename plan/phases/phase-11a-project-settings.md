# Phase 11a — Per-Project Settings

**Goal:** Build the **per-project settings surface** — the project detail page that edits one project's
identity, run policy, notifications, and commands, with an explicit **shared (`solo.yml`) vs local (app
state)** storage choice for every command. This is the project-scoped half of the settings work that
Phase 11 only sketched (I7 Task 6 covers the *global* screen; Task 11 covers local-vs-shared commands);
it is extracted here so nothing is missed.

**Delivers:** the project settings page (`OVERVIEW`, `SETTINGS`, `NOTIFICATIONS`, `COMMANDS`) + the
"Add command" / per-command editors. **Architecture:** Tauri adapter + frontend over **existing** core
commands in C1 (`solo.yml` config) and C2 (supervisor); new app-local project-settings persisted in
SQLite behind a port. No business logic in the adapter — every field routes to one `Facade` method.

## Position & provenance
- **Sits after Phase 11 (UX Polish), before Phase 12 (Packaging).** Depends on Phase 2 (config &
  projects), Phase 3 (supervisor), Phase 5 (dashboard UI), and Phase 11 (theming, command palette,
  local-vs-shared command UI in Task 11). Does **not** depend on Phase 12/13.
- **Source of every Solo fact below:** the screencast *"Your new agentic development environment"* by
  Aaron Francis, `https://www.youtube.com/watch?v=kVyFCcP6B28`, the project-settings tour at **4:06–4:54**
  (`faster.dev` project), and the `cat solo.yml` at **4:30–4:54**. Read frame-by-frame, no assumptions.
- **Doc follow-ups (intentionally not done in this doc — see "touch only phases"):** add granular rows to
  `plan/02` (split I7 into project vs global) and record these video-sourced facts in `plan/05` §12 to
  keep clean-room discipline (§9). Do these before the phase is marked Verified.

## Settings inventory (every per-project setting — the contract)
Each row is **exactly** what the video shows, plus where it persists in our model.

### OVERVIEW (read/launch surface, not editable preferences)
| Item | Shown as | Persistence |
|------|----------|-------------|
| Directory | project root path + actions: copy path · open folder · open terminal · open in editor · **Reveal in Finder**¹ | derived (project root); actions route to open-in-editor/terminal (I9) |
| Config | `solo.yml` · **✓ Valid** badge · refresh | C1 — parse/validate state of the project's `solo.yml` |
| Commands | "N Running · M Total" | derived from C2 registry |

¹ "Reveal in Finder" is macOS wording; our Linux equivalent is "Show in file manager" (`xdg-open` the dir).

### SETTINGS
| Setting | Control | Default (video) | Storage |
|---------|---------|-----------------|---------|
| Auto Start | toggle — "Commands won't start automatically when Solo launches" | off | **app-local** project setting (project-level auto-start gate; distinct from per-command `auto_start`) |
| Editor | dropdown — "Override the default editor for this project" | (e.g. PHPStorm) | **app-local** project setting; falls back to global Tools default editor (11b) |
| Icon | badge + `Customize`; "Unsupported icon format: .svg (use png, jpg, gif, ico, or webp)" | project initials | `icon:` in `solo.yml` (**shared**); validation rejects `.svg` |

### NOTIFICATIONS
| Setting | Control | Default (video) | Storage |
|---------|---------|-----------------|---------|
| Crash & exit alerts | toggle — "Get notified when commands crash or exit unexpectedly" | on | **app-local** project setting |
| Terminal alerts | toggle — "Get notified when commands ring the bell or request attention" | on | **app-local** project setting |

### COMMANDS — list + per-command editor (expand a row)
Header text (verbatim): *"Commands are processes managed by Solo and optionally saved to your solo.yml.
Think of them as your dev stack. You can set auto-start, auto-restart, and file watching."* Each row shows
`AUTO` / `YML` badges + live status; `[+ Add command]` opens the modal.

| Per-command field | Control | Storage |
|-------------------|---------|---------|
| Name | text — `[Rename]` | the `processes:` map key (**shared**) or local-command name |
| Command | text — `[Edit]` | `command:` (**shared**) / app-local |
| Auto-start | toggle — "Start when project opens" | `auto_start:` |
| Auto-restart | toggle — "Restart if command exits" | `auto_restart:` |
| Terminal alerts | toggle — "Notify when command rings bell" | **app-local** (per command) |
| File watching | glob input ("e.g., src/**/\*.ts") + `[Add]` — "Restart when matching files change" | `restart_when_changed: []` (**shared**) |
| Storage | "saved to solo.yml for version control" → `solo.yml` / `[Make local]` | toggles **Visibility::Shared ⇄ Local** |

### "Add command" modal
| Field | Control | Maps to |
|-------|---------|---------|
| Command name | text ("e.g., Vite, Queue, Logs") | map key |
| Command | text ("e.g., npm run dev") | `command:` |
| Working directory | text + `Browse`; "Leave empty to use project root" | `working_dir:` (null → root) |
| Auto-start when project starts | checkbox (default on) | `auto_start:` |
| Auto-restart if command exits | checkbox (default off) | `auto_restart:` |
| Where to save | radio: **Save to solo.yml** ("Share with your team or use across machines via version control") / **Store locally only** ("Keep this command just for yourself on this machine") | **Visibility::Shared / Local** |

### `solo.yml` schema confirmed by the demo's `cat solo.yml` (matches D5 byte-for-byte)
```yaml
name: faster.dev
icon: public/favicon.svg
processes:
  'npm:dev':
    command: npm run dev
    working_dir: null
    auto_start: true
    auto_restart: false
    restart_when_changed: []
    env: {}
  # …Pint, Scheduler, Claude, Logs, Queue — same shape
```

## Scope
**In:** the project settings page and all editors above; the per-command shared/local storage toggle; the
"Add command" modal; safe `solo.yml` round-tripping (no silent rewrite); app-local project settings
persistence. **Out:** the global Settings window (Phase 11b); execution profiles (Phase 11 I8); env
capture (Phase 11 I10); packaging (12); parity walk (13).

## Tasks
1. **App-local project settings via the shared settings base (`plan/06` §5.9 — do not re-roll a store).**
   Add a `ProjectSettings` document (`auto_start_gate`, `editor_override: Option<String>`,
   `crash_exit_alerts`, `terminal_alerts`, per-command `terminal_alerts`), every field `#[serde(default)]`,
   persisted through the generic `SettingsStore<ProjectId, ProjectSettings>` over a
   `SettingsRepo<ProjectId, ProjectSettings>` (SQLite adapter + `Noop` default, versioned migration). This
   is the **same base** the global settings use with `K = ()` — generalize the existing non-generic
   `core::settings::SettingsStore` rather than copying it. Adding a field later is one edit, no new store.
2. **Core commands (one behavior, many frontends).** Expose `Facade` methods for each editable field
   (set auto-start gate, set/clear editor override, set notification toggles). Route command create/edit/
   rename/delete and the **shared⇄local move** through the **existing C1 config** context so a shared edit
   is a `solo.yml` write (hash-diff + debounce + re-trust per §3) and a local edit touches only app state.
   **Never silently rewrite the user's `solo.yml`** (§3): writes are explicit, validated, and re-trust on
   change.
3. **`solo.yml` validation surface.** Drive the `OVERVIEW` "Config ✓ Valid / invalid" badge and refresh
   from C1's parse/validate result; surface the 1 MB / schema errors inline. Icon validation rejects
   `.svg` (allow png/jpg/gif/ico/webp), matching the video.
4. **Tauri commands + frontend (`plan/06` §5.5, §5).** Thin `#[tauri::command]`s in
   `crates/app/src/commands.rs`, each → one `Facade` method. Build the page as small presentational
   components (`OverviewSection`, `ProjectSettingsSection`, `NotificationsSection`, `CommandList`,
   `CommandEditor`, `AddCommandModal`) over a projected read-model; **no business logic in components**
   (§15–16). Reuse the Phase 11 Task 11 shared/local command UI.
5. **Auto-save.** Toggles/dropdowns persist on change (Solo: "Most settings auto-save"); text fields
   (Command, globs, working dir) commit on blur/Enter. Optimistic UI reconciled from the pushed read-model.
6. **Storage move ("Make local" / "Save to solo.yml").** Moving a command between visibilities is one core
   command that adds/removes it from `solo.yml` and the local overlay atomically; a local command is
   **never** written to `solo.yml` (assert in tests, per Phase 11 risk).

## Acceptance criteria
- Every inventory row is editable and persists across an app restart; toggles auto-save.
- Adding a command via the modal with "Save to solo.yml" appends exactly one `processes:` entry with the
  shown fields; with "Store locally only" the command runs but `solo.yml` is **byte-unchanged**.
- "Make local" / "Save to solo.yml" moves a command between stores with no duplicate and no `solo.yml`
  corruption; a re-trust is required after a shared write.
- Editing a shared command rewrites only the intended `solo.yml` keys (file diff is minimal); an invalid
  edit shows the inline validation error and is **not** written.
- Icon rejects `.svg`; the Config badge reflects real validity.

## Test plan
- **Playwright:** edit each section → reload → values persist; add shared command (assert `solo.yml`
  contents) vs local command (assert `solo.yml` unchanged); "Make local" round-trip; invalid icon/command
  rejected.
- **Integration (core):** `ProjectSettingsRepo` round-trip + defaults; shared⇄local move atomicity; C1
  write goes through hash-diff/debounce and flips trust; serde-default back-compat of `ProjectSettings`.

## Risks & mitigations
- **Silent `solo.yml` rewrite** (§3 invariant) → all writes explicit + minimal-diff + re-trust; local
  overlay is a separate git-ignored store, never merged into the committed file.
- **macOS-only affordances** ("Reveal in Finder", editor list) → map to Linux (`xdg-open`, probed
  editors) per I9; don't copy macOS wording.
- **Field drift vs global defaults** (editor override) → resolve override → global Tools default (11b) in
  one core resolver, single source.

## Effort
~3–4 days (depends on the C1 write path from Phase 2 and the Phase 11 local/shared command UI).
