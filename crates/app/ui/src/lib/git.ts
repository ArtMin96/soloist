import type {
  BranchInfo,
  ChangeKind,
  CheckState,
  GitFileStatus,
  HunkRange,
  MergeMethod,
  ReviewThread,
} from "@/domain";

/**
 * The single source for turning a version-control change into its display, alongside
 * `lib/status.ts` for process status. Redundant by the same rule: a letter *and* a colour *and*
 * a word, so a change survives colour blindness and a grayscale screenshot. The exhaustive
 * Record makes the compiler require an entry for every kind the core can report.
 */
export interface ChangeDisplay {
  /** The letter shown beside the path — the same one version control prints. */
  letter: string;
  /** Human label, e.g. "Modified". The letter's accessible name, and its tooltip. */
  label: string;
  /** Tailwind text-color utility, bound to a `--git-*` token. */
  toneClass: string;
  /** A path that no longer exists is drawn struck through, so the row reads as gone. */
  gone: boolean;
}

export const CHANGE: Record<ChangeKind, ChangeDisplay> = {
  modified: { letter: "M", label: "Modified", toneClass: "text-git-modified", gone: false },
  type_changed: {
    letter: "T",
    label: "Type changed",
    toneClass: "text-git-modified",
    gone: false,
  },
  added: { letter: "A", label: "Added", toneClass: "text-git-added", gone: false },
  deleted: { letter: "D", label: "Deleted", toneClass: "text-git-deleted", gone: true },
  renamed: { letter: "R", label: "Renamed", toneClass: "text-git-added", gone: false },
  copied: { letter: "C", label: "Copied", toneClass: "text-git-added", gone: false },
  untracked: { letter: "U", label: "Untracked", toneClass: "text-git-added", gone: false },
  conflicted: { letter: "C", label: "Conflicted", toneClass: "text-git-conflicted", gone: false },
};

/** The tone a path version control was told to ignore is drawn in, on the Files tab. */
export const IGNORED_TONE_CLASS = "text-git-ignored";

/**
 * The one change a row shows for a path changed on both sides of the index. The working tree is
 * what the user is looking at and what they can act on, so it wins; the staged change is the
 * one already recorded. A path with neither is not a change at all.
 */
export function primaryChange(status: GitFileStatus): ChangeKind | null {
  return status.unstaged ?? status.staged;
}

// Which change a folder takes from its children. A folder is a summary, so it shows the child
// that most wants attention: an unresolved conflict first, then a change to what is tracked,
// then a path version control does not track yet. An exhaustive Record rather than a chain of
// comparisons, for the same reason the display map is one — a kind added to the union stops the
// build here until it has been placed.
const SEVERITY: Record<ChangeKind, number> = {
  conflicted: 5,
  deleted: 4,
  modified: 3,
  type_changed: 3,
  renamed: 2,
  copied: 2,
  added: 2,
  untracked: 1,
};

/** The stronger of two changes — what a folder inherits from a child. */
export function strongerChange(a: ChangeKind, b: ChangeKind): ChangeKind {
  return SEVERITY[b] > SEVERITY[a] ? b : a;
}

/**
 * How much of a path is recorded for the next commit, which is what its checkbox shows. A path
 * changed on both sides is neither staged nor unstaged but part of each, and says so rather than
 * rounding to one — ticking it then stages the rest.
 */
export type StagedState = "staged" | "unstaged" | "partial";

export function stagedState(status: GitFileStatus): StagedState {
  if (status.staged === null) return "unstaged";
  return status.unstaged === null ? "staged" : "partial";
}

/**
 * The key one action on one hunk is tracked by while it is in flight. Built from where the hunk
 * falls, which is also how the core names it — so a row that scrolls out of view and back comes
 * back to the same state rather than to whatever the new row in that position holds.
 */
export function hunkKey(path: string, hunk: HunkRange): string {
  return `${path}@${hunk.old_start},${hunk.old_lines},${hunk.new_start},${hunk.new_lines}`;
}

/**
 * The single source for turning a check's state into its display, on the same redundant rule the
 * change map follows — a glyph *and* a tone *and* a word. The exhaustive Record makes the compiler
 * require an entry for every state the core can report.
 */
