import { AdvisoryNotice } from "@/components/AdvisoryNotice";
import { cn } from "@/lib/utils";
import type { PurposeLimits, WatchError, WatchPurpose } from "@/domain";

// What a refused purpose costs the user, named once per purpose. A project whose commands declare
// no `restart_when_changed` globs never asks for a restart watch at all, while the git rail asks
// for one over every open project, so the losses have to be claimed separately or the ordinary
// project is told restart-on-change stopped when it never ran.
const CONSEQUENCE: Record<WatchPurpose, string> = {
  restarts: "restart-on-change",
  git_status: "live git status",
};

// What each refusal leaves the user able to do about it. Two of them have a remedy and the remedies
// are not interchangeable: the machine's own limit is raised with a setting, while the share Soloist
// keeps to is freed by closing a project — raising the setting releases none of what Soloist already
// holds. Naming one for the other sends the user to change something that will not help now. The
// remaining two are conditions to know about, not to act on.
const CAUSE: Record<WatchError, string> = {
  budget_exhausted:
    "The system's file-watch limit is exhausted; raising fs.inotify.max_user_watches restores it.",
  share_exhausted:
    "Soloist is holding every watch it allows itself — a share of the system's limit, split between open projects. Closing a project frees its watches.",
  unwatchable: "This project's directory could not be read.",
  unavailable: "The filesystem watcher could not start.",
};

// Why a purpose is degraded — one project-wide condition, said once no matter how many purposes it
// touches, because both purposes lose it to the same share. The system's own limit is not what was
// met here: the tree outgrew what Soloist gives one open project, so the system may have watches to
// spare and the setting that raises it is beside the point.
const DEGRADED_CAUSE =
  "Watching only this project's repository state — its file tree needs more watches than Soloist gives one open project.";

// What a degraded purpose still does and what it no longer does, named once per purpose. Neither
// loses everything: the repository's own state stays watched event-driven, so a degraded purpose is
// reduced rather than stopped, and the notice has to say what is still true or it reads as a bigger
// failure than it is.
const DEGRADED_EFFECT: Record<WatchPurpose, string> = {
  restarts: "Restart-on-change only sees the directories your patterns name.",
  git_status:
    "Live git status still follows commits and staging; edits to your files will not refresh it on their own.",
};

// The order the purposes are read in. Fixed here because limits arrive keyed by an unordered map,
// and a sentence that reshuffles itself between announcements reads as a new condition.
const PURPOSES = Object.keys(CONSEQUENCE) as WatchPurpose[];

// Says which of a project's file watches the OS has limited, what that costs (or doesn't), and why.
//
// Worth a standing strip rather than a passing toast because a refusal's consequence is an absence:
// watched files stop reloading their command and the git rail stops following the working tree, and
// that failure looks exactly like a project nobody is editing. Nothing else on screen would ever
// contradict that impression, so the strip stays up for as long as the condition does.
//
// A degradation is not that: the repository's own state is still watched event-driven, so commits,
// staging, checkouts and fetches are still seen live — only a refresh triggered by editing a file
// deep in the tree is lost. Saying so, and reading calmer than a refusal while doing it, is the
// whole point of telling the two apart rather than folding a degradation into "refused".
//
// Each loss and each cause is stated once and the sentences compose them, so the limits cannot drift
// into different accounts of what the user has lost.
export function WatchLimitNotice({
  limits,
  className,
}: {
  limits: PurposeLimits;
  className?: string;
}) {
  const held = PURPOSES.flatMap((purpose) => {
    const limit = limits[purpose];
    return limit ? [{ purpose, limit }] : [];
  });
  const refused = held.filter(
    (entry): entry is { purpose: WatchPurpose; limit: { refused: WatchError } } =>
      entry.limit !== "degraded",
  );
  const degraded = held.filter((entry) => entry.limit === "degraded");

  const sentences: string[] = [];
  if (refused.length > 0) {
    const losses = refused.map(({ purpose }) => CONSEQUENCE[purpose]).join(" or ");
    const stopped = refused.length > 1 ? "they have" : "it has";
    // Both purposes watch the same tree, so they usually fail for the same reason; saying it twice
    // would read as two separate faults.
    const causes = new Set(refused.map(({ limit }) => CAUSE[limit.refused]));
    sentences.push(
      `Not watching this project's files for ${losses}, so ${stopped} stopped.`,
      ...causes,
    );
  }
  if (degraded.length > 0) {
    sentences.push(DEGRADED_CAUSE, ...degraded.map(({ purpose }) => DEGRADED_EFFECT[purpose]));
  }

  // Any refusal keeps the strip's urgent, amber tone — something the user asked for is genuinely
  // gone. A project limited only by degradation reads as informational instead: a neutral, bordered
  // surface, the same one the rest of the app uses for a panel that is stating a fact rather than
  // raising one.
  const calm = refused.length === 0;

  return (
    // The rail is narrow and the setting to raise is one long unbreakable token, so the strip wraps
    // inside a word rather than reaching past the sidebar it sits in.
    <AdvisoryNotice
      urgency="status"
      className={cn("items-start break-words", calm && "border-border bg-card", className)}
    >
      {sentences.join(" ")}
    </AdvisoryNotice>
  );
}
