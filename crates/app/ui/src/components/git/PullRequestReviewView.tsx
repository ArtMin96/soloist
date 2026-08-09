import { useState } from "react";
import { RefreshCwIcon } from "lucide-react";
import { CheckList } from "@/components/git/CheckList";
import { HandoffNotice } from "@/components/git/HandoffNotice";
import { MergeAction } from "@/components/git/MergeAction";
import { ReviewThreadList } from "@/components/git/ReviewThreadList";
import { Button } from "@/components/ui/button";
import { writeClipboard } from "@/lib/clipboard";
import { openExternal } from "@/lib/opener";
import { usePullRequestReview } from "@/store/git/usePullRequestReview";
import type { Handoff, MergeMethod } from "@/domain";

const CHECKS_HEADING = "Checks";
const CONVERSATION_HEADING = "Conversation";
const REFRESH_LABEL = "Read it again now";
const DELIVERED = "Context is in the agent's session, unsent.";

/**
 * What an open pull request looks like: what the service's checks say, what people have written,
 * and putting it into its base branch.
 *
 * The one place in the view that reaches the core, so the three lists below stay presentational.
 * It holds what is being looked at and what the last handoff came back as, and nothing else —
 * which check states exist, which agent a handoff reaches, what it says, and whether a merge is
 * allowed are every one of them the core's answers.
 */
export function PullRequestReviewView({
  project,
  agent,
  methods,
}: {
  project: number;
  /**
   * The agent to hand context to, or null to let the core pick the project's only running one.
   * Which process the reader is looking at is the one fact the core cannot know, so it is the one
   * thing this passes.
   */
  agent: number | null;
  /** The ways this repository allows a merge, read with the surface. */
  methods: MergeMethod[];
}) {
  const { review, error, merging, refresh, merge, handOff } = usePullRequestReview(project);
  const [handoff, setHandoff] = useState<Handoff | null>(null);

  if (review === null) return null;
  const deliver = (subject: Parameters<typeof handOff>[0]) => {
    void handOff(subject, agent).then(setHandoff);
  };
  return (
    <div className="flex flex-col gap-4 px-4 pb-4">
      <div className="flex items-center gap-2">
        <p className="min-w-0 flex-1 truncate type-label text-muted-foreground">
          {delivered(handoff)}
        </p>
        <Button variant="ghost" size="sm" onClick={refresh}>
          <RefreshCwIcon aria-hidden />
          {REFRESH_LABEL}
        </Button>
        <MergeAction
          pullRequest={review.pull_request}
          methods={methods}
          busy={merging}
          onMerge={(method: MergeMethod) => void merge(review.pull_request.number, method)}
        />
      </div>
      {error !== null && (
        <p role="alert" className="text-[0.8125rem] text-destructive">
          {error}
        </p>
      )}
      <Section heading={CHECKS_HEADING} count={review.checks.length}>
        <CheckList
          checks={review.checks}
          onHandOff={(check) => deliver({ kind: "check", name: check.name })}
          onOpen={openOnForge}
        />
      </Section>
      <Section heading={CONVERSATION_HEADING} count={review.threads.length}>
        <ReviewThreadList
          threads={review.threads}
          onHandOff={(thread) => deliver({ kind: "thread", id: thread.id })}
          onOpen={openOnForge}
        />
      </Section>
      <HandoffNotice
        handoff={handoff}
        onCopy={(text) => {
          void writeClipboard(text);
          setHandoff(null);
        }}
        onDismiss={() => setHandoff(null)}
      />
    </div>
  );
}

/** What the last handoff came back as, said once and quietly rather than as an interruption — and
 *  saying outright that nothing was submitted, since that is the whole contract. */
function delivered(handoff: Handoff | null): string {
  return handoff?.delivery === "delivered" ? DELIVERED : "";
}

/** A quiet sentence-case heading with the count beside it, in the app's grouped-list idiom. */
function Section({
  heading,
  count,
  children,
}: {
  heading: string;
  count: number;
  children: React.ReactNode;
}) {
  return (
    <section className="flex flex-col gap-1">
      <h3 className="type-label text-muted-foreground">{`${heading} · ${count}`}</h3>
      {children}
    </section>
  );
}

/** Opening anything on the service is the desktop's job, never the webview's. */
function openOnForge(url: string): void {
  void openExternal(url);
}
