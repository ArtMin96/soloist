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
   **20.04** (webkit 4.0) without manual installs; verify on a minimal image.
5. **Tray + autostart (optional):** a status tray icon; opt-in "start on login".
6. **Update channel (J4, later):** Tauri updater pointed at a static release feed (e.g. GitHub
   Releases JSON); ship **disabled by default** with a manual "check for updates".
7. **Checksums/provenance (J5, later):** emit SHA-256 sums; document verification. (Signed apt repo is a
   future nice-to-have.)
8. **CI release pipeline:** tag → build `.deb` (22.04) + `.AppImage` (20.04-compatible) on x86_64 →
   attach to a GitHub Release with checksums.

## Acceptance criteria
- `sudo apt install ./soloist_*.deb` on a clean **Ubuntu 22.04 (x86_64)** installs with a working menu
  entry + icon; launches; `apt remove` cleans up.
- The `.AppImage` runs on a clean **Ubuntu 20.04 (x86_64)** with **no** manual webkit install.
- Published artifacts carry matching SHA-256 checksums produced by CI.

## Test plan
- **Automated:** containerized install/uninstall of the `.deb` (22.04) + headless launch (xvfb) of the
  `.AppImage` (20.04); assert startup + window class.
- **Manual:** install on a real Ubuntu desktop; check menu icon, tray, double-click `solo.yml`.

## Risks & mitigations
- **webkit 4.0 vs 4.1 split** → `.deb` targets 22.04; 20.04 users use the self-contained `.AppImage`;
  document clearly (a second 20.04 `.deb` only if demanded).
- **AppImage missing libs on minimal systems** → use Tauri/linuxdeploy bundling; test on a minimal base
  image, not a dev box.

## Effort
~3–5 days (x86_64 `.deb` + `.AppImage`); +1–2 days for the update channel.
