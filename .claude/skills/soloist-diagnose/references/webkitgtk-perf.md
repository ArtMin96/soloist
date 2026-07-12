# WebKitGTK / Tauri performance — gotchas & what's already fixed

Read this before diagnosing any slow, janky, laggy, or rendering issue in Soloist. It records the
WebKitGTK-specific traps this project already hit, so you apply the known fix and **never re-derive or
re-fix**. Full write-up + primary sources: `plan/performance-native-feel.md`.

WebKitGTK has far less rendering headroom than Chromium: waste that's invisible in a browser is a
visible stall here. Verify against the sources in `plan/performance-native-feel.md` §G, not memory.

## Platform gotchas (durable)

- **CSS class/theme toggles stagger.** Toggling a class on `<html>` repaints the *root layer* on the
  main thread (deferred), while composited layers (WebGL, `overflow` scrollers) flush on the
  compositor thread — so an un-promoted element recolors seconds late. Fix: promote it with
  `transform: translateZ(0)`; make the swap atomic (freeze transitions + one forced `getComputedStyle`).
- **Live WebGL contexts are capped at 16** (WebKit source `WebGLRenderingContextBase.cpp`); a 17th
  force-loses the oldest. Any pool of WebGL-backed terminals must stay bounded (Soloist caps at 6).
- **Events are eval'd JS + JSON per `emit`** — wrong for high-frequency streams. Batch/throttle on the
  Rust side or use a `Channel`; keep low-rate structural `DomainEvent`s on `emit`.
- **React Context has no selector** — a whole-object context value re-renders every consumer on every
  change, and `React.memo` does NOT stop it. Use `useSyncExternalStore` + a per-id selector.
- **`display:none` makes `FitAddon.fit()` unreliable** — on a hidden host it either no-ops (size is
  unmeasurable) or, if the host has an explicit size, clamps to the wrong (2×1) dimensions; either way
  refit on show. (Older xterm threw here — #3118/#3029, since fixed to a NaN-guarded no-op.) A hidden
  xterm auto-pauses its renderer, so it is cheap to keep mounted/alive.
- **Font weight renders ~100 heavier** than a browser (tauri#14286) — tune typography on WebKitGTK, not
  in a browser. `-webkit-font-smoothing` is a macOS-only no-op on Linux (crispness = Fontconfig/FreeType).

## Env vars — mostly DON'T ship

- Diagnostic only, never ship: `WEBKIT_DISABLE_COMPOSITING_MODE=1` (kills GPU compositing — use once to
  confirm a compositing-asymmetry lag, then remove).
- Ship ONLY if a real regression is confirmed (Linux-only, skip if the user already set it):
  `WEBKIT_DISABLE_DMABUF_RENDERER=1` / `__NV_DISABLE_EXPLICIT_SYNC=1` (blank window / Nvidia / Wayland
  Error 71). **None are set in Soloist today — rendering is clean; do not add speculatively.**

## Already fixed (2026-07) — do NOT re-diagnose

- **Theme/titlebar lag** → `Titlebar.tsx` (`translateZ(0)`), `lib/appearance.ts` (`applyDarkClass` atomic
  swap), `store/AppearanceProvider.tsx` (memoized value).
- **Slow terminal switching** → keep-alive pool `store/useTerminalPool.ts` + `App.tsx` +
  `TerminalPane.tsx` `visible`; backend multi-forwarder `crates/app/src/pty_bridge.rs`.
- **Metrics re-render storm** → `store/signalStore.ts` + `signalsContext.ts` selector; Rust
  emit-on-change `crates/core/src/metrics/sampler.rs`.

Deferred, only if a real bottleneck appears: a full `MetricsBatch` event; bundling a newer WebKitGTK
(≥ 2.42, which fixes the deferred-repaint bug) in the AppImage. Details in `plan/performance-native-feel.md`.
