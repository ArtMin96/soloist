<div align="center">

<img src="crates/app/icons/128x128.png" width="96" alt="Soloist">

# Soloist

Run your dev stack and your CLI coding agents from one window, and let the agents work together.

[![Release](https://img.shields.io/github/v/release/ArtMin96/soloist)](https://github.com/ArtMin96/soloist/releases/latest)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
![Platform](https://img.shields.io/badge/platform-Ubuntu%2022.04%2B%20x86__64-e95420)

</div>

## What Soloist is

Soloist is a Linux desktop app that runs everything a project needs in one window: the dev servers,
workers and build watchers you already have, plus the CLI coding agents you point at that project.
Each process gets a real terminal you can type into. Soloist keeps them alive, restarts them when
they crash, tells you when one needs an answer, and gives the agents a shared workspace to
coordinate through.

It is a Linux alternative to [Solo](https://soloterm.com/), which runs only on macOS. Soloist is a
clean-room rebuild for Ubuntu, written from Solo's public documentation rather than its source. It
reads the same `solo.yml` schema and keeps its own extra settings out of that file, so the config in
your repo stays portable. The [Origins](#origins) section says exactly what that does and does not
mean.

Soloist is not a coding agent, not a terminal emulator, and not a worktree manager. It runs the
agent CLIs you already installed and handles everything around them.

## Multi-agent orchestration

Running one coding agent in a terminal tab is easy. Running five is where it comes apart. You lose
track of which one is blocked on a permission prompt, two of them edit the same file, and you become
the message bus between them.

Soloist gives agents a project-scoped workspace they reach over MCP, with 122 tools. Through it they
can:

- Read and control the project's processes. Start, stop and restart commands, read a process's
  output, search its scrollback, find which TCP ports it ended up listening on, send it input.
- Claim work through shared to-dos. A to-do carries blockers, tags, comments, and a lock. Completing
  one is refused while its blockers are still open, and the refusal names them, so the ordering is
  enforced by the store instead of by a paragraph in a prompt.
- Take a lease on a key before touching something, with a TTL and a named owner. A contending agent
  is told who holds it. If the holder crashes, the lease expires by itself instead of wedging the
  project.
- Message each other directly, broadcast to the project, read a roster of who is live, and report
  completion when they finish.
- Write to shared Markdown scratchpads. Writes are revision guarded, so a stale write is rejected
  rather than quietly overwriting someone else's edit.
- Draw Mermaid diagrams.
- Set a timer that fires at a deadline, or when any (or all) of a watched set of processes go idle.
  That last one is how a lead agent waits for its workers without burning tokens polling them.
- Share a small key-value store for whatever the agents need to agree on.

A lead agent can spawn workers. Those workers nest under it in the sidebar, so the tree of who
spawned whom stays visible while it runs. Soloist reads each agent's output and classifies it as
idle, waiting on a permission prompt, thinking, working, or errored. That classification runs on
per-provider heuristics with no model in the loop, so it works offline and costs nothing. Optional
idle summaries do use a model, and they are off by default.

The orchestration view has tabs for the agent tree, to-dos, scratchpads, diagrams, timers and
messages, so you can watch the same workspace the agents are using and step in when you want to.

Seven agent CLIs ship in the registry: Claude, Codex, Amp, Gemini, OpenCode, Copilot and Kimi. Any
other CLI can be added as a generic tool, where you choose whether its prompt arrives on stdin or as
an appended argument. Soloist probes `--version` to work out which ones you actually have installed.

## Supervising your stack

The commands a project needs live in a committable `solo.yml` at its root. Soloist starts them,
watches them, and restarts them when they die.

Crash restarts are rate limited to 10 in 60 seconds. After that Soloist stops trying and says so
rather than looping forever. A command can also restart when files matching a glob change, debounced
so a burst of saves triggers one restart and not twenty. Each process shows its CPU and memory, and
the TCP ports it bound, which saves guessing which of four servers took port 3000.

Nothing starts until you trust it. Trust is stored on your machine and scoped to the project plus
the exact command variant, meaning the command string, its working directory and its environment.
Renaming a command keeps its trust. Changing what it actually runs asks you again. The gate lives in
the core, so it applies to a start from the UI, from an agent over MCP, from the HTTP API, and to
auto-start and file-watch restarts alike.

When a process crashes, or an agent stops and waits for you, Soloist raises a desktop notification
and marks the row in the sidebar.

## Git and pull requests

A version-control rail sits beside the terminal instead of replacing it, so you can watch a
repository's state while an agent keeps working in the pane next to it.

The working tree view shows status, stages and discards changes by file or by individual hunk,
commits, and renders diffs side by side or unified. Branches can be created, switched and deleted,
and there is fetch, pull, push, stash and pop.

Pull requests run through your own `gh` CLI: create a PR, read its review threads, merge it. That
choice is deliberate. Because `gh` owns the account, your host and any enterprise configuration
apply without Soloist knowing about them, and Soloist stores no token, names no credential helper,
and never sees a secret. Plain `git` operations leave credentials to your own `git` the same way.

Every one of these is also an MCP tool, so an agent can do the same things you can, under the same
rules.

## Terminals and the keyboard

Every process gets an xterm.js terminal. It is interactive, searchable, takes dropped files, and has
adjustable font size. Agents run on a real PTY, so they behave the way they do in your own terminal.

These defaults are all remappable in Settings > Hotkeys:

| Keys | Action |
|---|---|
| <kbd>Ctrl</kbd>+<kbd>K</kbd> | Command palette |
| <kbd>Ctrl</kbd>+<kbd>P</kbd> | Quick actions |
| <kbd>Ctrl</kbd>+<kbd>E</kbd> | Quick jump |
| <kbd>Ctrl</kbd>+<kbd>T</kbd> | New agent or terminal |
| <kbd>Ctrl</kbd>+<kbd>W</kbd> | Close agent or terminal |
| <kbd>Ctrl</kbd>+<kbd>F</kbd> | Search the terminal |
| <kbd>Ctrl</kbd>+<kbd>,</kbd> | Settings |
| <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>C</kbd> / <kbd>V</kbd> | Copy and paste. Bare <kbd>Ctrl</kbd>+<kbd>C</kbd> goes to the process, as it should. |

## Themes

Six themes ship with the app: Soloist Default, Poimandres, Catppuccin Mocha, Dracula, Tokyo Night
and GitHub Light. Any of them can be duplicated and edited live across 57 named colour roles, and
the terminal's colours follow the theme along with everything else. Soloist reads and writes
T3-compatible v1 theme JSON, so a palette you already use elsewhere can come with you, and one you
build here can go in a dotfile repo.

## Install

Ubuntu 22.04 or newer, x86_64 only. Both artifacts are on the
[latest release](https://github.com/ArtMin96/soloist/releases/latest).

The `.deb` links the system WebKitGTK 4.1:

```bash
sudo apt install ./Soloist_*_amd64.deb
```

The `.AppImage` bundles its own WebKit, so it needs nothing installed:

```bash
chmod +x Soloist_*_amd64.AppImage
./Soloist_*_amd64.AppImage
```

Both are signed, and every release attaches `SHA256SUMS`:

```bash
sha256sum -c SHA256SUMS
```

The `.deb` installs `/usr/bin/soloist` (the app), `/usr/bin/soloist-mcp` (the MCP server) and
`/usr/bin/soloist-cli` (the command line client), along with a desktop entry, hicolor icons, and a
MIME association for `solo.yml` so your file manager offers "Open with Soloist" on a project file. A
tray icon carries Show Soloist, Start on login (off unless you turn it on), Check for Updates, and
Quit Soloist.

Soloist never checks for updates on its own. The tray item is the only trigger, and it replaces the
AppImage in place. A `.deb` install updates through `apt`.

Ubuntu 20.04 is not supported, because Tauri v2 needs WebKitGTK 4.1 and 20.04 does not have it.
Neither are arm64, macOS or Windows.

## First run

Open a project, meaning a folder on this machine. If that folder has no `solo.yml`, Soloist scans it
for `package.json` scripts, a `Procfile`, a `Makefile` or `justfile`, `Cargo.toml`, `go.mod`, Docker
Compose and similar, then writes one for you. It never rewrites a `solo.yml` you already have.

Everything it found arrives untrusted, so nothing runs until you look at the commands and approve
them. After that, start what you want, or press <kbd>Ctrl</kbd>+<kbd>T</kbd> to launch an agent or
open a terminal.

## The solo.yml file

`solo.yml` sits at the project root and declares the commands Soloist supervises. It is capped at
1 MB, and an empty or comment-only file is valid. The implementation is
[`crates/core/src/config/model.rs`](crates/core/src/config/model.rs) and the JSON Schema is
[`solo.schema.json`](solo.schema.json).

```yaml
name: storefront                 # optional, the display name for the project
icon: assets/icon.png            # optional, an image path relative to the project root

processes:                       # a MAP keyed by each command's display name, not a list
  web:
    command: npm run dev         # required, the shell command to run
    working_dir: web             # optional, relative to the project root (default: the root)
    auto_start: true             # optional, start when the project opens (default: true)
    auto_restart: true           # optional, relaunch after an unexpected exit (default: false)
    restart_when_changed:        # optional, globs relative to the root that trigger a restart
      - src/**/*.ts
      - config/**
    env:                         # optional, environment overrides for this command
      PORT: "3000"
  build:
    command: npm run build
    auto_start: false            # a one-shot task, so don't start it on open
```

Top level:

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `name` | string | no | | Display name for the project. |
| `icon` | path | no | | Image path, relative to the project root. |
| `processes` | map | no | `{}` | Commands keyed by display name. File order is preserved. |

Each entry under `processes:`

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `command` | string | yes | | The shell command to run. |
| `working_dir` | path | no | project root | Working directory, relative to the root. |
| `auto_start` | bool | no | `true` | Start this command when the project opens. |
| `auto_restart` | bool | no | `false` | Relaunch after an unexpected exit. |
| `restart_when_changed` | list of globs | no | `[]` | Globs that trigger a restart. Trusted commands only, debounced. |
| `env` | map | no | `{}` | Environment overrides for this command, highest precedence. |

Only `command` is required. When Soloist writes the file for you it leaves out anything sitting at
its default, so what lands in your repo stays short.

## Connecting an agent over MCP

The MCP server is the `soloist-mcp` binary and it speaks stdio, so there is no port to configure.
Your MCP client launches it, and it connects to the running app over a private Unix socket in
Soloist's data directory, then forwards each call to the same core command the UI uses.

Settings > Integrations generates the snippet with the helper path already filled in, which is worth
using because the right path differs between a `.deb` install, an AppImage and a dev build. For
Claude Code it produces a project-root `.mcp.json` like this one, from a `.deb` install:

```json
{
  "mcpServers": {
    "soloist": { "command": "/usr/bin/soloist-mcp" }
  }
}
```

Identity, projects, processes, bulk commands, output, services, lease locks and help are always
served. Scratchpads, To-dos, Timers, Key-Value and Prompt Templates toggle per group in the same
place, and Key-Value and Prompt Templates start off. Two tools are worth knowing before the
rest: `help` returns the agent usage guide and answers even while the app is closed, and
`setup_agent_integration` writes that guide into the project's `AGENTS.md` or `CLAUDE.md` as a
managed section that re-running updates in place.

On security: the data directory is owner-only (`0700`), so no other local user can reach the socket.
A session is identified by its connecting peer's process group, and a bind or project selection the
peer does not actually run in is refused, so one client cannot talk its way into a sibling project.
Starting or restarting a command sits behind the same trust gate as the UI.

Per-client setup for every supported client is in [`docs/mcp-setup.md`](docs/mcp-setup.md).

## Command line and HTTP API

While the app is running it serves a loopback HTTP API on `127.0.0.1:24678`, guarded by an
`X-Soloist-Local-Auth` header on mutations and a loopback-only `Host` check. The command line client
is a thin wrapper over it. Packaged installs ship it as `soloist-cli`, because the desktop app owns
the bare `soloist` name, so add `alias soloist=soloist-cli` if you want the short form.

```bash
soloist-cli status [--status running|crashed]
soloist-cli start|stop|restart <name|all> [--project <name>]
soloist-cli logs <name> [-n <lines>]
soloist-cli spawn <tool> [--project <name>] [-- <args>...]
soloist-cli focus | soloist-cli open
soloist-cli remove-project <name>
```

Endpoints, payloads and the auth model are in [`docs/http-api.md`](docs/http-api.md).

## Project status

Soloist is released and usable. Supervision, terminals, agents, MCP, the coordination workspace,
Git, the HTTP API, the command line client and Ubuntu packaging are all built and shipping. Check
the [releases](https://github.com/ArtMin96/soloist/releases) for the current version.

Still open: some UX polish, and the final end-to-end parity walk and longevity soak gate.
[`PROGRESS.md`](PROGRESS.md) is the working ledger of what has been verified against what is only
written, and [`KNOWN-DIVERGENCES.md`](KNOWN-DIVERGENCES.md) records every place Soloist deliberately
went its own way, with the reason.

## Build from source

You need Ubuntu 22.04 or newer, Rust stable, Node 20+, pnpm, `cargo-tauri` and `just`. The system
libraries are listed in [`CONTRIBUTING.md`](CONTRIBUTING.md).

```bash
just setup      # install UI dependencies
just dev        # run the desktop app with hot reload
just test       # cargo test --workspace plus vitest
just lint       # rustfmt, clippy -D warnings, tsc, ESLint, dependency-direction guard
just deb        # build the .deb
just bundle     # build the .deb and the .AppImage
```

A bundle has to be built on the oldest system it targets, because glibc is backward compatible but
not forward compatible. The artifacts people download come from CI on `ubuntu-22.04`. Build locally
to inspect what a bundle contains, not to ship it.

If you already have Soloist installed and running, use `just dev-alongside`, which leaves the
installed copy alone.

## Architecture

Ports and adapters. `crates/core` is a pure domain core that imports no application framework: no
`tauri`, no `rmcp`, no `axum`, no `rusqlite`. CI fails the build if that changes. Every surface is a
thin adapter crate routing to the same core commands, so "restart" is written once and not
reimplemented per front end.

| Crate | Role |
|-------|------|
| `core` | The pure domain: bounded contexts, port traits, domain types |
| `app` | Tauri v2 desktop shell. Hosts the core and bundles the React and TypeScript UI |
| `mcp` | The `soloist-mcp` stdio MCP server |
| `httpapi`, `cli` | The loopback `axum` API and its command line client |
| `store` | SQLite-backed durable ports, with WAL and versioned migrations |
| `pty`, `exec`, `sys` | PTY spawning, process containment, OS probes for CPU, memory, ports and file watching |
| `git`, `forge` | The `git` and `gh` adapters |
| `ipc` | Unix socket transport between the app and the MCP server |

Start with [`ARCHITECTURE.md`](ARCHITECTURE.md), then
[`plan/04-engineering-architecture-and-patterns.md`](plan/04-engineering-architecture-and-patterns.md)
for the design contract and
[`plan/06-codebase-blueprint-and-cleanup.md`](plan/06-codebase-blueprint-and-cleanup.md) for where
code is supposed to live.

## Origins

Soloist started as a clean-room rebuild of [Solo](https://soloterm.com/) for Linux, because Solo is
macOS-only and closed source and there was no Linux build to use. No code, assets, icons, strings or
branding were copied. The behavior it reproduces was worked out from Solo's public documentation and
written down with sources in
[`plan/05-solo-reference-and-sources.md`](plan/05-solo-reference-and-sources.md). `solo.yml` is
compatible by specification so the file can move between the two, not because any code is shared,
and MCP tool names are mirrored for interoperability while the schemas behind them are ours.

It has grown well past where it started. The Git integration, the Mermaid diagrams, the theme
library and the orchestration surfaces are Soloist's own. "Soloist" and `dev.soloist.app` are
working names and not a trademark claim.

## Documentation

| Document | Covers |
|---|---|
| [`docs/mcp-setup.md`](docs/mcp-setup.md) | Per-client MCP setup and the security model |
| [`docs/http-api.md`](docs/http-api.md) | HTTP endpoints, payloads, and the command line client |
| [`docs/packaging.md`](docs/packaging.md) | The `.deb` and `.AppImage`, desktop integration, updates |
| [`docs/releasing.md`](docs/releasing.md) | The release pipeline |
| [`docs/diagnostics.md`](docs/diagnostics.md) | Profiling and diagnostic tooling |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | Toolchain, system libraries, workflow |
| [`ARCHITECTURE.md`](ARCHITECTURE.md), [`plan/`](plan/) | Design and behavior contracts |
| [`PROGRESS.md`](PROGRESS.md) | What is verified and what is pending |
| [Releases](https://github.com/ArtMin96/soloist/releases) | Release notes and downloads |

## License

Licensed under either [MIT](LICENSE-MIT) or [Apache License 2.0](LICENSE-APACHE), at your option.

Unless you state otherwise, any contribution you intentionally submit for inclusion in this work, as
defined in the Apache-2.0 license, is dual licensed as above with no additional terms.
