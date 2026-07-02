---
name: release
description: Cut and ship a Soloist release. Reads what merged since the last release, decides the semantic-version bump, syncs the version across the three manifests, tags, and pushes so CI builds and publishes the GitHub Release the in-app updater reads from. Use when the user wants to release, ship, publish, or bump the app version.
argument-hint: "[patch|minor|major|X.Y.Z]"
disable-model-invocation: true
allowed-tools: Bash(git *), Bash(gh *), Bash(bash *), Read
---

# Cut a release

You are cutting a real release. Pushing the tag makes CI build the signed `.deb` and `.AppImage`
and publish a GitHub Release that every installed copy of Soloist will auto-update to. So the
version has to be right and the tree has to be clean. Decide the version from the evidence in front
of you, and don't guess.

`${CLAUDE_SKILL_DIR}/scripts/release.sh` does the mechanical part: it writes the version into the
three manifests, commits, tags, and pushes, and it refuses to run on a dirty or diverged tree. What
it can't do is choose the number. That part is yours, and it's the whole reason this isn't a
one-line script.

## 1. Read the state

Run these and read the output before deciding anything:

```bash
git fetch --quiet origin main --tags
cur=$(sed -n 's/.*"version": "\([0-9][0-9.]*\)".*/\1/p' crates/app/tauri.conf.json | head -1)
echo "current version: $cur"
git log --oneline "v$cur"..HEAD
git diff --stat "v$cur"..HEAD | tail -1
```

Also pull the merged PRs since the last release for their titles and labels:

```bash
since=$(git log -1 --format=%cs "v$cur")
gh pr list --state merged --base main --search "merged:>=$since" \
  --json number,title,labels --jq '.[] | "#\(.number) \(.title) [\(.labels|map(.name)|join(","))]"'
```

If nothing has merged since `v$cur`, stop and say so. There is nothing to release.

## 2. Decide the version

If the user passed an argument, honor it:

- `patch`, `minor`, or `major`: bump that part of `$cur`.
- A literal `X.Y.Z`: use it as-is, as long as it is higher than `$cur`.

`$ARGUMENTS`

With no argument, decide from what you just read. Soloist is pre-1.0, and pre-1.0 semver works a
little differently: the leading `0.` stays put, so even a breaking change bumps the minor rather
than the major. The commits follow Conventional Commits, so read them as your main signal:

- **Minor** (`0.2.0` → `0.3.0`) when the range adds a feature or changes behavior in a
  backward-incompatible way: any `feat(...)`, or a `!` / `BREAKING CHANGE`.
- **Patch** (`0.2.0` → `0.2.1`) when the range is only fixes, refactors, docs, tests, deps, or
  tooling: `fix`, `perf`, `refactor`, `docs`, `test`, `build`, `chore`, `ci`.
- Take the highest bump any single change in the range calls for. One `feat` among ten `fix`es is
  still a minor.
- **Major** (`→ 1.0.0`) is a deliberate call to leave 0.x. Never reach for it on your own. Cut it
  only when the user explicitly asks to go 1.0.

Diff size only breaks ties. A one-line `feat` is still a minor, and a thousand-line `refactor` is
still a patch. When the commits genuinely don't settle it, pick patch and say why.

Before you act, state the version you chose and the change or two that drove it.

## 3. Cut it

```bash
bash "${CLAUDE_SKILL_DIR}/scripts/release.sh" <version>
```

The script validates the version, checks that you are on a clean and current `main`, confirms the
tag is free, writes the version into `Cargo.toml`, `crates/app/ui/package.json`, and
`crates/app/tauri.conf.json`, then commits, tags `vX.Y.Z`, and pushes. If it exits non-zero, read
the message and fix what it flags. Don't work around it.

## 4. Confirm

The script prints the build and release URLs. Report them, and watch the run if you want to see it
through:

```bash
gh run watch --exit-status "$(gh run list --workflow=release.yml --limit 1 --json databaseId --jq '.[0].databaseId')"
```

Once the run is green, the release is public and the in-app updater will offer it. If the run
fails, remember that the tag is already pushed, so fix forward with the next patch instead of
deleting a published tag.
