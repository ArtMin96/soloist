# Setting up MCP clients with Soloist

Soloist exposes its coordination workspace to AI coding agents over the Model Context Protocol.
The server is the `soloist-mcp` binary: an MCP client launches it and speaks to it over
stdin/stdout — there is no network host or port to configure. The helper connects to the running
desktop app over a private Unix socket in Soloist's data directory and forwards every tool call
to the same core command the UI and the HTTP API use.

This document is the setup reference for each supported client. The behavior contract is
[`plan/05` §7](../plan/05-solo-reference-and-sources.md); the parity rows are F1 to F14 in
[`plan/02`](../plan/02-feature-parity-matrix.md). The Settings → Integrations panel generates
these snippets with the resolved paths filled in — prefer copying from there.

## How it runs

- **Transport: stdio only.** The client launches `soloist-mcp` per session; nothing listens on a
  port. Until packaged installs land, the helper is a build artifact next to the `soloist`
  binary — the Integrations panel resolves that sibling path and generates snippets with it; if
  no sibling exists the snippet falls back to the bare name and `soloist-mcp` must be on `PATH`.
- **The app must be running.** The helper serves its tool list on its own (and the `help` tool
  answers app-down), but every other tool call needs the desktop app's socket. A call while the
  app is closed returns a clear "Soloist is not running" error.
- **Data directory.** The socket lives in `$SOLOIST_APP_DATA_DIR`, else `$XDG_DATA_HOME/soloist`,
  else `~/.local/share/soloist`. When you run the app with `SOLOIST_APP_DATA_DIR` set, the
  client-launched helper does not inherit that variable — the snippet must carry it, and the
  generated snippets do exactly that (an `env` entry appears only when the override is active).

## Security model

- The data directory (socket and database) is owner-only (`0700`), so other local users cannot
  reach the socket.
- Every session is identified by its connecting peer's process group. Scoped tools act only on
  the caller's effective project, and a bind or project selection that the peer does not actually
  run in is refused — one client cannot forge its way into a sibling project.
- Starting or restarting commands stays behind the trust gate in the core: an untrusted command
  is refused over MCP exactly as it is in the UI.

## Tool surface

The core groups are always served: identity (`whoami`, `bind_session_process`,
`register_agent`), projects, processes, bulk commands, output, services, lease locks, and
setup/support (`help`, `submit_solo_feedback`, `setup_agent_integration`). The feature groups —
Scratchpads, Todos, Timers, Key-Value — are toggled per group in Settings → Integrations
(Key-Value is off by default). The server reads the enablement when it starts, so a toggle
applies to the next client connection.

Two setup tools are worth knowing before anything else:

- `help` returns the agent usage guide (identity, scope, trust, timers, etiquette) and works
  even while the app is down.
- `setup_agent_integration` writes that guide into the current project's `AGENTS.md` (default)
  or `CLAUDE.md` as a marker-delimited managed section, so re-running updates it in place.

## Client configuration

Every snippet below registers the server under the name `soloist`. `<helper>` is the resolved
helper command from the Integrations panel (an absolute path when the helper sits next to the
app binary). Add the `env` entry only when you launch Soloist with `SOLOIST_APP_DATA_DIR` set —
the panel includes it automatically when that is the case.

### Claude Code

`.mcp.json` at the project root (shared, checked in), or `claude mcp add`:

```json
{
  "mcpServers": {
    "soloist": { "command": "<helper>" }
  }
}
```

### Codex

`~/.codex/config.toml`:

```toml
[mcp_servers.soloist]
command = "<helper>"

# only with a non-default data dir:
[mcp_servers.soloist.env]
SOLOIST_APP_DATA_DIR = "/custom/dir"
```

### Amp

`~/.config/amp/settings.json` (the same `amp.mcpServers` key works in VS Code `settings.json`):

```json
{
  "amp.mcpServers": {
    "soloist": { "command": "<helper>" }
  }
}
```

### OpenCode

`opencode.json` at the project root or `~/.config/opencode/opencode.json`. Note OpenCode's own
shape: the key is `mcp`, the command is an argv array, and the env key is `environment`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "soloist": { "type": "local", "command": ["<helper>"], "enabled": true }
  }
}
```

### Cursor

`.cursor/mcp.json` in the project, or `~/.cursor/mcp.json` globally:

```json
{
  "mcpServers": {
    "soloist": { "command": "<helper>" }
  }
}
```

### Windsurf

`~/.codeium/windsurf/mcp_config.json`, same `mcpServers` shape as Cursor.

### Cline

Open the MCP settings from the Cline panel (MCP Servers → Configure — the file is
`cline_mcp_settings.json` in the editor's storage), same `mcpServers` shape as Cursor.

### Claude Desktop

Not supported: Claude Desktop ships for macOS and Windows only, and a stdio server must run on
the same machine as the app it connects to — there is no working configuration against a
Linux-only Soloist.

## Troubleshooting

- **"Soloist is not running"** — the desktop app is closed, or the helper resolved a different
  data directory than the app. If you set `SOLOIST_APP_DATA_DIR` for the app, make sure the
  snippet carries the same value.
- **Tools missing from the list** — the group is toggled off in Settings → Integrations, or the
  client connected before you toggled it; reconnect (relaunch the client's MCP session) to pick
  up the change.
- **`spawn soloist-mcp ENOENT`** — the helper is not on `PATH` and the snippet used the bare
  name. Point `command` at the binary next to `soloist` (the Integrations panel resolves this
  for you).

## References

- MCP specification: <https://modelcontextprotocol.io/docs>
- Client documentation: [Claude Code](https://code.claude.com/docs/en/mcp),
  [Codex](https://developers.openai.com/codex/mcp), [Amp](https://ampcode.com/manual),
  [OpenCode](https://opencode.ai/docs/mcp-servers/),
  [Cursor](https://cursor.com/docs/context/mcp),
  [Windsurf](https://docs.devin.ai/desktop/cascade/mcp),
  [Cline](https://docs.cline.bot/mcp/configuring-mcp-servers)
