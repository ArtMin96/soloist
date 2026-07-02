# Changelog

All notable changes to Soloist are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and Soloist adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] — 2026-07-02

### Added

- **macOS-faithful interface.** Redesigned app shell, sidebar, settings, and controls
  with native-style spring motion, dialogs, and a reworked terminal header.
- **Command palette and hotkeys.** `Ctrl+K` command palette plus terminal-search,
  quick-jump, and quick-actions palettes; every actionable hotkey now dispatches.
- **`soloist open`** — raise the running app from the CLI (shares the `/focus` path).
- **`soloist spawn`** — spawn a known agent over the loopback API.
- **`POST /projects/:id/reload`** — reload a project's `solo.yml` over the HTTP API,
  built on a registration-reconcile primitive.
- **Cross-project todo/scratchpad transfer** between coordinated projects.
- **Opt-in agent idle summaries.** Headless summarizer adapter and an idle-summary
  caption on agents (off by default; the core never hard-depends on an LLM).
- **Spawn delegation.** One-level spawn-delegation gate with a live lineage-edges read.
- **Worker lineage in the sidebar** — spawned workers nest under the lead that spawned them.

### Fixed

- `AgentPicker` now always shows the project step when multiple projects are open.

### Docs

- Resolved the scratchpad free-form and file-I/O deferrals.

## [0.1.0] — 2026-06-29

First packaged release — a native-Linux (Ubuntu 22.04+, x86_64) process-supervisor and
AI-agent coordination workspace, shipped as a signed `.deb` and portable `.AppImage`.

[0.2.0]: https://github.com/ArtMin96/soloist/releases/tag/v0.2.0
[0.1.0]: https://github.com/ArtMin96/soloist/releases/tag/v0.1.0
