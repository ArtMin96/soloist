# Soloist HTTP API and the `soloist` CLI

Soloist exposes a small HTTP API on the loopback interface so a shell script or a launcher can
read and control the running stack. The `soloist` command-line tool is a thin client of that API:
it issues one HTTP request per command. Both the API and the CLI route every action to the same
core command the desktop UI and the MCP server already use, so a restart triggered from a script
behaves exactly like the restart button in the window.

This document is the reference for the wire surface. The behavior contract it implements is
[`plan/05` §8](../plan/05-solo-reference-and-sources.md); the parity rows are H1 to H4 in
[`plan/02`](../plan/02-feature-parity-matrix.md).

## How it runs

The server is compiled into the desktop app behind the `http` Cargo feature (on by default) and
runs as a supervised task inside the app process. There is no separate API binary to launch: when
Soloist is running, the API is up.

It binds `127.0.0.1:24678`. If that port is taken, it tries the next 16 ports in order, and if all
of those are taken it asks the operating system for any free loopback port. The port it actually
bound is written to a runtime file so the CLI can find it after a fallback:

```
$SOLOIST_APP_DATA_DIR/http-api.json     # default: ~/.local/share/soloist/http-api.json
```

The file holds one field, `{ "port": 24678 }`, and the server rewrites it on every bind, so a
present file always names the live port. If no loopback port can be bound at all, the API is
disabled with a logged message and the rest of the app keeps running.

## Security model

The API is local-only by three independent measures:

- **Loopback bind.** The server listens on `127.0.0.1` only, so it is never reachable from another
  machine.
- **Localhost CORS.** A browser may call the API only from a page served by `localhost`,
  `127.0.0.1`, or `[::1]`. A page on the wider web that the user happens to be viewing cannot script
  the loopback server.
- **Mutation auth header.** Every mutating request must carry `X-Soloist-Local-Auth: 1`. A request
  without it, or with a different value, is rejected with `401` before the handler runs. Reads carry
  no header and stay open on loopback.

The header is a deliberately weak local gate. Its job is to stop a drive-by request from a page the
user is merely viewing, not to authenticate a remote caller. There is no remote access and no
stronger auth, by design.

## Conventions

- **Base URL:** `http://127.0.0.1:<port>`, where `<port>` is the value in the runtime file (default
  `24678`).
- **Bodies:** responses are JSON. Mutating requests take no body; the header carries the only input
  beyond the URL.
