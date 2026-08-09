import { useState } from "react";
import { GitPullRequestIcon } from "lucide-react";
import { PullRequestForm } from "@/components/git/PullRequestForm";
import { PullRequestSummary } from "@/components/git/PullRequestSummary";
import { SplitMessage, SplitSurface } from "@/components/git/SplitSurface";
import { openExternal } from "@/lib/opener";
import { useAssistTool } from "@/store/git/useAssistTool";
import { usePullRequest } from "@/store/git/usePullRequest";
import type { PullRequestTemplate } from "@/domain";

const PANE_LABEL = "Pull request";

/** What the view says when it cannot offer a form, per state. Each names the one thing that
 *  changes the answer. */
const TOOL_MISSING =
  "Pull requests go through the GitHub command-line tool, and it is not installed. Install `gh` and this opens up.";
const SIGNED_OUT =
  "The GitHub command-line tool is signed in to no account. Run `gh auth login` in a terminal and this opens up.";
const DETACHED = "Nothing is checked out by name, so there is no branch to propose.";

/**
 * The pull-request view of the split surface: what this branch already has on the forge, or the
 * form that proposes it.
 *
 * The one place in the view that reaches the core, so the form and the summary below stay
 * presentational. It holds what is being typed and which shape it was seeded from, and nothing
 * else: whether the forge can be reached, which shape wins, whether the branch has to be pushed
 * first, and whether the project may be acted on are every one of them the core's answers.
 */
export function PullRequestPane({ project, onClose }: { project: number; onClose: () => void }) {
  const { surface, loading, error, proposing, drafting, propose, draft } = usePullRequest(project);
  const assistable = useAssistTool();
  // What has been typed, rather than what is shown: `null` means nobody has touched that field, so
  // it still follows what the read brought. Derived rather than seeded, which is what keeps a
  // status change re-reading the surface from ever overwriting a half-written description — and
  // what keeps the two facts from having to be kept in step at all.
  const [typedTitle, setTypedTitle] = useState<string | null>(null);
  const [typedBase, setTypedBase] = useState<string | null>(null);
  const [typedBody, setTypedBody] = useState<string | null>(null);
  const [asDraft, setAsDraft] = useState(false);
  const [chosen, setChosen] = useState<string | null>(null);

  const templates = surface?.templates ?? [];
  // The shape being filled: the one chosen if a choice was made and is still on offer, otherwise
  // the first the read brought.
  const shape = templates.find((offered) => offered.name === chosen) ?? templates[0] ?? null;
  const title = typedTitle ?? "";
  const base = typedBase ?? surface?.base ?? "";
  const body = typedBody ?? shape?.body ?? "";

  const submit = () => {
    void propose({ title, body, base, draft: asDraft }).then((made) => {
      if (made === null) return;
      // What was proposed is now what the branch has, so the form gives way to the summary the
      // re-read brings — and what was typed into it goes with it rather than lingering behind it.
      setTypedTitle(null);
      setTypedBody(null);
      setAsDraft(false);
      void openExternal(made);
    });
  };

  return (
    <SplitSurface label={PANE_LABEL} title={<PaneTitle />} onClose={onClose}>
      <div className="flex flex-col gap-4 px-4 pt-4">
        {surface?.existing != null && (
          <PullRequestSummary
            pullRequest={surface.existing}
            onOpen={(url) => void openExternal(url)}
          />
        )}
        {error !== null && (
          <p role="alert" className="text-[0.8125rem] text-destructive">
            {error}
          </p>
        )}
      </div>
      <Body
        loading={loading}
        readiness={surface?.readiness ?? null}
        head={surface?.head ?? null}
        proposable={surface?.existing?.state !== "open"}
        form={{
          title,
          base,
          body,
          asDraft,
          template: shape?.name ?? null,
          templates,
          proposing,
          assist: assistable
            ? {
                drafting,
                request: () =>
                  void draft(base, body).then((drafted) => {
                    if (drafted !== null) setTypedBody(drafted);
                  }),
              }
            : null,
        }}
        onTitleChange={setTypedTitle}
        onBaseChange={setTypedBase}
        onBodyChange={setTypedBody}
        onDraftChange={setAsDraft}
        // Choosing a shape gives the description back to it, since choosing one is asking for it.
        onTemplateChange={(name) => {
          setChosen(name);
          setTypedBody(null);
        }}
        onSubmit={submit}
      />
    </SplitSurface>
  );
}

/** What the view is, said once in the split's header. */
function PaneTitle() {
  return (
    <p className="flex min-w-0 flex-1 items-center gap-2 text-[0.8125rem]">
      <GitPullRequestIcon aria-hidden className="size-4 shrink-0 text-muted-foreground" />
      <span className="truncate">{PANE_LABEL}</span>
    </p>
  );
}

/** What the view shows below the summary: a reason it can offer nothing, or the form. */
function Body({
  loading,
  readiness,
  head,
  proposable,
  form,
  onTitleChange,
  onBaseChange,
  onBodyChange,
  onDraftChange,
  onTemplateChange,
  onSubmit,
}: {
  loading: boolean;
  readiness: "missing" | "logged_out" | "ready" | null;
  head: string | null;
  /** Whether a new one is worth offering — a branch whose pull request is open already has one. */
  proposable: boolean;
  form: {
    title: string;
    base: string;
    body: string;
    asDraft: boolean;
    template: string | null;
    templates: PullRequestTemplate[];
    proposing: boolean;
    assist: { drafting: boolean; request: () => void } | null;
  };
  onTitleChange: (title: string) => void;
  onBaseChange: (base: string) => void;
  onBodyChange: (body: string) => void;
  onDraftChange: (draft: boolean) => void;
  onTemplateChange: (name: string) => void;
  onSubmit: () => void;
}) {
  // Nothing at all until the first read has answered, rather than a claim while it is still being
  // fetched.
  if (loading || readiness === null) return null;
  if (readiness === "missing") return <SplitMessage>{TOOL_MISSING}</SplitMessage>;
  if (readiness === "logged_out") return <SplitMessage>{SIGNED_OUT}</SplitMessage>;
  if (head === null) return <SplitMessage>{DETACHED}</SplitMessage>;
  if (!proposable) return null;
  return (
    <PullRequestForm
      head={head}
      title={form.title}
      base={form.base}
      body={form.body}
      draft={form.asDraft}
      templates={form.templates}
      template={form.template}
      busy={form.proposing}
      assist={form.assist}
      onTitleChange={onTitleChange}
      onBaseChange={onBaseChange}
      onBodyChange={onBodyChange}
      onDraftChange={onDraftChange}
      onTemplateChange={onTemplateChange}
      onSubmit={onSubmit}
    />
  );
}
