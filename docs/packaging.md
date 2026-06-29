# Packaging & distribution (Ubuntu, x86_64)

Soloist ships two artifacts, both **x86_64 only** (decision D2):

| Artifact | Target | Install | Updates |
|----------|--------|---------|---------|
| `.deb` | Ubuntu **22.04+** (system WebKitGTK 4.1) | `sudo apt install ./soloist_*.deb` | `apt` / re-install |
| `.AppImage` | Ubuntu **22.04+** (portable, bundled WebKit) | run directly | in-app updater (opt-in) |

The `.deb` links the system `libwebkit2gtk-4.1` and therefore targets 22.04+, where that
library exists. The `.AppImage` bundles its own WebKit runtime so it needs no system
WebKit install — but its floor is still **22.04+**, not 20.04; see
[AppImage & the 22.04 floor](#appimage--the-2204-floor).

## Building

```bash
just deb        # .deb only (fastest; mirrors the per-PR CI gate)
just bundle     # .deb + .AppImage
```

Both run `cargo tauri build` from `crates/app`, which builds the React UI
(`beforeBuildCommand`), compiles the release binary (LTO, one codegen unit, stripped),
and packages the bundles into `target/release/bundle/{deb,appimage}/`.

> **glibc note.** A bundle must be built on the **oldest** system it targets — glibc is
> backward- but not forward-compatible. The distributable artifacts come from CI on
> `ubuntu-22.04`; a bundle built on a newer host (e.g. a dev machine on glibc 2.4x) will
> refuse to start on 22.04. Use `just bundle` locally only to inspect bundle *structure*;
> ship the CI artifacts.

## Desktop integration (J3)

The `.deb` installs:

- the binary at `/usr/bin/soloist`;
- a menu entry `/usr/share/applications/Soloist.desktop`
  (`Categories=Development;Utility;`, `StartupWMClass=soloist`) generated from
  [`crates/app/bundle/soloist.desktop`](../crates/app/bundle/soloist.desktop);
- hicolor icons under `/usr/share/icons/hicolor/*/apps/soloist.png` (our own mark, not
  Solo's art);
- a MIME type **`application/vnd.soloist.project+yaml`** matching the glob `solo.yml`
  (from [`crates/app/bundle/soloist-mimetype.xml`](../crates/app/bundle/soloist-mimetype.xml)),
  so a file manager offers **Open with Soloist** for a `solo.yml`.

A post-install script refreshes the desktop, MIME, and icon caches; a post-remove script
refreshes them again after uninstall. Opening a `solo.yml` (or passing any folder on the
command line) launches Soloist and **opens that project**: the path is resolved to the
project root and handed to the one core `load_project` command. A second launch is
forwarded to the running instance (single-instance) so it focuses rather than starting a
rival process.

## System tray & launch-on-login

A status-tray icon carries a menu: **Show Soloist**, **Start on login** (a checkbox,
off by default — opt-in), **Check for Updates…**, and **Quit Soloist** (which runs the
normal deterministic shutdown, reaping every managed process group). All tray actions are
window-shell or plugin calls; the tray holds no domain logic.

## Updates (J4 — disabled by default)

Soloist never checks for updates on its own. The tray's **Check for Updates…** is the
only trigger. It compares the running version against a static feed
(`latest.json` on the GitHub Releases of `ArtMin96/soloist`), and on a newer signed
release downloads, verifies (against the bundled public key), installs, and restarts.
On Linux the updater replaces the **AppImage**; `.deb` installs update through `apt`.

### Signing key (maintainers)

Update artifacts are signed in the release pipeline only. Generate a keypair once:

```bash
cargo tauri signer generate -w ~/.tauri/soloist-updater.key
```

- the **public** key is committed in `plugins.updater.pubkey` of
  [`tauri.conf.json`](../crates/app/tauri.conf.json);
- the **private** key + its password go into the repository secrets
  `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

`createUpdaterArtifacts` is enabled **only** in the release overlay
([`crates/app/bundle/release.conf.json`](../crates/app/bundle/release.conf.json)), so
ordinary keyless builds (local, per-PR CI) never demand the signing key.

## Releases & checksums (J5)

Tag a commit `vX.Y.Z` (matching the version in `tauri.conf.json`):

```bash
git tag v0.1.0 && git push origin v0.1.0
```

[`.github/workflows/release.yml`](../.github/workflows/release.yml) then, on `ubuntu-22.04`:

1. builds the signed `.deb` + `.AppImage`;
2. publishes them to a **draft** GitHub Release;
3. attaches `SHA256SUMS` (verify a download with `sha256sum -c SHA256SUMS`);
4. runs the clean-22.04 AppImage smoke (below).

## AppImage & the 22.04 floor

D2 originally set a 20.04 floor on the assumption that a self-contained `.AppImage` would
cover it. Phase-12 containerized smokes proved otherwise:

- The `.deb` installs and launches on a clean **Ubuntu 22.04** (all libraries resolve;
  desktop entry, icon, and `solo.yml` MIME association register; `apt remove` cleans up).
- The `.AppImage` runs on a clean **Ubuntu 22.04+** desktop with **no manual WebKit
  install** — WebKit is bundled.
- The `.AppImage` does **not** run on **Ubuntu 20.04**: Tauri v2 needs WebKitGTK 4.1
  (absent on 20.04), so the bundle is built on 22.04, and the libraries `linuxdeploy` pulls
  from that host need `GLIBC_2.33/2.34`, which 20.04's glibc 2.31 lacks. There is no clean
  20.04 build path for a Tauri-v2 app. Full detail: [`KNOWN-DIVERGENCES.md`](../KNOWN-DIVERGENCES.md) D-11.

**The supported floor for both artifacts is Ubuntu 22.04+, x86_64.** The release pipeline
verifies it: a clean Ubuntu 22.04 container with the desktop base present but **no WebKit
installed** runs the AppImage and asserts it stays up — proving WebKit is genuinely bundled.
