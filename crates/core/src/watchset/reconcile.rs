//! The planning half of [`ProjectWatchSet`](super::ProjectWatchSet): what each open project's
//! watches should be, and reconciling what is actually held to that. [`super::set`] owns the
//! event loop that calls [`ProjectWatchSet::resync`] and the incremental
//! (`Appeared`/`Vanished`) maintenance that runs between one re-sync and the next.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::filewatch::{compile, literal_prefix, FileChange};
use crate::ids::ProjectId;
use crate::ports::ProjectRecord;
use crate::supervision::run_blocking;
use crate::vcs::{REFS_DIR, STATE_DIR};
use crate::watch::{WatchError, WatchLimit, WatchOutcome, WatchPurpose};

use super::ignored_names;
use super::plan::{plan, ProjectPlan};
use super::scan::{Scan, ScanRequest};
use super::set::{ProjectWatchSet, Registered, RunState};

impl ProjectWatchSet {
    /// Reconciles every open project's watches: ensures the session is open, hands back what a
    /// since-closed project held, and re-plans a project whose plan inputs moved — it is new, or
    /// its root, its watch-eligible globs or its share of the budget changed, or `force_rescan`
    /// says the filesystem may have moved without any of those signals catching it. Then, for
    /// every project and regardless of whether it re-planned, it reconciles what is held to what
    /// the plan wants in both directions: releasing what the plan no longer covers, and retrying
    /// whatever it wants that is not currently held. That last step, run unconditionally on every
    /// re-sync, is what re-establishes a refusal that has since cleared without needing a fresh
    /// scan to trigger it (the [`crate::git::routing`] guarantee, at this layer).
    ///
    /// `force_rescan` is [`super::set::ProjectWatchSet::run_loop`]'s answer to a dropped
    /// change: a directory whose `Appeared` never arrived looks, from a settled project's own
    /// plan, identical to a tree nobody touched — nothing about its root, globs, or share moved
    /// — so only an explicit "the ground may have shifted, look again" makes the re-scan happen.
    pub(super) async fn resync(
        &self,
        state: &mut RunState,
        changes_tx: &mpsc::Sender<FileChange>,
        dropped: &Arc<AtomicU64>,
        force_rescan: bool,
    ) {
        let Ok(records) = self.projects.list() else {
            return;
        };
        let open: HashSet<ProjectId> = records.iter().map(|record| record.id).collect();
        let globs_by_project = self.eligible_globs();

        let session = match &state.session {
            Some(session) => session.clone(),
            None => match self.watcher.open(changes_tx.clone(), dropped.clone()) {
                Ok(session) => {
                    state.session = Some(session.clone());
                    session
                }
                Err(_) => {
                    self.report_unavailable(&records, &globs_by_project);
                    return;
                }
            },
        };

        // Before anything is planned: a project closed since the last re-sync still holds its
        // watches, and what returning them frees is exactly what the projects still open are
        // about to be offered.
        let closed: Vec<ProjectId> = state
            .projects
            .keys()
            .filter(|project| !open.contains(project))
            .copied()
            .collect();
        for project in closed {
            self.release_project(state, project);
        }

        let share = state.registrations.share(open.len());
        let mut git_outcomes = Vec::new();
        let mut restart_outcomes = Vec::new();

        for record in &records {
            let project = record.id;
            let root = record.root.clone();
            let globs = globs_by_project.get(&project).cloned().unwrap_or_default();
            let restart_eligible = !globs.is_empty();

            let needs_replan = force_rescan
                || match state.projects.get(&project) {
                    None => true,
                    Some(existing) => {
                        existing.root != root || existing.globs != globs || existing.share != share
                    }
                };
            if needs_replan {
                let computed = self.replan(&root, &globs, share).await;
                let paths: HashSet<PathBuf> = computed
                    .directories
                    .into_iter()
                    .chain(computed.trees)
                    .collect();
                state.projects.insert(
                    project,
                    Registered {
                        root: root.clone(),
                        globs: globs.clone(),
                        share,
                        paths,
                        limit: computed.limit,
                    },
                );
            }

            let refs_path = root.join(STATE_DIR).join(REFS_DIR);
            let wanted: HashSet<PathBuf> = state
                .projects
                .get(&project)
                .map(|entry| entry.paths.clone())
                .unwrap_or_default();
            let stale: Vec<PathBuf> = state
                .registrations
                .held_by(project)
                .filter(|held| !wanted.contains(*held))
                .cloned()
                .collect();
            for path in stale {
                state
                    .registrations
                    .release(&path, project, session.as_ref());
            }
            let mut root_refused = None;
            for path in &wanted {
                if state.registrations.is_held(path, project) {
                    continue;
                }
                let tree = *path == refs_path;
                if let Err(err) =
                    state
                        .registrations
                        .register(path, project, tree, session.as_ref())
                {
                    if *path == root {
                        root_refused = Some(err);
                    } else {
                        // Only the working-tree root's refusal is reported (see below): a
                        // project that is not a repository has no `.git` to watch, so a
                        // state-dir refusal is the ordinary case rather than a loss.
                        tracing::warn!(
                            path = %path.display(),
                            refusal = %err,
                            "a project's watched files will not report: the directory could not be watched",
                        );
                    }
                }
            }

            let mut limit = state
                .projects
                .get(&project)
                .map(|entry| entry.limit.clone())
                .unwrap_or_default();
            if let Some(err) = root_refused {
                limit.insert(WatchPurpose::GitStatus, WatchLimit::Refused(err));
                if restart_eligible {
                    limit.insert(WatchPurpose::Restarts, WatchLimit::Refused(err));
                }
            }
            git_outcomes.push(WatchOutcome {
                project,
                limit: limit.get(&WatchPurpose::GitStatus).copied(),
            });
            if restart_eligible {
                restart_outcomes.push(WatchOutcome {
                    project,
                    limit: limit.get(&WatchPurpose::Restarts).copied(),
                });
            }
        }

        self.status.resynced(WatchPurpose::GitStatus, &git_outcomes);
        self.status
            .resynced(WatchPurpose::Restarts, &restart_outcomes);
    }

