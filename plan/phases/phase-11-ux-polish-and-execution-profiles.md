# Phase 11 — UX Polish & Execution Profiles

**Goal:** Bring the app to "looks good, feels complete" parity: command + jump palettes, `soloist://`
deep links, themes, keyboard-first navigation, the settings screen, **execution profiles**, env capture,
open-in-editor, and the scratchpad/todo panels (UI for Phase 9 data).

**Delivers:** I2–I11, plus UI for G1–G10 and E6 toggles. **Architecture:** Tauri adapter + frontend;
execution profiles + env capture touch C1/C2.

## Scope
**In:** command palette, jump/attention-jump palettes, deep links, light/dark/system theming, keyboard
nav (remapped to Ctrl/Super), settings screen, execution profiles, env capture, open-in-editor,
scratchpad/todo panels, markdown+mermaid, local/shared command UI. **Out:** packaging (Phase 12); the
parity walk (Phase 13).

## Tasks
1. **Command palette (I2, `Ctrl+K`):** a registry of actions (start/stop/restart any/all, switch
   project, open in editor, toggle theme, focus a terminal, new agent/terminal) so new actions
   auto-appear; fuzzy filter; Enter runs.
2. **Jump palettes (I3):** `Ctrl+E` quick-jump to any destination (process/project/todo/scratchpad);
   `Ctrl+Shift+E` attention-jump (unread only); copy link (`Ctrl+C`) yields a `soloist://` URL.
3. **Deep links (I4):** register the `soloist://` scheme; opening a link navigates to the target
   (project/process/todo/scratchpad).
4. **Theming (I5):** finalize the CSS-variable tokens (Phase 0) → light/dark/**system**
   (`prefers-color-scheme`); persist; apply to xterm.js theme too.
5. **Keyboard nav (I6, ref §10):** remap Solo's Cmd-shortcuts to Ctrl/Super; full dashboard operable
   without a mouse (arrow/`j`/`k` selection, Enter focuses terminal, header keys S/A/P/R, row keys
   S/R/C, project `Super+1–9`, process `Ctrl+1–9`); visible focus + ARIA.
6. **Settings screen (I7, ref §10):** tabs Appearance, Terminal (font/line-height/copy-on-select),
   Notifications, Sidebar, Agents (incl. the Phase 7 tool registry + summarization toggle), Tools
   (default editor/terminal), MCP (server toggle + per-group tool toggles + setup snippet), Hotkeys.
   Auto-save.
7. **Execution profiles (I8, ref §11):** project-level profiles selecting the shell/runtime a command/
   agent executes in (zsh/bash/fish as interactive login shells); processes launch under the chosen
   profile.
8. **Env capture (I10, ref §5):** implement `$SHELL -ilc env` capture, parse, **cache ~10 min**, with the
   precedence from D9 (process `env` > captured > app). Supervisor (Phase 3) consumes it.
9. **Open in editor / terminal (I9):** launch configured `editor` (`code`/`zed`/`subl`) / terminal on the
   project root; detect availability; fall back to `$EDITOR`/`xdg-open`.
10. **Scratchpad & Todo panels (UI for G1–G10):** Markdown editors (markdown-it) with mermaid (lazy),
    search/filter/sort/archive, checkbox todo lists, blockers/comments, "terminal selection →
    scratchpad". Edits go through the Phase 9 repos (revision-guarded).
11. **Local vs shared commands (I-/A12):** show shared (YAML) vs local (app-state) distinctly; local
    additions never written to `solo.yml`.
12. **First-launch guided demo (I11):** a bundled demo project on first run.

## Acceptance criteria
- `Ctrl+K` runs any process/project action; `Ctrl+E` jumps to a destination; a copied `soloist://` link
  reopens it.
- Theme toggle (incl. "system") restyles the whole app + terminal; persists across restart.
- The dashboard is fully operable via keyboard; settings persist and auto-save.
- A command runs under a selected execution profile; env capture exposes a version-manager binary on
  `PATH`.
- "Open in editor" opens the project root; scratchpad/todo panels read/write Phase 9 data (a stale edit
  shows the revision conflict).

## Test plan
- **Playwright:** palette runs an action; theme token changes; jump opens a target; settings persist;
  scratchpad edit + conflict path; local command stays out of `solo.yml` (assert file contents).
- **Integration:** env-capture cache + precedence; execution-profile selection affects the spawned env.

## Risks & mitigations
- **Editor/terminal detection across distros** → probe `$PATH`; fallback `xdg-open`/`$EDITOR`.
- **mermaid bundle size on WebKit** → lazy-load only when a diagram is present.
- **Local/shared write safety** → local overlay is a separate, git-ignored store; never mutate
  committed `solo.yml` without explicit save + re-trust.

## Effort
~6–9 days (v1 items); scratchpad/todo panels + mermaid + deep links can land incrementally.