export interface CheckDisplay {
  /** What the state is called, which is also the glyph's accessible name. */
  label: string;
  /** Tailwind text-color utility, bound to the same `--git-*` tokens the change letters use, so
   *  version control spends one saturated vocabulary rather than two. */
  toneClass: string;
}

export const CHECK: Record<CheckState, CheckDisplay> = {
  pending: { label: "Running", toneClass: "text-git-modified" },
  passed: { label: "Passed", toneClass: "text-git-added" },
  failed: { label: "Failed", toneClass: "text-git-deleted" },
  skipped: { label: "Skipped", toneClass: "text-git-ignored" },
  cancelled: { label: "Cancelled", toneClass: "text-git-ignored" },
  unknown: { label: "Unrecognised", toneClass: "text-git-ignored" },
};

/** How a merge method is named to somebody choosing one. */
export const MERGE_METHOD: Record<MergeMethod, string> = {
  merge: "Create a merge commit",
  squash: "Squash and merge",
  rebase: "Rebase and merge",
};

/** Where a conversation hangs, or null for one about the change as a whole. */
export function threadPlace(thread: ReviewThread): string | null {
  if (thread.path === null) return null;
  return thread.line === null ? thread.path : `${thread.path}:${thread.line}`;
}

/**
 * The single source for turning what is checked out into its display, on the same redundant rule the
 * change map follows: a word beside the tone. Every standing a branch can be in is named, so a
 * branch nobody has published never reads as one that matches its upstream on hue alone.
 */
export interface BranchStanding {
  /**
   * How the branch stands against its upstream, in words. `null` only on a detached head, where
   * there is no branch to have an upstream and the name slot says so itself.
   */
  label: string | null;
  /** Tailwind text-color utility, bound to a `--git-branch-*` token. */
  toneClass: string;
  /** Commits the branch holds that its upstream does not, `0` where there is no comparison. */
  ahead: number;
  /** Commits the upstream holds that the branch does not, `0` where there is no comparison. */
  behind: number;
}

/** The tone a branch known to match its upstream carries. */
const SYNCED_TONE_CLASS = "text-git-branch-synced";
/** The tone every other standing carries: nothing to compare against, or something left to settle. */
const UNSETTLED_TONE_CLASS = "text-git-branch-local";

/** A branch that tracks nothing: it exists here and nowhere else this repository knows of. */
const LOCAL_ONLY = "Local only";
/** A branch that tracks something whose position nothing has fetched yet. */
const NOT_FETCHED = "Not fetched";
const UP_TO_DATE = "Up to date";

export function branchStanding(branch: BranchInfo): BranchStanding {
  const settled = { ahead: 0, behind: 0 };
  // A detached head is not a branch and has no upstream to stand against. The badge names it where
  // a branch name would go, so there is nothing left for the standing to add beside it.
  if (branch.name === null) return { label: null, toneClass: UNSETTLED_TONE_CLASS, ...settled };
  // Tracking nothing and tracking something unfetched are both "no comparison to make", and they
  // are not the same thing to the reader: the first is published nowhere and wants a push, the
  // second wants a fetch before anything can be said about it.
  if (branch.upstream === null) {
    return { label: LOCAL_ONLY, toneClass: UNSETTLED_TONE_CLASS, ...settled };
  }
  switch (branch.sync.state) {
    case "unknown":
      return { label: NOT_FETCHED, toneClass: UNSETTLED_TONE_CLASS, ...settled };
    case "up_to_date":
      return { label: UP_TO_DATE, toneClass: SYNCED_TONE_CLASS, ...settled };
    case "ahead":
      return {
        label: `${branch.sync.ahead} ahead`,
        toneClass: UNSETTLED_TONE_CLASS,
        ahead: branch.sync.ahead,
        behind: 0,
      };
    case "behind":
      return {
        label: `${branch.sync.behind} behind`,
        toneClass: UNSETTLED_TONE_CLASS,
        ahead: 0,
        behind: branch.sync.behind,
      };
    case "diverged":
      return {
        label: `${branch.sync.ahead} ahead, ${branch.sync.behind} behind`,
        toneClass: UNSETTLED_TONE_CLASS,
        ahead: branch.sync.ahead,
        behind: branch.sync.behind,
      };
  }
}