    /// Every currently watch-eligible command's globs, grouped by project — commands whose
    /// globs all fail to compile contribute nothing, the same predicate
    /// [`crate::filewatch::WatchReactor`] matches against. Sorted per project so a re-sync
    /// comparing against the cached [`Registered::globs`] is not fooled by registry iteration
    /// order alone.
    fn eligible_globs(&self) -> HashMap<ProjectId, Vec<String>> {
        let mut by_project: HashMap<ProjectId, Vec<String>> = HashMap::new();
        if let Some(supervisor) = self.supervisor.upgrade() {
            for target in supervisor.watch_targets() {
                if compile(&target.globs).is_some() {
                    by_project
                        .entry(target.project)
                        .or_default()
                        .extend(target.globs);
                }
            }
        }
        for globs in by_project.values_mut() {
            globs.sort_unstable();
        }
        by_project
    }

    /// Scans `root`'s whole tree and everything its globs ask for off the runtime, then hands the
    /// results to the pure [`plan`]. One [`run_blocking`] closure covers every scan this project
    /// needs, so the registrations [`Self::resync`] then applies for it happen as one
    /// uninterrupted batch against a single, consistent read of the filesystem.
    async fn replan(&self, root: &Path, globs: &[String], share: usize) -> ProjectPlan {
        let prefixes = prefix_requests(root, globs, share);
        let scanner = self.scanner.clone();
        let scan_root = root.to_path_buf();
        let (tree, prefix_scans) = run_blocking(move || {
            let tree = scanner.scan(ScanRequest {
                root: scan_root,
                ignored_names: ignored_names(),
                honour_repository_ignores: true,
                ceiling: share,
            });
            let prefix_scans: Vec<Scan> = prefixes
                .into_iter()
                .map(|request| scanner.scan(request))
                .collect();
            (tree, prefix_scans)
        })
        .await;
        plan(root, globs, &tree, &prefix_scans, share)
    }

    /// Reports every open project as refused for want of a working backend — the session itself
    /// could not be opened. Retried, from scratch, on the next re-sync.
    fn report_unavailable(
        &self,
        records: &[ProjectRecord],
        globs_by_project: &HashMap<ProjectId, Vec<String>>,
    ) {
        let mut git_outcomes = Vec::new();
        let mut restart_outcomes = Vec::new();
        for record in records {
            git_outcomes.push(WatchOutcome {
                project: record.id,
                limit: Some(WatchLimit::Refused(WatchError::Unavailable)),
            });
            if globs_by_project.contains_key(&record.id) {
                restart_outcomes.push(WatchOutcome {
                    project: record.id,
                    limit: Some(WatchLimit::Refused(WatchError::Unavailable)),
                });
            }
        }
        self.status.resynced(WatchPurpose::GitStatus, &git_outcomes);
        self.status
            .resynced(WatchPurpose::Restarts, &restart_outcomes);
    }
}

/// One scan request per distinct directory `globs` asks to have watched, each with the
/// repository's own ignore rules disabled — a glob names the directory, so a gitignored one it
/// can match is still watched even though the whole-tree scan skips it.
///
/// A glob whose first component is already a metacharacter names no directory but can still
/// match at any depth ([`literal_prefix`] reports the empty path for it), so what it asks for is
/// the whole root. That scan — and only that one — is told to skip the always-ignored directory
/// names: a change inside one never restarts anything ([`crate::filewatch::is_ignored`]), so
/// descending into `node_modules` or `target` would spend the project's whole share on
/// registrations that cannot fire. A glob naming such a directory explicitly is scanned without
/// that list, since the directory it names is the walk's own root.
///
/// Bounded twice over: every walk stops at `ceiling`, and [`plan`] drops a scan that does not fit
/// the share rather than registering past it.
///
/// The whole-root request comes **last**, because [`plan`] fits them in order and this is the one
/// a project can most afford to lose: a glob naming a directory states a small, specific intent
/// and is cheap to satisfy, where the whole-root scan is the broad fallback and is large enough to
/// exhaust the share on its own.
fn prefix_requests(root: &Path, globs: &[String], ceiling: usize) -> Vec<ScanRequest> {
    let mut named: HashSet<PathBuf> = HashSet::new();
    let mut requests = Vec::new();
    let mut whole_root = None;
    for glob in globs {
        let Some(prefix) = literal_prefix(glob) else {
            continue;
        };
        if prefix.as_os_str().is_empty() {
            whole_root = Some(ScanRequest {
                root: root.to_path_buf(),
                ignored_names: ignored_names(),
                honour_repository_ignores: false,
                ceiling,
            });
            continue;
        }
        let scan_root = root.join(prefix);
        if !named.insert(scan_root.clone()) {
            continue;
        }
        requests.push(ScanRequest {
            root: scan_root,
            ignored_names: Vec::new(),
            honour_repository_ignores: false,
            ceiling,
        });
    }
    requests.extend(whole_root);
    requests
}
