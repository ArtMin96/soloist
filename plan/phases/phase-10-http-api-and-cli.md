# Phase 10 — Local HTTP API & `soloist` CLI

**Goal:** A loopback HTTP API (`127.0.0.1:24678`, ref §8) for local automation, plus a `soloist` CLI
that is a **thin client** of it — so the stack is controllable from a shell or a launcher, exactly like
Solo's CLI + Raycast HTTP API.

**Delivers:** H1–H4. **Architecture:** the HTTP adapter (`crates/httpapi`, hosted in the app) and the
CLI (`crates/cli`) — both drive the **same core commands** as the UI and MCP (`04` §2).

## Scope
**In:** the `axum` loopback server inside the app; the documented read + mutation endpoints; localhost-
only CORS + mutation auth header; the `soloist` CLI over HTTP. **Out:** remote/networked access (loopback
only); auth beyond the local header (it's a local-only API).

## Tasks
1. **HTTP server (H1, ref §8):** `axum` bound to `127.0.0.1:24678` (port configurable only while
   disabled; auto-fallback if taken). Mutations require header **`X-Soloist-Local-Auth: 1`**; CORS
   limited to `localhost`/`127.0.0.1`. Runs as a supervised task in the app (`04` §6).
2. **Read endpoints (H2):** `GET /health`, `/status`, `/processes`, `/processes/:id/ports`, `/projects`
   → JSON projections from the core (`facade.snapshot()` etc.).
3. **Mutation endpoints (H3):** `POST /processes/:id/start|stop|restart`; `POST /projects/:id/start-all|
   stop-all|reload|start-auto|restart-running|restart-all`; `POST /focus` (raise the window). Each maps
   to a core command; honors the trust gate.
4. **`soloist` CLI (H4, ref §8):** subcommands over the HTTP API — `status` (process table, filterable by
   status), `start|stop|restart <name|all>`, `logs <name>` (recent output), `spawn`, `open`, `focus`.
   Resolves the port/auth from the app's runtime/config; clear error if the app isn't running.
5. **Shared projections:** reuse the `ProcessView`/`ProjectView` types so HTTP/CLI/MCP/UI stay
   consistent.
6. **Docs:** an API reference (endpoints, payloads, the auth header) + `soloist --help`.

## Interfaces
```
GET  /health -> {ok,version}        GET /status -> {projects,processes summary}
GET  /processes -> [ProcessView]    GET /processes/:id/ports -> [port]
GET  /projects  -> [ProjectView]
POST /processes/:id/{start|stop|restart}        (X-Soloist-Local-Auth)
POST /projects/:id/{start-all|stop-all|reload|start-auto|restart-running|restart-all}
POST /focus
```
```
soloist status [--status running|crashed] | start <name|all> | stop <name|all>
soloist restart <name|all> | logs <name> [-n N] | spawn <tool> | open | focus
```

## Acceptance criteria
- `GET /health` returns version; `GET /processes` returns the live stack as JSON.
- `POST /processes/:id/restart` **with** the auth header restarts; **without** it → 401/403.
- Requests from a non-localhost origin are rejected (CORS).
- `soloist status` prints the live process table from a shell; `soloist restart web` restarts that
  command; with the app down, the CLI prints a clear "Soloist is not running" message.
- The same restart triggered via UI, MCP, and HTTP produces identical core behavior.

## Test plan
- **Integration:** spin up the app (headless) with a fixture stack; hit each endpoint; assert auth/CORS
  enforcement and that mutations change real state (observed via events).
- **CLI:** drive `soloist status/start/stop/restart` against the fixture app; assert output + the
  app-down error path.

## Risks & mitigations
- **Port already in use** → documented auto-fallback; report the chosen port via `/health` + runtime
  file (CLI reads it).
- **Local auth is weak by design** → loopback-only bind + CORS + header; never expose beyond localhost.
- **Drift between API and core** → API is a thin adapter over `facade`; contract tests pin it.

## Effort
~3–5 days.
