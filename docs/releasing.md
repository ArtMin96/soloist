# Cutting a release

A runbook for shipping a Soloist version as a `.deb` + `.AppImage` on GitHub. The build,
signing, publishing, and checksums are automated by
[`.github/workflows/release.yml`](../.github/workflows/release.yml); your job is to bump
the version, tag, and verify. See [`packaging.md`](packaging.md) for what the artifacts
contain.

## One-time setup (per repository)

The release pipeline signs the updater artifacts, so the repo needs the updater's **private**
key as a secret. The keypair is generated once with `cargo tauri signer generate` (the
**public** half already lives in `plugins.updater.pubkey` of
[`crates/app/tauri.conf.json`](../crates/app/tauri.conf.json) — do not change it without
re-issuing the key, or installed apps can no longer verify updates).

```bash
# private key content -> repo secret (the file written by `signer generate`)
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/soloist-updater.key
# the key's password (empty if the key was generated without one)
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --body ""
```

`GITHUB_TOKEN` is provided automatically; the workflow grants itself `contents: write`, so
no repository-wide Actions permission change is needed.

> **Private repo note.** The updater endpoint
> (`…/releases/latest/download/latest.json`) is only reachable by authenticated users while
> the repo is private. The in-app **Check for Updates…** works for the owner; it begins
> serving everyone once the repo (or its releases) is public. This does not affect the
> `.deb`/`.AppImage` downloads themselves.

## Releasing a version

1. **Bump the version** in two places, kept in step:
   - `version` in [`crates/app/tauri.conf.json`](../crates/app/tauri.conf.json)
   - `version` under `[workspace.package]` in [`Cargo.toml`](../Cargo.toml)
   Commit on a normal PR and merge to `main`.

2. **Tag the merged commit** `vX.Y.Z` — the tag's version **must** match
   `tauri.conf.json` (the workflow fills `__VERSION__` from it):

   ```bash
   git checkout main && git pull
   git tag v0.1.0
   git push origin v0.1.0
   ```

   (Or run the workflow by hand from the Actions tab via `workflow_dispatch`.)

3. **The pipeline runs** on `ubuntu-22.04` and:
   - builds the signed `.deb` + `.AppImage` (the release overlay turns on
     `createUpdaterArtifacts`);
   - creates a **draft** GitHub Release `Soloist vX.Y.Z` with the artifacts and
     `latest.json` (the updater feed);
   - attaches `SHA256SUMS`;
   - runs the AppImage on a clean Ubuntu 22.04 with no WebKit installed (the J2 gate).

4. **Verify the draft** (Releases → the draft):
   - the `.deb`, `.AppImage`, `latest.json`, and `SHA256SUMS` are attached;
   - `sha256sum -c SHA256SUMS` matches after downloading;
   - the smoke job is green.

5. **Publish** the draft when satisfied (Releases → Edit → Publish). Publishing is the only
   manual, irreversible step — everything before it is a reviewable draft.

## If the build fails

- **`A public key has been found, but no private key`** — the signing secret is missing or
  misnamed; re-check the one-time setup.
- **Tag/version mismatch** — the pushed tag's version differs from `tauri.conf.json`; delete
  the tag (`git push --delete origin vX.Y.Z`), fix the version, re-tag.
- **AppImage smoke red** — the AppImage did not run on a clean 22.04 with no WebKit (see
  [`packaging.md`](packaging.md#appimage--the-2204-floor)); treat as a release blocker and
  investigate the WebKit bundling rather than publishing.
