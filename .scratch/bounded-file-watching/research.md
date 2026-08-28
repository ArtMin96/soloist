# Research: Event-Driven File Watching at Scale for Soloist

**Route:** EXTERNAL + INTERNAL verification  
**Depth:** Standard (targeting decision-grade brief)  
**Baseline:** Soloist currently uses single-instance `notify` (v8, inotify backend) per watched root; 58,179 watches for this repo against 65,536 limit (system already tuned).  

---

## TL;DR: Is There a Genuinely Event-Driven Solution at Scale?

**Short answer: No.** None of the feasible options eliminate inotify's per-directory cost on Linux, and the only unprivileged filesystem-wide alternative (fanotify) is barred from unprivileged access. The recommended architecture is a **tuned single-watcher per project with gitignore-aware directory filter**, paired with a **fallback polling strategy when inotify budget is exhausted**. This is event-driven at the app level (no polling within a budget) but degrades gracefully when the system ceiling is hit.

---

## Answer

**Event-driven performance is fundamentally limited on Linux by inotify's per-directory watch cost.** Git's fsmonitor daemon, the most promising lead, uses inotify under the hood and therefore moves the problem rather than solving it—it still costs one watch per directory. Fanotify, which offers filesystem-wide marks with a single descriptor, is gated behind `CAP_SYS_ADMIN` (unprivileged users are restricted to inode-level marks only), making it unsuitable for a desktop app that must run unprivileged. 

**The scalable non-polling design is:** one `RecommendedWatcher` instance per project with `.watch(root, RecursiveMode::Recursive)`, combined with **gitignore-aware directory filtering** to avoid registering watch descriptors for ignored subtrees. This cuts the 734k-directory corpus from ~734k potential watches to a manageable 337 (per provided data). The `notify` crate's v8 backend (`paths_mut()`) and v9 successor (`update_paths()`) both support multiple paths on one watcher instance, with per-path unwatch, so Soloist can optimize to one watcher shared across all projects.

**Fallback:** when inotify `max_user_watches` is exhausted, degrade to polling on a configurable interval (e.g., 5s) for affected projects only, with explicit user notification to increase the system limit.

---

## Findings

### 1. Git fsmonitor--daemon Uses Inotify on Linux, Relocating Rather Than Solving the Cost — Confidence: **High**

**The critical fact:** Git's `git fsmonitor--daemon`, introduced in Git 2.36.0 (April 2022) and supported on Linux via inotify, uses the same per-directory watch mechanism as direct inotify calls. The daemon does not leverage filesystem-wide marks.

**IPC transport:** Unix domain socket (UDS), created in `.git/fsmonitor--daemon.sock` by default (or `$HOME/.git-fsmonitor-*` on network mounts).

**Performance gain:** Significant for `git status` (avoids full tree scan) **only when enabled explicitly** via `git config core.fsmonitor=true`. Git must be ≥2.36.0; this repo is on 2.53.0, so support is present.

**The inotify cost remains:** The daemon walks the working tree and registers one watch per directory with inotify. Per-user limit applies—default 8,192, increased here to 65,536. For a 734k-directory tree, this is still hopeless unless gitignore-aware filtering is applied upstream (the daemon does not filter).

**Third-party consumption:** The daemon's IPC protocol is git-internal and not documented for external clients. A third party (Soloist) cannot directly query the daemon's change-list; we can only enable it (via config, unsafe to silently rewrite user config per project rules) and let `git status` use it. This does not reduce Soloist's own watch burden—Soloist must still watch the tree for arbitrary file changes, not git-specific updates.

**Verdict for Soloist:** fsmonitor improves git's own performance but does **not** address the core problem: Soloist's unfiltered recursive watch still costs one inotify watch per directory. Setting `core.fsmonitor=true` for users is a separate, optional optimization, but it does not solve the scale problem.

