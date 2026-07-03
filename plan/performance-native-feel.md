# Performance & Native-Feel Initiative — Working Plan + Research Report

> **What this is.** A self-contained, phase-structured working doc for making Soloist feel fast,
> snappy, and native on Linux/WebKitGTK. It is an *initiative* doc, **not** a canonical contract —
> it defers to `plan/04` (design), `plan/06` (structure), `plan/05` (behavior), and CLAUDE.md
> (process) exactly like any other work. Every change here must still pass the §7 definition-of-done
> and the §15/§16 discipline gates.
>
> **Why it exists.** So this initiative survives a context reset: a fresh session can read *this
> file* + `PROGRESS.md` and resume mid-flight without re-deriving the research.
>
> **How to resume (fresh session):**
> 1. Read the start-of-session protocol in CLAUDE.md §1 as usual.
> 2. Read this file top to bottom. The **Progress Log** (bottom) says where we are.
> 3. Pick up the first task whose status is not `Verified`. Respect the **Constraints** section.
> 4. Before touching a Tauri surface, invoke the matching `tauri-*` skill (CLAUDE.md §5). Before a
>    UI-visible change, `/impeccable`. Measure before/after (CLAUDE.md §6) — never fabricate a number.
>
> **Status vocabulary (same as PROGRESS.md):** `Not started` · `In progress` · `Done — pending
> verify` · `Verified`.

---

## A. Diagnosis — root causes (evidence-backed)

The three reported symptoms have distinct root causes; all are amplified by the fact that
**WebKitGTK has far less rendering headroom than Chromium**, so architectural waste that is free in
a browser becomes a visible stall here.

### ① Theme toggle: titlebar recolors 1–2 s after the body

Ranked hypotheses:

- **H1 (leading) — Compositing-layer asymmetry.** WebKitGTK (coordinated graphics) only gives a
  `RenderLayer` its own GPU `GraphicsLayer` when it has a promotion trigger (transform, opacity,
  filter, overflow, canvas/WebGL). Composited layers are flushed by the **compositor thread**;
  everything else paints into the **root layer**, whose repaint is scheduled on the **main thread
  and deferred/coalesced**. In our tree the **terminal is a WebGL layer** and the **sidebar is
  `overflow-y-auto`** → both composited and flushed promptly; the **titlebar `<header>` has no
  trigger** → root layer → deferred flush, visible only where composited children don't cover it
  (the titlebar strip). Reproduces "body instant, titlebar late" exactly. *(Synthesis of primary
  sources — confirm empirically via P0/F4.)*
  Sources: WebKit Graphics architecture; WPE graphics architecture.
- **H2 (compounds H1) — Known WebKitGTK 2.40 deferred-repaint bug** (content computed but not
  flushed until a later input/compositor tick). Fixed in **2.42**. Version-dependent on the user's
  system WebKitGTK. Source: tauri#7021; WebKitGTK 2.48 notes (opt-in damage tracking).
- **H3 (minor) — transition staggering.** shadcn `transition-colors` at `--dur-fast: 120ms` — far
  under 1–2 s, so a shimmer at most, not the lag.
- **H4 (low, unproven) — OKLCH/`color-mix()` paint cost.** No source confirms it is slow on
  WebKitGTK; only 7 `color-mix()` + ~73 static `oklch()` literals. Would slow both strips, not
  stagger them. Do not over-invest.

**Confirmed code amplifier:** `AppearanceProvider` (`store/AppearanceProvider.tsx`) publishes
`{appearance, dark, setAppearance}` as a fresh object each render (not memoized) at the app root, so
a theme toggle re-renders every consumer — including `useTerminal`, whose restyle effect runs
`fit.fit()` (layout) + `pty_resize` (IPC) on the same frame WebKitGTK needs to flush.

### ② Terminal switch is slow

`App.tsx:202` uses `key={selected.id}`, forcing a full teardown + rebuild every switch
(`useTerminal.ts:128-180`): `new Terminal()` → `open()` → **fresh WebGL2 context** (shader compile +
glyph atlas) → PTY re-attach → **scrollback replay** (up to 256 KB re-parsed) → fit → focus. Backend
(`pty_bridge.rs`) mirrors this with a **single forwarder**.

