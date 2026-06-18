# Phase 2 — Config & Projects (C1)

**Goal:** The `solo.yml`-driven project model: parse/validate the **real** schema (ref §3), the **trust**
store (ref §4), **sync** (hash-diff + debounce, ref §4), **command auto-detection** (ref §9), and a
multi-project registry. Headless and testable; the supervisor (Phase 3) consumes this.

**Delivers:** A1–A13. **Architecture:** context C1; uses `Store` (SQLite) + `FileWatcher` ports.

> **Scope decisions for the v1 build (2026-06-14).** The parity matrix (the higher source of truth,
> CLAUDE.md §2) marks **A5 (JSON Schema)** and **A10 (command auto-detection)** as `later`/non-gating,
> whereas Tasks 3 and 8 below listed them as work. Resolved toward the matrix: **A5 and A10 are
> deferred** (tracked `later`, not built this phase), along with the other `later` rows A8/A12/A13.
> Also: the **live `notify` file-watcher** is deferred to Phase 6 (which owns glob file-watch restart,
> D6, on the same infra); Phase 2 ships the deterministic sync engine + a Clock-driven debouncer
> behind the `FileWatcher` port, tested headless. See `KNOWN-DIVERGENCES.md` D-2 and `PROGRESS.md`.
> The trust **variant** covers command+working_dir+env (Task 5) — `KNOWN-DIVERGENCES.md` D-1.

## Scope
**In:** the `solo.yml` model + serde; validator; JSON Schema; project registry (SQLite); trust store
(SQLite) with variant hashing; the file-watcher + hash-diff + debounced sync; the confirm-on-change
decision/event; auto-detection. **Out:** running commands (Phase 3); the trust/sync UI (Phase 5 renders;
core decisions live here).

## The schema (ref §3 — authoritative)
```yaml
name: storefront                 # optional
icon: assets/project-icon.png    # optional, relative to root
processes:                       # MAP keyed by name
  Web:
    command: npm run dev         # required
    working_dir: null            # optional, relative to root
    auto_start: true             # optional; OUR documented default = true (ref §3 gap)
    auto_restart: false          # optional, default false
    restart_when_changed: []     # optional glob list
    env: {}                      # optional
```
```rust
struct SoloYml { name:Option<String>, icon:Option<PathBuf>, processes:IndexMap<String,ProcessSpec> }
struct ProcessSpec { command:String, working_dir:Option<PathBuf>, auto_start:bool /*=true*/,
                     auto_restart:bool /*=false*/, restart_when_changed:Vec<String>, env:BTreeMap<String,String> }
```

## Tasks
1. **Types + serde** with documented defaults; `deny_unknown_fields`; `IndexMap` preserves order.
2. **Loader/validator:** resolve `working_dir`/`icon` relative to file; enforce **1 MB** limit; empty/
   comment-only = empty config; unique names (map enforces); valid globs. Rich `ConfigError` with key
   path — never panics.
3. **JSON Schema** via `schemars` → `solo.schema.json` (+ `# yaml-language-server` header convention).
   **[A5 — `later`, deferred this build.]**
4. **Project registry (SQLite):** add/list/remove projects (`id`, name, root, icon, editor, exec_profile
   ref Phase 11); "recent projects".
5. **Trust store (SQLite, ref §4):** persist trust keyed by **(project, command-variant hash)** where the
   hash covers command + working_dir + env. `is_trusted`, `trust`, `untrust`; renaming **preserves**
   trust (same command string); changing command/dir/env invalidates. "Automatically trust command
   changes" setting (only on user-initiated saves).
6. **Watcher + sync (ref §4):** watch `solo.yml`; **debounce** FS events; compare **file hash**; produce
   a `ConfigSync { added, updated, removed, renamed }` diff; mark changed commands **untrusted**;
   **sync updates config only — never auto-starts**. Emit `ConfigChanged{diff, requires_trust}`.
   **[Sync engine + Clock-driven debouncer built & tested this build; the live `notify`-backed
   `FileWatcher` adapter is deferred to Phase 6 — `KNOWN-DIVERGENCES.md` D-2.]**
7. **Rename detection:** unambiguous remove/add with same command string → preserve row + trust.
8. **Auto-detection (ref §9):** on first add with no `solo.yml`, scan root (package.json scripts,
   Procfile, Make/Just/Task, PM2, turbo/nx, Docker Compose, framework markers) → **suggest** commands
   (dev servers pre-checked for auto-start/restart). Suggest-only; user confirms (keeps trust intact).
   **[A10 — `later`, deferred this build.]**
9. **Fixtures + tests:** valid/invalid/changed/renamed configs; detection fixtures per ecosystem.

## Interfaces
```rust
fn load(path:&Path)->Result<SoloYml,ConfigError>;
struct Projects { /* registry over Store */ }  struct TrustStore { /* over Store */ }
impl TrustStore { fn is_trusted(&self,proj:ProjectId,spec:&ProcessSpec)->bool; fn trust(&self,…); }
enum DomainEvent { ConfigChanged{ project:ProjectId, diff:ConfigSync, requires_trust:bool }, … }
fn detect_commands(root:&Path)->Vec<SuggestedCommand>;
```

## Acceptance criteria
- A real `solo.yml` (the ref §3 example) parses with correct defaults; oversize/invalid → typed error,
  no panic.
- Editing the file emits `ConfigChanged` with an accurate add/update/remove/rename diff; changed
  commands flip to **untrusted**; **no process is started** by sync.
- Trust persists across restart; editing a command's text invalidates its trust; renaming preserves it.
- ~~Auto-detection suggests sensible commands for Node, Rust, Laravel, and Docker-Compose fixtures.~~
  **(A10 — deferred `later`.)**
- ~~`solo.schema.json` validates positive/negative fixtures in CI.~~ **(A5 — deferred `later`.)**

## Test plan
- **Unit:** defaults, validation, 1 MB limit, hash stability, variant-hash trust rules, diff/rename.
- **Integration:** write→mutate→assert `ConfigChanged{requires_trust:true}`; detection per ecosystem.

## Risks & mitigations
- **`auto_start` default unknown (ref §3 gap)** → we default `true`, documented; configurable.
- **Watcher event storms** → debounce + hash-diff (don't reparse-spam); mirrors Solo.
- **Trust bypass** → all gating enforced in core (Phase 3 honors it), not UI.

## Effort
~4–6 days.
