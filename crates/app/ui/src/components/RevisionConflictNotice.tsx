import { AdvisoryNotice } from "@/components/AdvisoryNotice";
import { Button } from "@/components/ui/button";

// The documents that carry a revision and can therefore be refused a stale write. Closed, so a new
// editor names its subject here rather than writing a fourth copy of the sentence in its own words.
export type ConflictSubject = "scratchpad" | "todo" | "template";

// What a refused revision-guarded save says, in one place for every editor that can meet one.
//
// The reassurance is the load-bearing half: the user's edits are still on screen and the stored
// document was left untouched, so this is a prompt to reload — not a report of lost work. Said once
// here because three editors were saying it in three places, which is how the same reassurance
// drifts into three slightly different promises.
export function RevisionConflictNotice({
  subject,
  revision,
  onReload,
  className,
}: {
  subject: ConflictSubject;
  revision: number;
  onReload: () => void;
  className?: string;
}) {
  return (
    <AdvisoryNotice
      className={className}
      action={
        <Button variant="outline" size="sm" onClick={onReload}>
          Reload
        </Button>
      }
    >
      This {subject} changed elsewhere (now at revision {revision}). Your edits were not saved and
      nothing was overwritten.
    </AdvisoryNotice>
  );
}