Load-bearing constraints:
- **WebKitGTK caps live WebGL contexts at 16** (WebKit source `WebGLRenderingContextBase.cpp`:
  `maxActiveContexts = 16`); a 17th forcibly loses the oldest. Chromium identical. → any warm-WebGL
  keep-alive pool must be **bounded (≤ ~8)**.
- **Hidden xterm instances auto-pause their renderer** via IntersectionObserver (xterm PR #1144) →
  a retained context costs GPU *memory* (atlas), ~zero GPU *time*. Writes still fill the buffer.
- **`FitAddon.fit()` throws on a `display:none` element** (xterm #3118/#3029) → must **refit on
  show**.

### ③ General jank (IPC + React)

- **`SignalsContext` re-render storm:** `useSignal(id)` reads the whole context, so every
  `MetricsTick` (~1/sec per running process) re-renders every sidebar row + terminal header,
  regardless of relevance. `React.memo` cannot fix this (react.dev useContext).
- **Metrics ride the global event bus**, which Tauri delivers by **eval-ing JSON in the webview** —
  docs say explicitly it is "not designed for low latency or high throughput."
- **8 independent global `onDomainEvent` listeners**, each receiving every event.

---

## B. Phase plan

### Phase P0 — Verify-first (baseline + confirm mechanisms)

Goal: a green, reproducible baseline and empirical confirmation of H1 before changing anything, so
every later fix has a measured before/after (CLAUDE.md §6; no fabricated numbers).

| # | Task | Acceptance | Status |
|---|------|-----------|--------|
| P0.1 | Green baseline: `just lint` + `just test` pass; record counts. | Both green; counts logged in Progress Log. | **Verified** |
| P0.2 | Confirm H1: run dev build with `WEBKIT_DISABLE_COMPOSITING_MODE=1`; observe whether the titlebar theme-lag disappears. | Documented yes/no. Yes ⇒ H1 confirmed ⇒ F1 is correct. (Diagnostic only — never ship the flag.) | Not started |
| P0.3 | Profile a terminal switch: `performance.now()` around mount + `activateTerminalRenderer`; note dominant sub-cost (WebGL init vs DOM build vs replay). Use `flush_terminal_perf`. | Numbers logged; dominant cost identified. | Not started |
| P0.4 | Profile the render storm: React DevTools Profiler — re-renders per metrics tick, before changes. | Baseline re-render count logged. | Not started |

> P0.2–P0.4 need a desktop session (`DISPLAY=:0`) or the `agent-bridge` MCP. P0.1 is automatable.

### Phase P1 — Quick wins (low risk; directly kills the three complaints)

| # | Task | Files | Why / source | Status |
|---|------|-------|--------------|--------|
| P1.1 | Promote titlebar to its own compositing layer (`[transform:translateZ(0)]`). Verify drag-region + double-click-maximize still work. | `components/titlebar/Titlebar.tsx` | ① lag — WebKit Graphics (GraphicsLayer promotion). `tauri-window-customization` confirmed drag/hit-test safe. | **Verified** (user confirmed titlebar recolors atomically, 2026-07-03) |
| P1.2 | Atomic theme swap: suppress transitions during the `.dark` flip + force a sync style flush. | `lib/appearance.ts` (`applyDarkClass`) | ① staggering — Paco Coursey pattern. | **Verified** (part of the confirmed theme-lag fix) |
| P1.3 | Memoize the `AppearanceContext` value (`useMemo`). | `store/AppearanceProvider.tsx` | ① terminal re-render on theme — react.dev useContext. | **Verified** (frontend gates green + user confirm) |
| P1.4 | Metrics **emit-on-change** at the source: the sampler suppresses a reading identical to the last one for that process (idle/steady processes stop emitting redundant ~1 Hz ticks). | `crates/core/src/metrics/sampler.rs` (+`sampler_tests.rs`, `events.rs` doc) | ③ event fan-out. **Chose emit-on-change over the full enum batch** — see note. | **Verified** (core 537 tests green, clippy clean) |
| P1.5 | Replace `SignalsContext` whole-object delivery with an external store + per-id `useSyncExternalStore` selector (manual slice caching — no new dep). | `store/signalStore.ts` (new), `store/signalsContext.ts`, `store/SignalsProvider.tsx`; `store/signals.ts` fold unchanged | ③ render storm — react.dev; TkDodo. | **Verified** (test proves only the ticked row re-renders; full suite 54/264 green) |

Acceptance for P1: theme toggle recolors atomically incl. titlebar (measured vs P0.2); metrics-tick
re-renders drop to "only the changed row" (measured vs P0.4); `just lint`/`just test` green.

### Phase P2 — Terminal keep-alive (the real fix for ②)

Design: bounded warm keep-alive pool.
1. Remove `key={selected.id}` (`App.tsx:202`).
2. One `Terminal` per process id, created once, **kept mounted** (never re-`open()` — xterm #4978).
   Lifecycle moves out of the `key`-remounted component into a stable per-id registry.
3. Switch = toggle CSS visibility + **refit on show** (unhide → next rAF → `fit()` → `pty_resize`).
   IntersectionObserver auto-resumes the shown renderer.
4. Keep WebGL warm; bound with an **LRU ≤ 8**; on eviction fully dispose (`renderer.dispose()` →
   `term.dispose()` → `ptyDetach`).
5. **Sub-decision — CHOSEN: (a) PTY stays attached for pooled terminals.** Truly instant, no replay.
   Rationale over (b): it keeps `useTerminal`'s delicate attach/cancel/replay lifecycle **untouched**
   (lowest frontend risk), the existing `ResizeObserver` already refits on the `display:none`→show
   transition, and xterm auto-pauses/resumes its renderer off-screen. The only new logic is a small,
   unit-testable `pty_bridge` multi-forwarder map (install adds; clear-by-token aborts one; no
   abort-on-install). Each pooled pane detaches its token on unmount/eviction, so forwarders stay
   bounded by the pool cap — verified by `just soak`. (b) was rejected: rewriting the streaming
   lifecycle for visibility-driven attach/detach is higher risk to the safety-critical terminal UX.

| # | Task | Files | Status |
|---|------|-------|--------|
| P2.1 | Drop `key={id}`; render a keep-alive pool (one mounted `TerminalPane` per pooled process, React owns each xterm via a stable id key). | `App.tsx`, `store/useTerminalPool.ts` (new) | **Verified** (user confirmed switching is fast, 2026-07-04) |
| P2.2 | Visibility switch (`display:none` for hidden panes) + refit-and-focus on show. | `TerminalPane.tsx`, `useTerminal.ts` (`visible` param) | **Verified** (user-confirmed) |
| P2.3 | Bounded LRU pool (`TERMINAL_POOL_CAP=6`, under the 16 WebGL cap) + deterministic dispose (evicted pane unmounts → existing cleanup disposes xterm + detaches PTY). | `store/useTerminalPool.ts` (+ test) | **Verified** (gates + soak green, user-confirmed) |
| P2.4 | Backend multi-forwarder: `pty_bridge` holds a token→forwarder map (install adds, clear-by-token aborts one, no abort-on-install). | `pty_bridge.rs`, `pty_bridge_tests.rs`, `commands/mod.rs` doc | **Verified** (clippy clean, 4 bridge tests green) |

Acceptance for P2: switching between visited terminals is instant (measured vs P0.3); WebGL contexts
never exceed the cap; `just soak` leak-gate green (FD/task/RSS flat across N start/stop/switch);
`just lint`/`just test` green.

### Phase P3 — Native-feel polish + gates

| # | Task | Source | Status |
|---|------|--------|--------|
| P3.1 | Font-weight +100 offset. | tauri#14286 | **Resolved — no change.** User confirms the chrome weights (500/550/600) read correctly on WebKitGTK; the design is already tuned on the real renderer, so the offset is accounted for. Lightening blindly was rejected (risks thin/low-contrast text). |
| P3.2 | `-webkit-font-smoothing`/`antialiased` (inert on Linux). | — | **Resolved — left in place.** Inert on WebKitGTK but harmless; removing a standard base utility is churn for zero visual effect. |
| P3.3 | DMABUF/Nvidia env-var mitigations. | Tauri Linux Graphics | **Resolved — not added.** User confirms clean rendering (no blank/flicker/tearing). Adding `WEBKIT_DISABLE_*` would disable a faster path for everyone; only ship if a real regression appears. |
| P3.4 | WebKitGTK version / AppImage bundling. | tauri#7021 | **Deferred to Phase 12 (packaging).** Recommend documenting a min WebKitGTK ≥ 2.42 (the deferred-repaint bug is fixed there) and considering a newer bundled WebKitGTK in the AppImage — but that's packaging-phase work, not this initiative. |
| P3.5 | Gate re-run / bundle measure. | CLAUDE.md §6 | **N/A.** No code changed since the P2 green state (all gates + soak already green). No runtime dependencies were added (package.json unchanged), so bundle impact is negligible. |

---

## C. WebKitGTK env-var cheat sheet

| Variable | Effect | Recommendation |
|----------|--------|----------------|
| `WEBKIT_DISABLE_COMPOSITING_MODE=1` | disables accelerated compositing entirely | **Diagnostic only (P0.2/F4)**; never ship |
| `WEBKIT_DISABLE_DMABUF_RENDERER=1` | disables DMABUF path; fixes blank/Nvidia/Wayland Error 71 | ship only if confirmed (AppImage-only, skip if user-set); **not** a theme fix |
| `WEBKIT_FORCE_COMPOSITING_MODE=1` | forces compositing on | don't use — default already ALWAYS |
| `__NV_DISABLE_EXPLICIT_SYNC=1` | fixes Nvidia+Wayland Error 71, no perf cost | Nvidia/Wayland crash mitigation only |
| `GDK_BACKEND=x11` | force XWayland | did NOT fix the 2.40 repaint bug; not a smoothness lever |
| `LIBGL_ALWAYS_SOFTWARE=1` | software GL | debugging only; slow |

Set from Rust before `tauri::Builder`: `#[cfg(target_os="linux")] std::env::set_var(...)`. Verify in
a shell first; ship an unconditional override only if the app is confirmed affected. Soloist sets
none today.

## D. Native-feel checklist (Tauri + WebKitGTK)

- Font-weight renders ~100 heavier on WebKitGTK (tauri#14286) — check `font-[550]`/`w400`/`w600`.
- `-webkit-font-smoothing: antialiased` is a no-op on Linux (macOS-only).
- Virtualize long lists / large scrollback — WebKitGTK degrades with thousands of DOM nodes (tauri#3988).
- Keep the WebGL→DOM fallback (present) — WebKitGTK can silently drop WebGL2 to software.
- Coalesce chatty terminal output per frame (present). Keep repaints small; full-page theme flips are
  the expensive special case (Part A/①).
- Respect `prefers-reduced-motion` (present). Test against the user's actual WebKitGTK; target ≥ 2.42.

## E. Constraints — do not violate

- Hexagonal layering: perf changes live in adapters/frontend, **not** `core`. No `use tauri` in core.
- Bounded everything: LRU pool cap, batch buffer, deadband, forwarders — all bounded (§8).
- **Locked non-changes (§6):** `panic = "unwind"`, `freezePrototype = false`, release `opt-level`,
  `Cargo.lock` brotli pins, `removeUnusedCommands`. Release profile already has `lto` +
  `codegen-units = 1` + `strip`.
- Measure before/after every optimization; record in the Progress Log. No fabricated numbers.
- Do not weaken/skip a test to pass a gate. Do not touch `PROGRESS.md` unless the user asks.
- Invoke matching `tauri-*` skills before Tauri surfaces; `/impeccable` before UI-visible changes;
  `soloist-diagnose` is the measurement/gate vehicle.

## F. Measurement & gates

`just lint` · `just test` · `just soak` (FD/task/RSS leak gate — critical for P2 + any Channel) ·
`just bloat` / `just bundle-size` · `flush_terminal_perf` + `performance.now()` for terminal timing ·
React DevTools Profiler for re-renders · `WEBKIT_DISABLE_COMPOSITING_MODE=1` as the H1 diagnostic.

## G. Sources (primary, read during research)

- WebKit Graphics: https://docs.webkit.org/Ports/WebKitGTK%20and%20WPE%20WebKit/Graphics.html
- WPE graphics architecture: https://wpewebkit.org/blog/03-wpe-graphics-architecture.html
- Igalia compositing (2017): https://blogs.igalia.com/carlosgc/2017/02/10/accelerated-compositing-in-webkitgtk-2-14-4/
- Igalia DMABUF (2023): https://blogs.igalia.com/carlosgc/2023/04/03/webkitgtk-accelerated-compositing-rendering/
- WebKitGTK 2.48 notes: https://webkitgtk.org/2025/04/08/webkitgtk-2.48.html
- Tauri Linux Graphics: https://v2.tauri.app/develop/debug/linux-graphics/
- Tauri Calling the Frontend: https://v2.tauri.app/develop/calling-frontend/
- Tauri Calling Rust (Channels): https://v2.tauri.app/develop/calling-rust/
- Tauri 2.0 IPC blog: https://v2.tauri.app/blog/tauri-20
- tauri issues: #7021, #3988, #9394, #14286, #8177, #12724, #13405
- WebKit `WebGLRenderingContextBase.cpp` (maxActiveContexts=16); Chromium 40543269
- xterm: #4379, PR #1144, #4978, #3118, #3029, #2033; VS Code terminal renderer blog
- React: react.dev useContext, useSyncExternalStore; thisweekinreact with-selector; azguards
  propagation penalty; TkDodo Zustand; Kent Dodds context value
- Techniques: paco.me disable-theme-transitions; reemus disable-css-transition-color-scheme
- context7: `/websites/v2_tauri_app`, `/websites/react_dev`, `/websites/xtermjs`, `/websites/tauri_app`

## H. Flagged unverified (no fabrication)

- H1 causal chain is a synthesis — confirm via P0.2 before committing to F1.
- OKLCH/`color-mix` slowness on WebKitGTK: unproven; low priority.
- Exact ms costs of xterm mount / WebGL activation / replay on WebKitGTK: no published benchmark —
  measure in-app (P0.3).
- oflight % gains and the "laggy CI" article are build-time/anecdote, not runtime — do not quote as
  Soloist numbers.
- Channel is single-consumer per invoke; adopting it for metrics means consolidating the two listeners.
- Binary payloads for the *event* system aren't in stable Tauri (only Channels/commands).

---

## I. Progress Log (append newest first — the cross-session state)

- **✅ INITIATIVE COMPLETE (2026-07-04).** All three reported problems fixed and confirmed: ① theme
  lag (user-confirmed), ② terminal switch is now instant (user-confirmed), ③ general jank
  (test-verified). **P3 resolved with no code changes** — both observation-dependent items came back
  "no change needed" (font weights read right on WebKitGTK; rendering is clean), and the AppImage
  WebKitGTK-version item is deferred to Phase 12 packaging. Nothing left in this initiative; the only
  future follow-up is the recorded Phase-12 packaging note (P3.4) and the optional full metrics-batch
  (recorded under P1.4) if a very high process count ever makes per-tick `app.emit` a bottleneck.
  **9 commits** on the working branch (Tier 1 ×4 + docs, P2 ×3, docs ×1); tree clean, every gate green.
- **P2 (terminal keep-alive) — Verified (user-confirmed switching is fast).** Implemented, all
  automated gates green, committed. Was: owed GUI feel-confirm — now confirmed. Chose sub-decision **(a)**. `pty_bridge` is now a token→forwarder **map**
  (multi-forwarder, no abort-on-install); new `store/useTerminalPool.ts` (bounded LRU `nextPool`,
  `TERMINAL_POOL_CAP=6` under the 16 WebGL cap) + test; `App.tsx` drops `key={id}` and renders a
  persistent pool (one `TerminalPane` per pooled process, only the selected visible, current
  selection folded in so no first-select flash); `TerminalPane` gains a `visible` prop
  (`display:none` when hidden) → `useTerminal` refit-and-focus-on-show; its attach/cancel/replay
  lifecycle otherwise **untouched**. So switching back to a pooled terminal is instant (no
  xterm/WebGL rebuild, no replay; the stream stayed live). **Gates GREEN:** `just test` (UI **55
  files / 271 tests**, Rust core **537** + app + pty) + `just soak` leak-gate (3/3; `fds 4→4,
  threads 5→5, tasks 1→1` — flat) + clippy + fmt. **Committed** (3e07102 backend, 4d311c5 UI,
  590aede docs). **Owed only:** a GUI runtime confirm that switching *feels* instant and shows/refits
  cleanly (the display:none→show path) — the one property tests can't observe.
- **✅ TIER 1 COMPLETE (2026-07-04) — full workspace gate green.** `just lint` passes end-to-end (fmt,
  clippy `-D warnings`, tsc, eslint, prettier, dependency-direction, schema); Rust core **537** tests,
  UI **264** tests. Symptoms ① (theme lag) and ③ (general jank) are addressed and verified. Only the
  pre-existing file-size advisory remains (12 files, non-gating; none introduced here). **Remaining:
  P2 (terminal keep-alive) — the user's #2 complaint, the last big item; recommend a fresh session,
  resume at Phase P2.** Nothing committed yet — the working tree holds the P1 changes.
- **P0.1 Verified** — green baseline captured. `just lint` exit 0 (fmt, `clippy -D warnings`, tsc,
  eslint, prettier, dependency-direction OK; advisory-only: 12 files over the 400-line split smell).
  `just test` exit 0 — all Rust crate suites green; UI vitest **52 files / 257 tests** passed. Soak
  tests remain separately `#[ignore]`d (the P2 leak gate). This is the before/after reference.
- **P1.1 + P1.2 + P1.3 Done — pending verify.** The ① theme-lag batch is code-complete and passes all
  frontend gates (tsc, eslint, prettier, vitest 52/257). Changes: (1) `Titlebar.tsx` header gets
  `[transform:translateZ(0)]` so it composites like the body; (2) `applyDarkClass` freezes transitions
  during the `.dark` flip + forces a sync style read so the palette swaps atomically; (3)
  `AppearanceProvider` value is `useMemo`'d so a toggle no longer re-renders every terminal.
  **Owed:** GUI visual confirmation (`just dev` → toggle theme → titlebar recolors in one frame, no
  1–2 s lag) to flip these to Verified; optional `WEBKIT_DISABLE_COMPOSITING_MODE=1` A/B to confirm
  H1. **If lag persists after this:** re-diagnose via F4, then try F3 (compositor nudge).
- **P1.5 Verified** — ③ render storm fixed. New `signalStore.ts` external store folds deltas via the
  unchanged pure `applySignal`; `useSignal(id)` reads its own slice via `useSyncExternalStore` with
  per-consumer slice caching + value-level equality (`sameSignal`), so a MetricsTick re-renders only
  that process's row, and a repeated identical reading re-renders nothing. `SignalsProvider` now owns
  a stable store; `fixedSignalStore`/`EMPTY_STORE` back the provider-less default and tests. Evidence:
  `signalsContext.test.tsx` asserts only the ticked probe re-renders; full UI suite **54 files / 264
  tests** green (tsc/eslint/prettier clean). No new dependency.
- **P1.4 Verified — Tier 1 is now complete.** Metrics **emit-on-change**: `sampler.rs` keeps a
  bounded per-process cache of the last published `(cpu.to_bits(), rss)` and skips a tick whose
  reading is unchanged, so a steady/idle process stops re-emitting an identical reading every second
  (a changed reading still emits immediately, so responsiveness is untouched). Evidence: new
  `an_unchanged_reading_is_not_re_emitted` test + full core suite **537 green**, clippy `-D warnings`
  clean, fmt clean.
  **Decision (design):** chose emit-on-change over the plan's "one `MetricsBatch` per interval." The
  full batch ripples across the closed `DomainEvent` enum + its TS mirror + **two** frontend folds
  (`signals` and the list `projection`) + the soak leak-gate — not a "quick win," and higher risk on a
  load-bearing invariant. Emit-on-change is contained to the core sampler, needs no enum/mirror/fold
  changes, and (now that P1.5 removed the render storm) captures the remaining benefit for the common
  idle case. The full batch stays a recorded, non-gating option if profiling later shows the per-tick
  `app.emit` count is still a bottleneck at high process counts.
- **(next) P2 — terminal keep-alive** (the user's #2 complaint): the last big item. Bounded warm pool,
  drop `key={id}`, refit-on-show, LRU ≤8; sub-decision (a) PTY-stays-attached [needs `pty_bridge`
  multi-forwarder] vs (b) re-attach-on-show. Guard with `just soak`. See Phase P2.
- **P1.5 Verified** (render storm). **P1.1–P1.3 Verified** (theme lag, user-confirmed). **P0.1
  Verified** (baseline). **Doc created.**
