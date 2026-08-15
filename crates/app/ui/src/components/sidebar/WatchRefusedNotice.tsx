import { AdvisoryNotice } from "@/components/AdvisoryNotice";
import { cn } from "@/lib/utils";
import type { PurposeRefusals, WatchError, WatchPurpose } from "@/domain";

// What a refused purpose costs the user, named once per purpose. A project whose commands declare
// no `restart_when_changed` globs never asks for a restart watch at all, while the git rail asks
// for one over every open project, so the losses have to be claimed separately or the ordinary
// project is told restart-on-change stopped when it never ran.
const CONSEQUENCE: Record<WatchPurpose, string> = {
  restarts: "restart-on-change",
  git_status: "live git status",
};

// What each refusal leaves the user able to do about it. Only the exhausted budget has a fix, and
// naming the setting is the whole value of saying anything at all — the other two are conditions to
// know about, not to act on.
const CAUSE: Record<WatchError, string> = {
  budget_exhausted:
    "The system's file-watch limit is exhausted; raising fs.inotify.max_user_watches restores it.",
  unwatchable: "This project's directory could not be read.",
  unavailable: "The filesystem watcher could not start.",
};

// The order the losses are read in. Fixed here because the refusals arrive keyed by an unordered
// map, and a sentence that reshuffles itself between announcements reads as a new condition.
const PURPOSES = Object.keys(CONSEQUENCE) as WatchPurpose[];

// Says which of a project's file watches the OS turned down, what that costs, and why.
//
// Worth a standing strip rather than a passing toast because the consequence is an absence: watched
// files stop reloading their command and the git rail stops following the working tree, and both
// failures look exactly like a project nobody is editing. Nothing else on screen would ever
// contradict that impression, so this notice stays up for as long as the condition does.
//
// Each loss and each cause is stated once and the sentence composes them, so the refusals cannot
// drift into different accounts of what the user has lost.
export function WatchRefusedNotice({
  refusals,
  className,
}: {
  refusals: PurposeRefusals;
  className?: string;
}) {
  const refused = PURPOSES.flatMap((purpose) => {
    const reason = refusals[purpose];
    return reason ? [{ purpose, reason }] : [];
  });
  const losses = refused.map(({ purpose }) => CONSEQUENCE[purpose]).join(" or ");
  const stopped = refused.length > 1 ? "they have" : "it has";
  // Both purposes watch the same tree, so they usually fail for the same reason; saying it twice
  // would read as two separate faults.
  const causes = new Set(refused.map(({ reason }) => CAUSE[reason]));

  return (
    // The rail is narrow and the setting to raise is one long unbreakable token, so the strip wraps
    // inside a word rather than reaching past the sidebar it sits in.
    <AdvisoryNotice urgency="status" className={cn("items-start break-words", className)}>
      {[`Not watching this project's files for ${losses}, so ${stopped} stopped.`, ...causes].join(
        " ",
      )}
    </AdvisoryNotice>
  );
}