- **Response types:** read responses reuse the core read-model types, so the JSON shape is the same
  one the UI and MCP see. The field-by-field definitions are in [Response types](#response-types).

### Status codes

A mutation returns one status code and no body. The mapping is enforced in the adapter from the
core outcome:

| Status | Meaning |
|--------|---------|
| `200 OK` | The command succeeded. `stop` and `stop-all` are idempotent, so stopping something already at rest is also `200`. |
| `401 Unauthorized` | The `X-Soloist-Local-Auth` header was missing or wrong (mutations only). |
| `403 Forbidden` | The command is not trusted. Trust it in Soloist first. |
| `404 Not Found` | No process or project with that id. |
| `500 Internal Server Error` | A durable-store read or write failed. |

Reads do not use `403`/`404`: an unknown id reads as an empty result (see each endpoint). The only
read codes are `200`, and `500` for `/status`, `/projects`, and `/feedback` if the store cannot be read.

## Read endpoints

Open on loopback; no header required.

| Method | Path | Returns |
|--------|------|---------|
| `GET` | `/health` | Liveness and the running version. |
| `GET` | `/status` | A cross-project process tally. |
| `GET` | `/processes` | The live process list. |
| `GET` | `/processes/{id}/ports` | The TCP ports a process is listening on. |
| `GET` | `/processes/{id}/output` | A process's recent output lines. |
| `GET` | `/projects` | The open projects. |
| `GET` | `/feedback` | Locally stored agent feedback, oldest first. |

### `GET /health`

Confirms the client reached Soloist and reports which build answered. `version` is the running
build's version.

```json
{ "ok": true, "version": "0.1.0" }
```

### `GET /status`

A small summary for a shell to glance at without reading every row: how many projects are open, how
many processes exist, and how many are running.

```json
{ "projects": 1, "processes": 3, "running": 2 }
```

### `GET /processes`

The live process read model as a JSON array of [`ProcessView`](#processview).

```json
[
  {
    "id": 7,
    "project": 1,
    "kind": "Command",
    "label": "web",
    "status": "Running",
    "exit_code": null,
    "requires_trust": false,
    "ports": [3000],
    "ready": "Ungated"
  }
]
```

### `GET /processes/{id}/ports`

The TCP ports the process is currently listening on, as an array of port numbers. An unknown id has
no row and reads as `[]`.

```json
[3000]
```

### `GET /processes/{id}/output`

The process's most recent rendered output lines, oldest first, as an array of strings. The optional
`lines` query parameter requests at most that many lines; omitting it uses the API's recent window.
The default count and the ceiling are enforced in the core, the same way the MCP output tools read.
An unknown id has no buffer and reads as `[]`.

```
GET /processes/7/output?lines=2
```

```json
["Compiled successfully.", "Listening on http://localhost:3000"]
```

### `GET /projects`

Every open project's display identity, as a JSON array of [`ProjectView`](#projectview).

```json
[
  { "id": 1, "name": "storefront", "root": "/home/you/projects/storefront", "icon": null }
]
```

### `GET /feedback`

Every feedback entry agents left via the `submit_solo_feedback` MCP tool, oldest first.
Feedback is stored locally and never transmitted; this endpoint is how the owner reads it back.

```json
[
  { "id": 1, "message": "the sidebar flickers", "submitted_unix_millis": 1783000000000 }
]
```

## Mutation endpoints

Every endpoint below requires the `X-Soloist-Local-Auth: 1` header and returns a
[status code](#status-codes) with no body. Each one delegates to a single core command, the same one
the UI button and the MCP tool drive.

| Method | Path | Action |
|--------|------|--------|
| `POST` | `/processes/{id}/start` | Start one process. |
| `POST` | `/processes/{id}/stop` | Stop one process (idempotent). |
| `POST` | `/processes/{id}/restart` | Restart one process. |
| `POST` | `/projects/{id}/start-auto` | Start the project's `auto_start` commands. |
| `POST` | `/projects/{id}/start-all` | Start every trusted command in the project. |
| `POST` | `/projects/{id}/stop-all` | Stop every live process in the project. |
| `POST` | `/projects/{id}/restart-running` | Restart only the running processes. |
| `POST` | `/projects/{id}/restart-all` | Bring the trusted command set up fresh. |
| `POST` | `/projects/{id}/reload` | Re-read `solo.yml` and reconcile the command set. |
| `POST` | `/projects/{id}/spawn-agent` | Launch a known agent tool as a worker (JSON body; returns the new id). |
| `POST` | `/projects/{id}/transfer-todo` | Move a todo from this project to another (JSON body). |
| `POST` | `/projects/{id}/transfer-scratchpad` | Move a scratchpad from this project to another (JSON body). |
| `POST` | `/focus` | Raise the desktop window to the front. |

The two bulk-start scopes are distinct on purpose. `start-auto` starts only the commands marked
`auto_start` (the dashboard's launch-the-stack action). `start-all` starts every trusted command
regardless of `auto_start`. `restart-running` cycles only what is already running, while
`restart-all` cycles the running commands and starts the resting ones too.

`/focus` is the one action that does not go through the core, because the core has no window. The
composition root supplies the window-raising callback, so the API crate stays free of any UI
dependency.

`/projects/{id}/reload` re-reads the project's `solo.yml` and reconciles the registered command
set to it, **without ever killing running work**: a command added to the file is registered resting
(untrusted until you trust its variant — reload never starts anything); a changed command's spec is
updated in place, keeping its process id (so a reload never duplicates a command) — if it is running
it keeps running until its next restart, which the trust gate re-checks; a renamed command is
relabelled in place, preserving trust; a removed command is dropped only if it is resting, and left
running otherwise. A byte-identical file is a no-op success; an unknown project is a `404`.

`/projects/{id}/spawn-agent` is the one mutation that carries a request body and returns one. Post
`{ "tool": "<name>", "args": [] }` — `tool` is an entry in the app's agent-tool registry (e.g.
`Claude`), `args` are optional extra command-line arguments — and it launches that agent as an
ungated worker in the project, replying `{ "id": <process id> }`. This is the local user's authority
on the loopback socket, the same `launch_agent` the desktop launch picker drives; the spawned agent
is a root process. An unknown tool or project is a `404`. (The session-scoped MCP `spawn_agent`,
which nests a worker under a bound lead, stays MCP-only — and additionally refuses a caller that
is itself a spawned worker, since delegation is one level deep; this local route carries the
user's own authority and no such gate.)

`/projects/{id}/transfer-todo` and `/projects/{id}/transfer-scratchpad` move a coordination
aggregate from the path (source) project to another. Post `{ "todo": <id>, "to_project": <id> }` or
`{ "name": "<name>", "to_project": <id> }`. The move keeps the aggregate's document, tags, and
durable id (a todo also keeps its comments and completion but clears its blockers and lock, which
reference the source project). This is the local user's authority on the loopback socket — the same
`*_transfer_in` core path a desktop board drives — so it addresses **both** projects by explicit id;
the target project must be loaded (an unknown todo, scratchpad, or target project is a `404`, and a
scratchpad name already used in the target is a `409`). The session-scoped MCP `todo_transfer` /
`scratchpad_transfer` cannot reach across projects (a session is scoped to one project), so
cross-project transfers go through here or the desktop, never over MCP.

### Bulk endpoint to core mapping

Each project bulk endpoint maps to one supervisor command. The CLI and MCP use the same scopes.

| Endpoint | Core command | Scope |
|----------|--------------|-------|
| `start-auto` | `start_all` | The trusted `auto_start` subset. |
| `start-all` | `start_all_commands` | Every trusted command. |
| `stop-all` | `stop_all` | Every live process. |
| `restart-running` | `restart_running` | The running processes only. |
| `restart-all` | `restart_all_commands` | The trusted command set, running and resting. |

## Response types

These are the core read-model types, serialized as-is. Both ids serialize as plain numbers.

### `ProcessView`

| Field | Type | Meaning |
|-------|------|---------|
| `id` | number | The process id, unique within the current run. |
| `project` | number | The id of the project the process belongs to. |
| `kind` | string | `"Command"`, `"Agent"`, or `"Terminal"`. |
| `label` | string | The display name (the `solo.yml` key for a command). |
| `status` | string | One of `"Stopped"`, `"Starting"`, `"Running"`, `"Crashed"`, `"Restarting"`, `"Stopping"`, `"RestartExhausted"`. |
| `exit_code` | number or null | The most recent terminal exit code; `null` while running or when ended by a signal. |
| `requires_trust` | boolean | `true` for a trust-gated command whose variant is not yet trusted. The UI blocks its start until trusted. |
| `ports` | array of numbers | The TCP ports the process is listening on; empty until discovery finds any, cleared when its group ends. |
| `ready` | string | The readiness gate: `"Ungated"` (the default), `"Waiting"` (a port wait is in effect and the port has not bound), or `"Ready"`. |

### `ProjectView`

| Field | Type | Meaning |
|-------|------|---------|
| `id` | number | The durable project id (stable across runs). |
| `name` | string | The `solo.yml` `name:` if set, else the project folder's name. |
| `root` | string | The project's root path. |
| `icon` | string or null | The `solo.yml` `icon:` loaded into a ready-to-render `data:` URL, or `null` when there is none or it cannot be read. |

## The `soloist` CLI

`soloist` controls the local stack from a shell over the API above. It reads the runtime file to
find the port (falling back to the default `24678` when the file is absent), sends the auth header on
every mutation, and prints one line of result. A success goes to stdout with exit code `0`; an error
goes to stderr as `soloist: <message>` with a non-zero exit code, so a script can branch on it.

The packaged installs ship it as **`/usr/bin/soloist-cli`** — the desktop app owns the bare
`soloist` name — so on an installed system the commands below read `soloist-cli status`,
`soloist-cli logs web`, and so on (`KNOWN-DIVERGENCES.md` D-14; `alias soloist=soloist-cli`
restores the short form). A dev checkout runs it as `cargo run -p soloist-cli --`.

When the API cannot be reached (almost always because the app is not running), every command prints
`soloist: Soloist is not running` and exits non-zero.

`soloist --help` lists the subcommands and is generated from the definitions, so it is always current.

### Subcommands

```
soloist status [--status running|crashed]
soloist start   <name|all> [--project <name>]
soloist stop    <name|all> [--project <name>]
soloist restart <name|all> [--project <name>]
soloist logs    <name> [-n <count>]
soloist focus
```

A process is named by its `label`. Because a label is not guaranteed unique across projects, a name
that matches more than one process is refused with the matching ids rather than guessed. The bulk
form `all` acts on a whole project: the sole open project when one is open, or the project named by
`--project` when two or more are.

### Subcommand to endpoint map

| Command | Requests | Notes |
|---------|----------|-------|
| `status` | `GET /processes` | Filtered by `--status` and tabulated client-side. |
| `start <name>` | `GET /processes`, then `POST /processes/{id}/start` | Resolves the name to an id first. |
| `start all` | `GET /projects`, then `POST /projects/{id}/start-all` | Starts every trusted command in the project. |
| `stop <name>` / `stop all` | `POST /processes/{id}/stop` / `POST /projects/{id}/stop-all` | |
| `restart <name>` / `restart all` | `POST /processes/{id}/restart` / `POST /projects/{id}/restart-all` | |
| `logs <name>` | `GET /processes/{id}/output` | `-n <count>` becomes `?lines=<count>`. |
| `spawn <tool> [-- args]` | `GET /projects`, then `POST /projects/{id}/spawn-agent` | Resolves the project (sole, or `--project`), then launches the agent. |
| `focus` | `POST /focus` | |
| `open` | `POST /focus` | Solo's raise-app alias of `focus`; shares its handler. |

The CLI's `start all` and `restart all` use the `start-all` and `restart-all` scopes. The
`start-auto` and `restart-running` scopes are reachable directly over HTTP when a script needs them.

### Examples

```
$ soloist status
ID  NAME   KIND     STATUS   PORTS
7   web    command  running  3000
8   build  command  stopped  -

$ soloist restart web
Restarted "web".

$ soloist logs web -n 1
Listening on http://localhost:3000

$ soloist status        # with the app quit
soloist: Soloist is not running
```

## Deferred endpoints

These were named in the original phase plan and are tracked deferrals, recorded in
[`plan/05` §12](../plan/05-solo-reference-and-sources.md). They are not part of the current surface.

- **`POST /projects/{id}/reload`.** A correct reload must re-read `solo.yml` and reconcile the
  supervisor's registrations in place, which needs a registration-reconcile path that does not exist
  yet. A naive "sync then restart" would restart with stale specs, so the endpoint is held for a
  focused follow-up.
- **`soloist spawn`.** Launching an agent over HTTP has no endpoint: the core's agent spawn is
  session-scoped and the loopback API has no session concept, so it needs its own scoping and trust
  design first.
- **`soloist open`.** Raising the app already overlaps `focus`, and opening a project folder needs a
  project-load endpoint that does blocking filesystem work and spawns actors. Deferred to keep the
  surface tight.

## References

- [`plan/05` §8](../plan/05-solo-reference-and-sources.md): the HTTP API and CLI behavior contract.
- [`plan/05` §12](../plan/05-solo-reference-and-sources.md): the gap decisions, including the bulk
  endpoint mapping, the auth and status mapping, the output endpoint, and the deferrals above.
- [`plan/02`](../plan/02-feature-parity-matrix.md): parity rows H1 to H4.
- [`plan/06` §5.4](../plan/06-codebase-blueprint-and-cleanup.md): the recipe for adding an HTTP
  endpoint or CLI command, and why all three frontends route to one core command.
- [`README.md`](../README.md): the `solo.yml` project file the processes above come from.