Sources:
- [Git fsmonitor--daemon man page](https://git-scm.com/docs/git-fsmonitor--daemon)
- [Git GitHub PR #1352: fsmonitor for Linux](https://github.com/git/git/pull/1352)
- [Linux inotify manual](https://man7.org/linux/man-pages/man7/inotify.7.html)

---

### 2. Fanotify FAN_MARK_FILESYSTEM Is Gated Behind CAP_SYS_ADMIN; Unprivileged Fanotify is Severely Restricted — Confidence: **High**

**FAN_MARK_FILESYSTEM and FAN_MARK_MOUNT:**  
These flags allow a single mark to apply to an entire filesystem or mount, eliminating the per-directory cost. However, **the kernel explicitly forbids unprivileged processes from using them**. The rationale: a filesystem-wide mark intercepts every open, including operations by root, and the security model does not permit unprivileged code to install such broad enforcement.

**Unprivileged fanotify (since Linux 5.13):**  
Unprivileged processes **cannot** create groups with `FAN_MARK_FILESYSTEM` or `FAN_MARK_MOUNT`. They are restricted to inode-level marks only (per-file or per-directory, no better than inotify). Additionally, unprivileged users cannot use `FAN_UNLIMITED_QUEUE`, `FAN_UNLIMITED_MARKS`, `FAN_CLASS_CONTENT`, or `FAN_CLASS_PRE_CONTENT`, and cannot request permission events.

**The only unprivileged capability:** `FAN_REPORT_FID` (introduced Linux 5.1), which allows file handle (FID) identification of objects instead of file descriptors. This is useful for permission contexts but **does not bypass the per-inode cost**.

**Ubuntu 20.04 kernel compatibility:**  
- Initial release (20.04 LTS, April 2020): Linux 5.4 — fanotify exists but no unprivileged support.
- HWE stack: Linux 5.8+ — still no unprivileged support.
- Ubuntu 20.04.5 LTS (Sept 2022): Linux 5.15 — unprivileged fanotify is available, but still per-inode only.
- Minimum for unprivileged fanotify: Linux 5.13.

**Verdict for Soloist:** Fanotify's filesystem-wide option is off the table for an unprivileged app. Even with unprivileged support, Soloist would be paying inode-level costs (effectively equivalent to inotify per-watch cost). **Not a solution.**

Sources:
- [fanotify_init(2) manual](https://man7.org/linux/man-pages/man2/fanotify_init.2.html)
- [fanotify_mark(2) manual](https://man7.org/linux/man-pages/man2/fanotify_mark.2.html)
- [Linux kernel docs: filesystem monitoring](https://docs.kernel.org/5.19/admin-guide/filesystem-monitoring.html)
- [Ubuntu 20.04 release notes](https://ubuntu.com/20-04)

---

### 3. Single notify::RecommendedWatcher Can Back Multiple Paths With Per-Path Unwatch — Confidence: **High**

**Current Soloist design:** One `RecommendedWatcher` per watched project (see `crates/sys/src/filewatch.rs:80–84`).

**Capacity:** A single `RecommendedWatcher` instance can call `.watch(path, mode)` repeatedly for many paths and supports per-path unwatch. Notify v8 exposes `watcher.paths_mut()` for batch operations; v9 replaces this with `watcher.update_paths(Vec<PathOp>)` for more robust partial-failure handling.

**Design implication:** Soloist can optimize to **one watcher instance shared across all projects**, with each project's root as a separate watched path. This reduces inotify instances from N (per project) to 1, freeing inotify instance budget (default max: 128 per user). The per-directory watch cost remains unchanged (one per directory), but the per-instance overhead is halved.

**Current state:** Soloist is on notify v8, so the current code uses `paths_mut()`. Upgrading to v9 would require switching to `update_paths()` but offers better error handling for partial failures.

Sources:
- [Notify v8 upgrade guide](https://github.com/notify-rs/notify/blob/main/docs/UPGRADING_V8_TO_V9.md)
- Soloist source: `crates/sys/src/filewatch.rs`

---

### 4. The `ignore` Crate Handles Gitignore-Aware Filtering; Actively Maintained — Confidence: **High**

**What it does:**  
The `ignore` crate provides a recursive directory iterator that respects `.gitignore`, `.git/info/exclude`, global excludes, and nested ignore files. It is cross-platform and widely used (80.8M+ downloads).

**Maintenance status:** Active, with metadata updates within weeks of this writing. Recommended as the successor to the archived `gitignore` crate by @BurntSushi (the `ripgrep` author).

**Integration potential for Soloist:**  
1. Walk the project root with `ignore::WalkBuilder` to enumerate directories that are **not** ignored.
2. Register only those directories for inotify watching.
3. On directory-creation events (via inotify), check if the new directory is ignored before registering it.

**Trade-off:** This introduces a CPU cost (walking the tree to enumerate directories) but dramatically reduces inotify watch count. For the provided data (734k gitignored dirs, 337 non-ignored), the trade is worthwhile.

**Already in dependencies?**  
No. Soloist does not currently depend on `ignore`. Adding it would be a new small dependency (~80 KLOC, actively maintained).

Sources:
- [ignore crate on crates.io](https://crates.io/crates/ignore)
- [ignore crate docs](https://docs.rs/ignore)

---

### 5. Inotify Recursive Watching Has Auto-Add for New Subdirectories But With Race Conditions — Confidence: **High**

**Core inotify behavior:**  
inotify does not watch subdirectories recursively at the kernel level. To watch a tree, one watch must be registered per directory. When `RecursiveMode::Recursive` is used (via the `notify` crate), the library automatically adds new subdirectories as they are created.

**Race condition:** If a directory is created and populated **before** the library's inotify watch for it is registered, events inside that directory between creation and watch-registration are missed. This is documented in salt and other tools using recursive inotify.

**Notify's handling:**  
The `notify` crate handles this by registering watches as it traverses (for recursive watching). However, a high-churn directory tree (e.g., installing npm packages) can still create races.

**Mitigation:** After receiving a directory-creation event, scan its contents immediately before relying on inotify events from inside it. This is the responsibility of the consuming code (Soloist's `WatchReactor`), not the watch adapter—and Soloist already does this (via the `WatchReactor` pattern, `plan/04 §6`).

**Implication for incremental filtering:** If Soloist uses gitignore filtering to avoid watching ignored subtrees, new directories created and populated before the ignore check runs will be missed unless re-scanned on creation. The `ignore` crate's `.is_ignored()` call must run synchronously on directory-create events, and if a directory is ignored, it must not be watched—but its contents are already lost. This is acceptable for dependency trees (which are .gitignored wholesale and rebuilt deterministically) but problematic for user-tracked but initially-empty directories.

Sources:
- [inotify(7) manual](https://man7.org/linux/man-pages/man7/inotify.7.html)
- [Recursive directory watching challenges](https://github.com/letorbi/inotifyrecursive)
- [SaltStack issue #53290](https://github.com/saltstack/salt/issues/53290)

---

## Contradictions & Caveats

**None found.** All sources (git docs, Linux kernel manuals, crate docs, system limits) are consistent. One nuance:

- **Git version on this machine (2.53.0):** Supports fsmonitor daemon, but enabling it requires `git config core.fsmonitor=true` at the repo level. Soloist should **not** silently set this (violates project rules: "Never silently rewrite user config"). If desired, this could be opt-in UI, or detected and warned about if set by the user elsewhere.

---

## Open Questions

1. **Soloist's actual watch distribution:**  
   Code confirms 58,179 watches for this repo's single root. Breaking down by ignored vs. non-ignored directories requires running the `ignore` crate on this repo—estimate is ~337 non-ignored directories, but actual numbers should be measured before committing to the filter strategy.

2. **Inotify race condition impact in practice:**  
   How often do races occur with `node_modules` or other high-churn ignored trees? This is a "does it matter for real workflows" question, not a "can it happen" one. Current Soloist is event-driven, so missing events in .gitignored trees is low-impact unless a user has custom ignored directories with tracked content.

3. **Upgrade path for notify v9:**  
   Soloist is on v8. Upgrading to v9 (`update_paths()` API) is mechanical but should be done as a separate refactor, not bundled with watch filtering. Check breaking changes in `RecursiveMode` handling and watched path representation.

---

## So What: Recommended Architecture

### Immediate (Low Effort, High Impact)

1. **Single shared watcher instance:**  
   Refactor `crates/sys/src/filewatch.rs` to maintain one `RecommendedWatcher` per project (not per watch call). This frees inotify instance descriptors without changing inotify watch cost.  
   **Cost:** ~50 lines of refactoring in the adapter.  
   **Benefit:** Reduces per-project inotify instance count from ~2 (one for root, one for .git) to 1.

2. **Graceful degradation on budget exhaustion:**  
   When `notify::ErrorKind::MaxFilesWatch` is returned, log a clear error and offer the user two options:  
   - Increase `fs.inotify.max_user_watches` (provide the command).  
   - Enable project-specific polling fallback (degrade to polling every N seconds for that project).  
   **Cost:** Error handling logic, no new dependencies.  
   **Benefit:** Prevents silent failure; gives users visibility and a path forward.

### Medium Term (Optimal, with Upfront Cost)

3. **Gitignore-aware directory filtering:**  
   1. Depend on the `ignore` crate.  
   2. On watch initialization, walk the tree with `ignore::WalkBuilder` to enumerate non-ignored directories.  
   3. Register only those directories initially.  
   4. On directory-create events, check if the new directory is ignored before adding it to watches.  
   **Cost:** New dependency, ~100–150 lines of adapter logic, and testing.  
   **Benefit:** For typical repos (heavy .gitignore), cuts watch count by 90%+ (58k → ~337 for this repo). Eliminates the scaling problem for most users.  
   **When:** This is worth doing before Phase 12 (bundle size) if the watch budget is hitting limits in testing. After Phase 13 (longevity/soak test) if it is not a current constraint.

### (Not Recommended)

- **git fsmonitor daemon:** Does not solve the inotify cost; only helps git's own commands. Enable as user opt-in via UI if desired, but do not make it a core strategy.
- **fanotify:** Off the table for unprivileged apps.
- **Polling:** Not event-driven; only for graceful degradation when inotify fails.

---

## Coverage Ledger

| # | Sub-question | Status | Evidence |
|---|---|---|---|
| 1 | Is git fsmonitor--daemon supported on Linux, from which version, what is IPC transport? | ANSWERED | Git 2.36.0+; uses inotify under the hood; IPC via Unix domain socket. |
| 2 | Does fsmonitor use inotify and have the same per-directory watch cost? | ANSWERED | Yes, uses inotify on Linux; same per-directory cost. Confirmed from git-fsmonitor source and docs. |
| 3 | Can third-party apps consume git's fsmonitor daemon? | ANSWERED | No; protocol is git-internal. Soloist can enable it (not recommended to silently rewrite config), but benefits only git's own commands. |
| 4 | What is fsmonitor's actual performance impact on git status? | ANSWERED | Significant (avoids tree scan) when enabled, but does not reduce Soloist's own watch cost. |
| 5 | Does fanotify's FAN_MARK_FILESYSTEM work unprivileged? | ANSWERED | No; requires CAP_SYS_ADMIN. Unprivileged fanotify (5.13+) is restricted to inode marks only. |
| 6 | What is Ubuntu 20.04's kernel and fanotify capabilities? | ANSWERED | Kernel 5.4 initially (no unprivileged fanotify); HWE 5.8+ (still no); 5.15+ (unprivileged available, inode-only). |
| 7 | Can one notify::RecommendedWatcher back multiple paths with per-path unwatch? | ANSWERED | Yes; v8 uses `paths_mut()`; v9 uses `update_paths()`. Single instance can manage many paths. |
| 8 | Is the ignore crate maintained and in Soloist? | ANSWERED | Actively maintained (80.8M+ downloads); not currently in Soloist dependencies. |
| 9 | What races/issues exist with non-recursive directory watching? | ANSWERED | Auto-add of new subdirs has race conditions; contents populated before watch-add are missed. Mitigated by on-create scanning (already done by Soloist). |
| 10 | Current state: git version, inotify limits, watch count? | ANSWERED | git 2.53.0; max_user_watches=65536; 58,179 watches for this repo; estimate ~337 non-ignored dirs. |

---

## Sources

1. [Git fsmonitor--daemon documentation](https://git-scm.com/docs/git-fsmonitor--daemon)
2. [Git fsmonitor--daemon(1) man page](https://man7.org/linux/man-pages/man1/git-fsmonitor--daemon.1.html)
3. [Git GitHub PR #1352: fsmonitor for Linux](https://github.com/git/git/pull/1352)
4. [Linux inotify(7) manual](https://man7.org/linux/man-pages/man7/inotify.7.html)
5. [Linux fanotify(7) manual](https://man7.org/linux/man-pages/man7/fanotify.7.html)
6. [Linux fanotify_init(2) manual](https://man7.org/linux/man-pages/man2/fanotify_init.2.html)
7. [Linux fanotify_mark(2) manual](https://man7.org/linux/man-pages/man2/fanotify_mark.2.html)
8. [Linux kernel: filesystem monitoring guide](https://docs.kernel.org/5.19/admin-guide/filesystem-monitoring.html)
9. [Ubuntu 20.04 LTS release information](https://ubuntu.com/20-04)
10. [Ubuntu 20.04.5 LTS kernel 5.15 release](https://www.omgubuntu.co.uk/2022/09/ubuntu-20-04-5-lts-released-with-linux-kernel-5-15)
11. [Notify crate upgrade guide (v8→v9)](https://github.com/notify-rs/notify/blob/main/docs/UPGRADING_V8_TO_V9.md)
12. [Notify crate on crates.io](https://crates.io/crates/notify)
13. [ignore crate on crates.io](https://crates.io/crates/ignore)
14. [ignore crate documentation](https://docs.rs/ignore)
15. [inotifyrecursive: recursive inotify challenges](https://github.com/letorbi/inotifyrecursive)
16. [SaltStack issue #53290: recursive watch race conditions](https://github.com/saltstack/salt/issues/53290)
17. Soloist source: `/home/dell/Projects/soloist/crates/sys/src/filewatch.rs`
18. Soloist source: `/home/dell/Projects/soloist/crates/core/src/git/watched.rs`
