# Diagnostics & audit tooling

A small set of detectors keeps the Soloist app fast and issue-free, plus a way to drive them
**from Claude Code** (see §4). Each covers a different layer, and **none ships** — they are opt-in
dev features, dev/CI-only binaries, or agent skills, all absent from `default` and release builds.

| Tool | Layer it covers | Run with | Detects |
|------|-----------------|----------|---------|
| **cargo-deny** | Rust dependency tree | `just audit` | known vulnerabilities, unmaintained crates, disallowed licenses, untrusted sources |
| **CrabNebula DevTools** | IPC bridge (Rust ⇄ webview) | `just devtools` | slow `invoke` commands, event flow, tracing spans, logs |
| **tokio-console** | async runtime (the actors) | `just tokio-console` | stalled tasks, long polls, lock contention, task leaks |

> Reaching for the right one: a "the app feels slow" symptom usually lives in exactly one
> layer — a slow Rust command (CrabNebula), a task blocking the runtime (tokio-console), or a
> webview render (the browser DevTools / `just ui-analyze`). Use the matching tool to bisect
> instead of guessing.

These complement what already exists: `just lint` (clippy correctness + the `clippy::perf`
group), `just bloat` / `just bundle-size` / `just ui-analyze` (size), and `just soak` (the
runtime leak gate).

---

## 1. cargo-deny — supply-chain & advisory gate

