import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import {
  BASE_LABEL,
  BODY_LABEL,
  PROPOSE_LABEL,
  PROPOSING_LABEL,
  TITLE_LABEL,
} from "@/components/git/pullRequestCopy";
import type { PullRequestSuggestion } from "@/domain";

const EDIT_LABEL = "Edit details…";

/** Where the description comes from, which is the one thing about it that is not on screen. */
const FROM_COMMITS = "From this branch's commits.";
const FROM_COMMITS_IN_TEMPLATE =
  "From this branch's commits, written into the pull-request template.";

/** Why there is nothing to propose. Neither is a failure — each names the one thing that changes
 *  the answer. */
const NO_BASE =
  "The repository names no branch to merge into, so there is nothing to propose against. Naming one is what changes that.";
const NOTHING_AHEAD = (base: string) =>
  `This branch holds nothing ${base} does not, so there is nothing to propose yet. Commit something, or merge into a different branch.`;

/**
 * What the branch would be proposed as, and the press that proposes it.
 *
 * The whole of what the button will do is on screen before it is pressed — the title it would carry,
 * the two branches it would join, and where its description came from — because it makes something
 * public on a service from words the reader did not type.
 *
 * Presentational: props in, callbacks out. Whether there is anything to propose is the core's
 * answer, and where there is not, this says so and still offers the way to write one by hand.
 */
export function PullRequestProposal({
  head,
  base,
  suggestion,
  templated,
  busy,
  onPropose,
  onEdit,
}: {
  head: string;
  /** Where it would go, or `null` where the repository named none. */
  base: string | null;
  /** What it would be opened with, or `null` where there is nothing to compute one from. */
  suggestion: PullRequestSuggestion | null;
  /** Whether a starting shape was on offer, so the description's provenance can be stated. */
  templated: boolean;
  busy: boolean;
  onPropose: () => void;
  onEdit: () => void;
}) {
  if (suggestion === null || base === null) {
    return (
      <div className="flex flex-col items-start gap-3 p-4">
        <p className="max-w-[70ch] type-body text-muted-foreground">
          {base === null ? NO_BASE : NOTHING_AHEAD(base)}
        </p>
        <Button size="sm" variant="outline" onClick={onEdit}>
          {EDIT_LABEL}
        </Button>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3 p-4">
      <div className="flex flex-col gap-2.5 rounded-md border border-border p-3">
        <Fact label={TITLE_LABEL}>
          <p className="type-body font-[550]">{suggestion.title}</p>
        </Fact>
        <Fact label={BASE_LABEL}>
          <p className="truncate font-mono type-body">{`${base} ← ${head}`}</p>
        </Fact>
        <Fact label={BODY_LABEL}>
          <p className="type-body text-muted-foreground">
            {templated ? FROM_COMMITS_IN_TEMPLATE : FROM_COMMITS}
          </p>
        </Fact>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <Button size="sm" disabled={busy} onClick={onPropose}>
          {busy ? PROPOSING_LABEL : PROPOSE_LABEL}
        </Button>
        <Button size="sm" variant="outline" onClick={onEdit}>
          {EDIT_LABEL}
        </Button>
      </div>
    </div>
  );
}

/** One field of the proposal, under the same quiet label the form gives it. */
function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-1">
      <p className="type-label text-muted-foreground">{label}</p>
      {children}
    </div>
  );
}
