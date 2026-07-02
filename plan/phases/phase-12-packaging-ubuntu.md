# Phase 12 — Packaging & Distribution (Ubuntu, x86_64)

**Goal:** Make Soloist installable the normal Ubuntu ways: a `.deb` (apt) and a portable `.AppImage`,
**x86_64 only** (D2), with desktop integration, icons, and a path to updates.

**Delivers:** J1–J5, supports K1. **Architecture:** Tauri bundler config + a CI release pipeline.

## Scope
**In:** Tauri bundler for `deb` + `appimage`; desktop entry + icons + MIME; precise webkit dependency
handling; checksums; install/uninstall verification on clean machines; an optional update channel.
**Out:** arm64, macOS, Windows (D2); Flatpak/Snap (optional future); a hosted update server (we ship the
feed format + checker).

## Tasks
1. **Bundler config:** `tauri.conf.json` `bundle.targets = ["deb","appimage"]`; product name, id
   `dev.soloist.app`, version, categories `Development;Utility;`, and an **own** icon set (PNG/SVG — not
   Solo's art).
2. **Desktop integration (J3):** `.desktop` entry (name/exec/icon/categories/`StartupWMClass`); hicolor
   icon paths; optional MIME association so opening a `solo.yml` launches Soloist.
3. **Dependencies (.deb):** declare runtime deps for the target — `libwebkit2gtk-4.1-0` (22.04),
   `libgtk-3-0`, `libayatana-appindicator3-1` (tray). Primary `.deb` targets **22.04**.
4. **AppImage self-containment (J2):** bundle the WebKit runtime so the `.AppImage` runs on a clean
   **22.04+** without a manual webkit install; verify on a minimal image. (D2's original 20.04 target
   proved infeasible — Tauri v2 needs WebKitGTK 4.1, absent on 20.04; see `KNOWN-DIVERGENCES` D-11.)
5. **Tray + autostart (optional):** a status tray icon; opt-in "start on login".
6. **Update channel (J4, later):** Tauri updater pointed at a static release feed (e.g. GitHub
   Releases JSON); ship **disabled by default** with a manual "check for updates".
7. **Checksums/provenance (J5, later):** emit SHA-256 sums; document verification. (Signed apt repo is a
   future nice-to-have.)
8. **CI release pipeline:** tag → build `.deb` + `.AppImage` (both 22.04+) on x86_64 →
   attach to a GitHub Release with checksums.
9. **Companion binaries (added 2026-07-03 — omitted from the original plan and caught by a
   user bug report):** both artifacts must ship `soloist-mcp` (F1 "bundled helper") and
   `soloist-cli` (H4) at `usr/bin` beside the app binary — `bundle.linux.{deb,appimage}.files`
   maps them from `target/release/`, and `beforeBuildCommand` release-builds both crates so
   every `cargo tauri build` produces complete packages. The app exports the helper to
   `<data dir>/bin` on startup so generated MCP snippets carry a path that survives an
   AppImage's per-launch mount (see `docs/mcp-setup.md`).

## Acceptance criteria
- `sudo apt install ./soloist_*.deb` on a clean **Ubuntu 22.04 (x86_64)** installs with a working menu
  entry + icon; launches; `apt remove` cleans up.
- Both artifacts contain `usr/bin/soloist-mcp` and `usr/bin/soloist-cli`, and a generated
  Claude Code snippet from a packaged install launches the helper successfully.
- The `.AppImage` runs on a clean **Ubuntu 22.04+ (x86_64)** with **no** manual webkit install (20.04
  infeasible — `KNOWN-DIVERGENCES` D-11).
- Published artifacts carry matching SHA-256 checksums produced by CI.

## Test plan
- **Automated:** containerized install/uninstall of the `.deb` (22.04) + headless launch (xvfb) of the
  `.AppImage` (22.04, no webkit installed); assert startup + window class.
- **Manual:** install on a real Ubuntu desktop; check menu icon, tray, double-click `solo.yml`.

## Risks & mitigations
- **webkit 4.0 vs 4.1 split** → both artifacts target **22.04+** (Tauri v2 requires WebKitGTK 4.1, absent
  on 20.04, so neither artifact can support 20.04 — `KNOWN-DIVERGENCES` D-11).
- **AppImage missing libs on minimal systems** → use Tauri/linuxdeploy bundling; test on a minimal base
  image, not a dev box.

## Effort
~3–5 days (x86_64 `.deb` + `.AppImage`); +1–2 days for the update channel.