Scans the whole Rust dependency tree against the [RustSec advisory DB](https://rustsec.org/)
plus license and source policy. This is the "is the app issue-free *today*?" check, and it
runs on every pull request in CI.

**One-time install**

```sh
cargo install --locked cargo-deny
```

**Run**

```sh
just audit          # = cargo deny check  (advisories + licenses + bans + sources)
cargo deny check advisories   # just one category, when triaging
```

Green output is `advisories ok, bans ok, licenses ok, sources ok`.

**The policy lives in `deny.toml`** (repo root). What it enforces:

- **Advisories** — vulnerabilities and unsound advisories fail the gate *everywhere*.
  *Unmaintained* advisories are scoped to crates Soloist depends on **directly**
  (`unmaintained = "workspace"`): the transitive tree is almost entirely Tauri's GTK3 Linux
  GUI bindings, which are upstream-unmaintained with no fix available here. A crate **you** add
  directly that is unmaintained still fails.
- **Licenses** — an allow-list of permissive licenses matching Soloist's own MIT/Apache-2.0.
  Anything outside it fails until reviewed and added.
- **Bans** — duplicate versions and wildcards are reported (`warn`), not failed.
- **Sources** — only crates.io is allowed; an unknown registry or git remote fails.

**When the gate fails, you have two honest moves:**

1. *Fixable* (a vulnerability with an upgrade, a new dependency with a copyleft license you
   don't want): fix it — bump the crate, drop the dependency, or find an alternative.
2. *Unfixable and accepted* (a vulnerability deep in an upstream tree, no patched version): add
   it to `deny.toml` **with a written reason** so the acceptance is reviewable:

   ```toml
   [advisories]
   ignore = [
       { id = "RUSTSEC-YYYY-NNNN", reason = "upstream <crate>; no patched release yet, low risk because …" },
   ]
   ```

   To allow a new permissive license, add its SPDX id to `[licenses] allow`.

Never silence a finding without a reason in the file — the reason *is* the audit trail.

---

## 2. CrabNebula DevTools — IPC command profiler

The Tauri-native instrumentation (`tauri-plugin-devtools`). It captures the app's logs and
tracing spans and opens a real-time viewer showing **every `invoke` command with its timing**,
event payloads/responses, and execution spans. Use it to find which command is slow or chatty.

**Run**

```sh
just devtools       # = cargo tauri dev --features devtools
```

The viewer opens automatically; watch the command list while you drive the UI. The `devtools`
feature is opt-in and never in `default`, so it cannot leak into a release build.

**When to reach for it:** a UI action feels laggy and you suspect the Rust side — confirm by
reading the command's measured duration instead of guessing.

---

## 3. tokio-console — async runtime inspector

A live view of the tokio runtime that backs every supervised process actor and sampler. Shows
each task's state, busy/poll time, wakers, and source location, and flags **tasks that poll too
long (runtime stalls)** and **lock contention**.

**One-time install** (the CLI that renders the view)

```sh
cargo install --locked tokio-console
```

**Run** — two terminals:

```sh
# Terminal 1 — the app, instrumented (sets --cfg tokio_unstable for you):
just tokio-console      # = RUSTFLAGS="--cfg tokio_unstable" cargo tauri dev --features tokio-console

# Terminal 2 — attach the console (connects to localhost:6669):
tokio-console
```

The `tokio-console` feature pulls in `tokio/tracing` and is opt-in only. The build uses the
`tokio_unstable` cfg, which forces a one-time recompile of tokio and its users — expected.

**When to reach for it:** chasing a task or FD leak (pair it with `just soak`), a runtime that
won't go idle, or to *see* the actor-per-process model holding the task/FD/PID-conservation
invariant during a start/stop loop.

> **Not at the same time as DevTools.** Both install a global tracing subscriber, so enabling
> `devtools` and `tokio-console` together is a compile error. Run one diagnostic at a time.

---

## 4. Driving all this from Claude Code

The detectors above split into two groups for an agent:

- **Agent-drivable** (text/CLI → Claude reads output and fixes): cargo-deny, clippy, the soak
  gate, bundle/bloat, and react-doctor. Claude runs these, interprets them, and applies fixes.
- **Human-visual** (live GUI/TUI → Claude can't read): CrabNebula DevTools and tokio-console.
  Claude can set them up and act on what *you* report, but it can't watch them. The exception is
  the Tauri MCP bridge below, which exposes IPC inspection to the agent in a readable form.

### react-doctor — React frontend skill

**Not installed by default.** Install it with `npx -y skills add millionco/react-doctor` (it
lands in `.agents/skills/`, symlinked into `.claude/skills/`; commit it if the whole team should
have it, as was done for `shadcn`). Once installed, invoke the react-doctor skill (or ask "scan
the React code") and Claude scans the frontend for lint, accessibility, bundle, and architecture
issues, applies fixes, then re-scans. This is the only tool that reads your React **component**
code — the Rust-side tools never touch it.

> Don't confuse it with Claude Code's **built-in `/doctor` command**, which diagnoses the Claude
> Code installation itself, not React code. The installer may also add react-doctor's
> rule-authoring skills and a few general ones; they're harmless (skills only trigger on relevant
> prompts) — prune any you don't want from `.agents/skills/`.

### `/soloist-diagnose` — the whole-app detect → fix → re-verify loop

A project skill (`.claude/skills/soloist-diagnose/`). Ask Claude to "optimize / health-check /
find and fix issues," or run `/soloist-diagnose [deps|lints|perf|leaks|frontend]`. It runs the
agent-drivable gates (`just audit`, clippy, `just soak`, `just bloat`), applies fixes or accepts
advisories with written reasons, delegates React to react-doctor (offering to install it if
absent), and **re-runs each gate to prove the fix**. It stays inside the architecture and the §6 locked non-changes, never weakens a test,
and never edits `PROGRESS.md` unless asked.

### Tauri MCP bridge — agent-readable IPC inspection (`just agent-bridge`)

For the Tauri IPC/UI layer, the [hypothesi mcp-server-tauri](https://hypothesi.github.io/mcp-server-tauri/)
MCP lets Claude inspect IPC calls and drive the webview — the agent-readable counterpart to the
CrabNebula DevTools GUI. Two parts, both already wired:

1. **App side** — the dev-only `agent-bridge` feature adds `tauri-plugin-mcp-bridge`, and
   `tauri.dev.conf.json` enables `withGlobalTauri` + the `mcp-bridge` capability. Neither enters a
   release build. Start it with:

   ```sh
   just agent-bridge      # cargo tauri dev --features agent-bridge --config tauri.dev.conf.json
   ```

   This opens a bridge on `ws://localhost:9223`.

2. **Claude side** — the `tauri-bridge` MCP server (`npx -y @hypothesi/tauri-mcp-server`) is
   registered in this project's local Claude Code config. It appears as agent tools **after a
   Claude Code restart**. With the app running via `just agent-bridge`, Claude can then inspect
   IPC calls, read console logs, and drive the webview.

> **Security.** The bridge grants the agent broad webview access (JS execution, IPC, DOM) and is
> an *unofficial* community plugin. It is strictly dev-only and gated three ways (cargo feature +
> dev-only config + Claude-Code approval). Run `just agent-bridge` only in a trusted session, and
> never enable the `agent-bridge` feature or `tauri.dev.conf.json` in a build you ship.

### Quick map

| I want Claude to… | Use |
|---|---|
| Fix React component issues | the react-doctor skill (install first, see above) |
| Audit + fix deps, lints, leaks, size across the app | `/soloist-diagnose` |
| Inspect IPC calls / drive the running app | `just agent-bridge` + the `tauri-bridge` MCP |
| Deep-dive a slow command or runtime stall myself | `just devtools` / `just tokio-console` |
