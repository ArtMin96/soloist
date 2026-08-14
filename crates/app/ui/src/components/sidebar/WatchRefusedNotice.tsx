import { AdvisoryNotice } from "@/components/AdvisoryNotice";
import { cn } from "@/lib/utils";
import type { WatchError } from "@/domain";

// What each refusal leaves the user able to do about it. Only the exhausted budget has a fix, and
// naming the setting is the whole value of saying anything at all — the other two are conditions to
// know about, not to act on.
const CAUSE: Record<WatchError, string> = {
  budget_exhausted:
    "The system's file-watch limit is exhausted; raising fs.inotify.max_user_watches restores it.",
  unwatchable: "Its directory could not be read.",
  unavailable: "The filesystem watcher could not start.",
};

// Says that a project's files have stopped being watched, and why.
//
// Worth a standing strip rather than a passing toast because the consequence is an absence: watched
// files stop reloading their command and the git rail stops following the working tree, and both
// failures look exactly like a project nobody is editing. Nothing else on screen would ever
// contradict that impression, so this notice stays up for as long as the condition does.
//
// The consequence is stated once and the cause varies, so the three refusals cannot drift into
// three different accounts of what the user has lost.
export function WatchRefusedNotice({
  reason,
  className,
}: {
  reason: WatchError;
  className?: string;
}) {
  return (
    // The rail is narrow and the setting to raise is one long unbreakable token, so the strip wraps
    // inside a word rather than reaching past the sidebar it sits in.
    <AdvisoryNotice className={cn("items-start break-words", className)}>
      Not watching this project's files, so restart-on-change and live git status have stopped.{" "}
      {CAUSE[reason]}
    </AdvisoryNotice>
  );
}
