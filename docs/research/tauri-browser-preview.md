# Tauri browser preview: official options and Soloist recommendation

## Question

How can Soloist's existing React/Vite frontend be opened in a normal browser for fast UI work,
why does `http://localhost:1420` currently fail, and which official Tauri approach best preserves
the app's actual UI without turning Soloist into a web application?

**Route:** Hybrid. Tauri and Vite define the available browser/runtime mechanisms; Soloist's
existing IPC, window, file-drop, event, terminal-stream, and HTTP boundaries determine which one
fits.

## Answer

`http://localhost:1420` is already the correct Vite development URL. Tauri opens that URL inside
its own WebView during development; opening it directly in Chrome or Firefox loads the same React
assets but not the Tauri runtime that injects window/webview metadata and the IPC bridge. The
reported crash is therefore expected from the current code, not a Vite routing failure.

For Soloist's stated goal—work on the exact renderer more easily—the best official option is a
**development-only, fixture-backed browser mode** using `@tauri-apps/api/mocks`. It should render
the production `App` and components unchanged, install `mockWindows("main")`, intercept commands
with `mockIPC`, enable mocked events, and provide deterministic project/process/settings fixtures.
Native window controls and OS file drops should remain visible but become no-ops in browser mode.
This gives the same component tree, styles, layout, responsive behavior, xterm renderer, Chrome
DevTools, and Vite HMR; it deliberately does **not** claim live Rust/native behavior.

If "exact" instead means a normal browser driving the live Rust supervisor and PTYs, no Tauri
switch provides that. It requires a second browser transport—HTTP for request/response commands
and a streaming transport such as WebSocket for domain events and PTY bytes—plus equivalent
authentication and authorization. Soloist has a secure loopback HTTP adapter already, but it is a
small automation surface rather than a complete frontend backend. Building feature parity there
would be a substantial product/security change, not a UI-development convenience.

## Why the current page crashes

