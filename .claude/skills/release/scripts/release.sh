#!/usr/bin/env bash
# Apply a release version and push its tag. The judgment about *which* version to cut lives in
# the skill; this script only carries it out — atomically and with every guard that keeps a
# half-finished release from ever reaching origin.
#
# It syncs the version across the three manifests that must agree (the workspace Cargo.toml, the
# UI package.json, and tauri.conf.json — the last is what the release tag and the in-app updater
# compare against), commits, tags `vX.Y.Z`, and pushes main plus the tag together. The tag push
# is what triggers .github/workflows/release.yml.
#
#   Usage: release.sh <major.minor.patch>
set -euo pipefail

new="${1:-}"
[[ -n "$new" ]] || { echo "usage: release.sh <major.minor.patch>" >&2; exit 2; }

# A plain X.Y.Z only. The git tag and the updater compare bare versions, so a pre-release or
# build suffix would quietly break the comparison.
[[ "$new" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "not a valid X.Y.Z version: '$new'" >&2; exit 1; }

root="$(git rev-parse --show-toplevel)"
cd "$root"

cargo_toml="Cargo.toml"
package_json="crates/app/ui/package.json"
tauri_conf="crates/app/tauri.conf.json"

# Preconditions — cutting from a dirty or diverged tree produces a release nobody can reproduce.
branch="$(git symbolic-ref --short HEAD)"
[[ "$branch" == "main" ]] || { echo "not on main (on '$branch')" >&2; exit 1; }
[[ -z "$(git status --porcelain)" ]] || { echo "working tree is dirty — commit or stash first" >&2; exit 1; }

git fetch --quiet origin main --tags
[[ "$(git rev-parse HEAD)" == "$(git rev-parse '@{u}')" ]] \
  || { echo "local main has diverged from origin/main — sync first" >&2; exit 1; }

tag="v$new"
if git rev-parse -q --verify "refs/tags/$tag" >/dev/null \
   || git ls-remote --exit-code --tags origin "$tag" >/dev/null 2>&1; then
  echo "tag $tag already exists — pick the next version" >&2
  exit 1
fi

# The version the three files currently hold, read from the manifest that owns the release number.
old="$(sed -n 's/.*"version": "\([0-9][0-9.]*\)".*/\1/p' "$tauri_conf" | head -1)"
[[ -n "$old" ]] || { echo "could not read the current version from $tauri_conf" >&2; exit 1; }
[[ "$old" != "$new" ]] || { echo "$new is already the current version" >&2; exit 1; }

# Replace the version in place, anchored so nothing else in the file can match by accident.
sed -i "s/^version = \"$old\"/version = \"$new\"/" "$cargo_toml"
sed -i "s/\"version\": \"$old\"/\"version\": \"$new\"/" "$package_json"
sed -i "s/\"version\": \"$old\"/\"version\": \"$new\"/" "$tauri_conf"

# Every manifest must now carry the new version — a file the substitution missed means drift, and
# drift is a bad release. Fail loudly rather than ship a mismatch.
for f in "$cargo_toml" "$package_json" "$tauri_conf"; do
  grep -q "\"$new\"\|\"version\": \"$new\"\|version = \"$new\"" "$f" \
    || { echo "$f still not at $new after the bump — aborting, no commit made" >&2; git checkout -- "$cargo_toml" "$package_json" "$tauri_conf"; exit 1; }
done

git add "$cargo_toml" "$package_json" "$tauri_conf"
git commit --quiet -m "chore(release): cut $tag"
git tag -a "$tag" -m "Soloist $tag"
git push --quiet origin main "$tag"

remote_url="$(git remote get-url origin)"
slug="${remote_url#*github.com[:/]}"; slug="${slug%.git}"
echo "Pushed $tag ($old -> $new)."
echo "Build:   https://github.com/$slug/actions/workflows/release.yml"
echo "Release: https://github.com/$slug/releases/tag/$tag"
