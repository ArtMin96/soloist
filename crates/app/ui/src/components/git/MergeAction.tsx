import { useState } from "react";
import { GitMergeIcon } from "lucide-react";
import { ConfirmDialog } from "@/components/git/ConfirmDialog";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { MERGE_METHOD } from "@/lib/git";
import type { MergeMethod, PullRequest } from "@/domain";

const MERGE_LABEL = "Merge";
const METHOD_LABEL = "How to merge";
const CONFIRM_TITLE = "Merge this pull request?";
const CONFIRM_VERB = "Merge";
const KEEP_VERB = "Leave it open";

/**
 * Putting the pull request into its base branch: the ways this repository allows, and the one
 * action that does it.
 *
 * Confirmed first, because it changes a branch everybody else is working from and there is no
 * undoing it from here. Nothing is pre-empted: whether the service will actually allow it — a
 * required check that has not passed, a review that is owed — is the repository's rule, and its
 * refusal is what the reader is shown.
 *
 * Presentational: props in, callbacks out. Absent altogether where the repository allows no way of
 * merging, or where the pull request is no longer open — an action nobody may take is not an action.
 */
export function MergeAction({
  pullRequest,
  methods,
  busy,
  onMerge,
}: {
  pullRequest: PullRequest;
  /** What this repository permits, the way it prefers first. */
  methods: MergeMethod[];
  busy: boolean;
  onMerge: (method: MergeMethod) => void;
}) {
  const [chosen, setChosen] = useState<MergeMethod | null>(null);
  const [asking, setAsking] = useState(false);
  const method = chosen ?? methods[0] ?? null;

  if (pullRequest.state !== "open" || method === null) return null;
  return (
    <>
      <Select value={method} onValueChange={(next) => setChosen(next as MergeMethod)}>
        <SelectTrigger size="sm" aria-label={METHOD_LABEL} className="w-auto">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {methods.map((offered) => (
            <SelectItem key={offered} value={offered}>
              {MERGE_METHOD[offered]}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Button size="sm" disabled={busy} onClick={() => setAsking(true)}>
        <GitMergeIcon aria-hidden />
        {MERGE_LABEL}
      </Button>
      <ConfirmDialog
        open={asking}
        title={CONFIRM_TITLE}
        confirm={CONFIRM_VERB}
        cancel={KEEP_VERB}
        onConfirm={() => {
          setAsking(false);
          onMerge(method);
        }}
        onCancel={() => setAsking(false)}
      >
        {`${MERGE_METHOD[method]}: #${pullRequest.number} goes into ${pullRequest.base}. The service decides whether it may — a required check that has not passed will refuse it.`}
      </ConfirmDialog>
    </>
  );
}