Soloist explicitly configures the Vite server and Tauri `devUrl` to the same address:
[`vite.config.ts`](../../crates/app/ui/vite.config.ts#L55-L60) and
[`tauri.conf.json`](../../crates/app/tauri.conf.json#L7-L11). This matches Tauri's official Vite
setup: the Tauri CLI uses the Vite development server as the WebView's development URL
([Tauri Vite guide](https://v2.tauri.app/start/frontend/vite/)). Vite itself describes `vite` as a
development server and exposes `--open` only as a convenience for opening that served page in a
browser ([Vite CLI](https://vite.dev/guide/cli#dev-server)). Neither statement says a normal browser
gains a Tauri backend.

The two stack traces map exactly to two native boundaries:

- [`lib/window.ts`](../../crates/app/ui/src/lib/window.ts#L20-L27) calls `getCurrentWindow()` for
  maximize/resize state. The installed official API implementation reads
  `window.__TAURI_INTERNALS__.metadata.currentWindow.label`
  ([package source](../../crates/app/ui/node_modules/@tauri-apps/api/window.js#L84-L88)). A plain
  browser has no injected `metadata`, hence the reported `reading 'metadata'` exception.
- [`lib/fileDrop.ts`](../../crates/app/ui/src/lib/fileDrop.ts#L22-L23) calls
  `getCurrentWebview()`, whose official implementation reads the same current-window metadata and
  current-webview label
  ([package source](../../crates/app/ui/node_modules/@tauri-apps/api/webview.js#L27-L31)).
  `FileDropProvider` installs that subscription at app mount
  ([source](../../crates/app/ui/src/store/FileDropProvider.tsx#L64-L89)).

Tauri publishes `isTauri(): boolean` as the supported runtime probe
([API reference](https://v2.tauri.app/reference/javascript/api/namespacecore/#istauri)); Soloist's
installed implementation reads the Tauri-set global flag
([package source](../../crates/app/ui/node_modules/@tauri-apps/api/core.js#L278-L281)). Runtime
detection prevents native-only boundaries from executing accidentally, but it does not supply app
data or emulate commands.

## Official options compared

| Option | Same renderer | Live Rust/native behavior | Fit for manual UI work | Conclusion |
|---|---|---|---|---|
| Vite URL alone (`pnpm dev`, browser at `:1420`) | Yes | No | Broken until native boundaries and startup IPC are handled | Necessary host, not a complete solution |
| Tauri development window using the Vite `devUrl` | Yes | Yes | Good HMR, but still a native WebView window | Use when verifying real behavior |
| `@tauri-apps/api/mocks` in a browser-only bootstrap | Yes | No; fixture behavior only | Excellent | **Recommended for day-to-day UI iteration** |
| WebdriverIO Tauri browser mode | Yes | No; per-command mocks | Excellent for automated renderer tests | Reuse its model; optional as the manual launcher |
| HTTP plus WebSocket browser adapter | Yes | Yes, if the full surface is implemented | High effort and high security impact | Do only if a real web client becomes a product goal |
| Tauri remote capability URLs | Inside a Tauri WebView only | Tauri-controlled | Does not connect an external Chrome/Firefox tab | Not applicable |
| `vite preview` | Production-built renderer | No | Useful for static bundle inspection only | It still cannot provide Tauri APIs |

### Official mock/browser-mode boundary

Tauri's official mock guide says `mockIPC` intercepts command calls and can simulate backend
results; event support is opt-in. It also says plainly that the mock runtime runs no real WebView or
Rust backend ([Mock Tauri APIs](https://v2.tauri.app/develop/tests/mocking/)). `mockWindows` is the
specific official helper that creates the `metadata` required by `getCurrentWindow`; it mocks only
window presence, while window properties must be answered through `mockIPC`
([mock API reference](https://v2.tauri.app/reference/javascript/api/namespacemocks/#mockwindows)).

The official Tauri WebDriver guide now documents a browser mode that runs the frontend in plain
Chrome against a Vite server and intercepts `invoke()`
([Tauri WebDriver guide](https://v2.tauri.app/develop/tests/webdriver/)). Its linked implementation
guide defines the boundary precisely: same frontend code path and standard browser DevTools/HMR,
but mocked Rust commands; no plugin bridge, real command round-trips, native window management, or
deep links
([WebdriverIO browser mode](https://github.com/webdriverio/desktop-mobile/blob/main/packages/tauri-service/docs/browser-mode.md)).

This disconfirms the tempting interpretation that official browser mode makes the whole Tauri app
run in Chrome. It makes the **renderer** run in Chrome.

### Why HTTP/WebSocket is a different project

Tauri's process model places operating-system access and all Tauri IPC in the Core process; a
WebView is browser-like, but it is managed by that Core
([Tauri process model](https://v2.tauri.app/concept/process-model/)). Tauri commands and events are
IPC primitives between that Core and its WebView, not a public network protocol
([Tauri IPC](https://v2.tauri.app/concept/inter-process-communication/)). A normal browser therefore
needs an actual network-facing adapter.

Soloist's typed UI boundary uses commands, events, and a Tauri `Channel` for binary PTY frames
([`api.ts`](../../crates/app/ui/src/api.ts#L1-L7),
[`api.ts`](../../crates/app/ui/src/api.ts#L667-L691), and
[`api.ts`](../../crates/app/ui/src/api.ts#L1068-L1079)). Its current HTTP adapter exposes selected
automation reads and mutations, all behind localhost CORS, a loopback `Host` guard, and a
per-launch token ([routes](../../crates/httpapi/src/routes.rs#L17-L42),
[CORS](../../crates/httpapi/src/cors.rs#L11-L23), and
[authentication](../../crates/httpapi/src/auth.rs#L21-L57)). It has no domain-event or PTY streaming
route and does not mirror the full typed UI surface.

Extending that adapter for a browser must preserve these boundaries. CORS is not authentication;
the existing code correctly treats the token as the user boundary and the `Host` check as DNS
rebinding protection. The token is intentionally stored where an ordinary browser cannot read it,
so a browser client also needs an explicit, safe session-establishment design. A generic endpoint
that blindly forwards arbitrary Tauri command names would bypass the project's narrow-adapter and
least-authority model.

Tauri's "remote API access" capability is not a shortcut. The official capability documentation
says it grants selected commands to remote sources associated with Tauri windows/WebViews and that
the API is otherwise limited to bundled app code
([Capabilities](https://v2.tauri.app/security/capabilities/#remote-api-access)). It does not inject
the Tauri core into a separate system-browser process. Likewise, `withGlobalTauri` only changes how
code already running in Tauri's frontend imports the API; it does not create a backend in Chrome.

## Recommended implementation shape for Soloist

This is a recommendation, not code written by this research task:

1. Add an explicit `dev:browser` command that runs Vite with a browser-preview environment flag.
   Keep normal `pnpm dev`, `just dev`, production builds, and Tauri behavior unchanged.
2. In that mode only, prepend a small bootstrap before `main.tsx`, following the same Vite-plugin
   pattern Soloist already uses for its e2e bridge. The official browser-mode guide warns that
   mocks injected after page load miss startup calls; a prepended module avoids that race.
3. In the bootstrap, call `mockWindows("main")`, then `mockIPC(fixtureDispatcher,
   { shouldMockEvents: true })`. Answer native window-property commands with stable values; make
   window actions and OS file-drop subscriptions no-ops.
4. Keep fixtures and state transitions in a browser-preview module, never in `App.tsx`, components,
   domain types, or Rust core. Reuse the same `App`, `domain.ts`, `api.ts`, stores, and components so
   the preview cannot become a second UI implementation.
5. Seed representative states that matter for visual work: multiple projects, each process kind
   and status, terminal output, trust/error/empty states, coordination data, Git changes, light/dark
   themes, and narrow/wide layouts. Mock mutations should update fixture state and emit the same
   domain events where interaction fidelity matters.
6. Keep native acceptance checks. Browser preview cannot validate Tauri window controls, OS file
   paths from drag/drop, dialogs, clipboard/store/opener plugins, capabilities, focus semantics,
   real PTY flow, or WebKitGTK-specific rendering.

This is the minimum architecture-preserving solution: one development adapter at the existing
frontend boundary, zero alternate business logic, and no new externally reachable backend.

## Contradictions and caveats

- "Same app in browser" has two meanings. The same renderer is officially supported through
  mocks; the same live native behavior is not.
- A mock that returns `undefined` for every command may make the first crash disappear but will not
  render meaningful Soloist state. Startup reads need typed fixture responses.
- `mockWindows` supplies labels, not true window dimensions, focus, maximize state, or events.
- Standard DOM drag/drop cannot reproduce Tauri's native absolute filesystem paths; browser mode
  should no-op or use synthetic fixtures for this boundary.
- Chrome is useful for UI iteration, but Soloist ships on Linux WebKitGTK. Final visual and runtime
  verification still belongs in the Tauri window.
- `vite preview` serves a built static bundle and is explicitly not a production server
  ([Vite CLI](https://vite.dev/guide/cli#vite-preview)); it changes neither the runtime boundary nor
  the answer.
- Exposing Vite beyond localhost is unnecessary here. Vite warns that broad host/CORS settings can
  expose source through DNS rebinding; keep the current localhost/strict-port settings
  ([Vite server options](https://vite.dev/config/server-options)).

## Coverage ledger

| # | Sub-question | Status | Evidence |
|---|---|---|---|
| 1 | What does `localhost:1420` represent? | ANSWERED | Tauri Vite guide, Vite CLI, local Vite/Tauri config |
| 2 | How should runtime availability be detected? | ANSWERED | Tauri `isTauri` API and installed source |
| 3 | What can official mocks/browser mode emulate? | ANSWERED | Tauri mock guide/API, Tauri WebDriver guide, browser-mode implementation guide |
| 4 | Which native APIs cause the crash? | ANSWERED | Full local window/file-drop definitions and installed official API implementations |
| 5 | Can a browser use the live backend through Tauri IPC? | ANSWERED | Tauri process model, IPC docs, capabilities docs |
| 6 | Would HTTP/WebSocket fit, and what is the security cost? | ANSWERED | Local HTTP routes/CORS/auth plus UI command/event/channel boundary |

## Sources

Primary/official sources only:

1. [Tauri: Vite frontend configuration](https://v2.tauri.app/start/frontend/vite/) — how `devUrl`
   points a Tauri development WebView at Vite.
2. [Tauri: Mock Tauri APIs](https://v2.tauri.app/develop/tests/mocking/) — `mockIPC`, event mocks,
   and the explicit no-Rust/no-WebView boundary.
3. [Tauri: WebDriver](https://v2.tauri.app/develop/tests/webdriver/) — official renderer-only browser
   mode and native-mode alternative.
4. [WebdriverIO: Tauri browser mode](https://github.com/webdriverio/desktop-mobile/blob/main/packages/tauri-service/docs/browser-mode.md)
   — operational behavior and limitations of the browser-mode implementation linked by Tauri.
5. [Tauri JavaScript API: core](https://v2.tauri.app/reference/javascript/api/namespacecore/#istauri)
   — `isTauri`, `invoke`, and `Channel` contracts.
6. [Tauri JavaScript API: mocks](https://v2.tauri.app/reference/javascript/api/namespacemocks/)
   — `mockWindows`, `mockIPC`, and `clearMocks` contracts.
7. [Tauri: process model](https://v2.tauri.app/concept/process-model/) and
   [IPC](https://v2.tauri.app/concept/inter-process-communication/) — why Core/WebView IPC is not a
   browser network API.
8. [Tauri: capabilities](https://v2.tauri.app/security/capabilities/#remote-api-access) — remote
   origin access remains scoped to Tauri windows/WebViews.
9. [Vite CLI](https://vite.dev/guide/cli) and
   [server options](https://vite.dev/config/server-options) — dev/preview server roles and local
   exposure cautions.
10. Soloist sources linked inline — the current Vite/Tauri config, native boundaries, typed IPC,
    and loopback HTTP security model.
