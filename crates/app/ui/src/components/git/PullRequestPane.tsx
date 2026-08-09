import { useState, type ReactNode } from "react";
import { GitPullRequestIcon } from "lucide-react";
import { PullRequestForm } from "@/components/git/PullRequestForm";
import { PullRequestProposal } from "@/components/git/PullRequestProposal";
import { PullRequestReviewView } from "@/components/git/PullRequestReviewView";
import { PullRequestSummary } from "@/components/git/PullRequestSummary";
import { SplitMessage, SplitSurface } from "@/components/git/SplitSurface";
import { openExternal } from "@/lib/opener";
import { useAssistTool } from "@/store/git/useAssistTool";
import { usePullRequest } from "@/store/git/usePullRequest";
import type { NewPullRequest, PullRequestSuggestion, PullRequestTemplate } from "@/domain";

const PANE_LABEL = "Pull request";

/** What the view says when it cannot offer a proposal, per state. Each names what is true, and then
 *  the one thing that changes it. */
const LOADING = "Reading what this branch has on the forge…";
const TOOL_MISSING =
  "Pull requests go through the GitHub command-line tool, and it is not installed. Install `gh` and this opens up.";
const SIGNED_OUT =
  "The GitHub command-line tool is installed but signed in to no account. Run `gh auth login` in a terminal and this opens up.";
const DETACHED =
  "Nothing is checked out by name — the working tree is on a commit rather than a branch. Check a branch out and this opens up.";
const ALREADY_OPEN =
  "This branch already has a pull request open, shown above. A second one would propose the same commits.";

/**
 * The pull-request view of the split surface: what this branch already has on the forge, the
 * proposal it would make, or the form that edits one before it is made.
 *
 * The one place in the view that reaches the core, so the proposal, the form and the summary below
 * stay presentational. It holds what is being typed, which shape it was seeded from, and whether the
 * reader asked to edit the details; whether the forge can be reached, which shape wins, what the
 * branch would be proposed as, whether it has to be pushed first, and whether the project may be
 * acted on are every one of them the core's answers.
 */
export function PullRequestPane({
  project,
  agent,
  onClose,
}: {
  project: number;
  /**
   * The agent a handoff would reach, or null to let the core pick the project's only running one.
   * Which process the reader is looking at is the one fact the core cannot know, so it is the one
   * thing that is passed down.
   */
  agent: number | null;
  onClose: () => void;
}) {
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
  // Whether the reader asked to see the fields. Proposing what the branch already says is a press;
  // this is the door to the same proposal with every word of it editable, and it stays shut until
  // somebody opens it.
  const [editing, setEditing] = useState(false);

  const templates = surface?.templates ?? [];
  // The shape being filled: the one chosen if a choice was made and is still on offer, otherwise
  // the first the read brought.
  const shape = templates.find((offered) => offered.name === chosen) ?? templates[0] ?? null;
  const suggestion = surface?.suggestion ?? null;
  // The suggested description was written into the first shape on offer, so it is the description
  // only while that is still the shape in play. Choosing another asks for that one instead, and
  // asking for it is asking for it empty.
  const suggested = shape === (templates[0] ?? null) ? suggestion : null;
  const title = typedTitle ?? suggestion?.title ?? "";
  const base = typedBase ?? surface?.base ?? "";
  const body = typedBody ?? suggested?.body ?? shape?.body ?? "";

  // Nothing is proposed without a press, and what a press sends is what the surface it came from
  // showed: the suggestion as it stands, or the fields as they were edited.
  const send = (request: NewPullRequest) => {
    void propose(request).then((made) => {
      if (made === null) return;
      // What was proposed is now what the branch has, so the proposal gives way to the summary the
      // re-read brings — and what was typed into it goes with it rather than lingering behind it.
      setTypedTitle(null);
      setTypedBody(null);
      setAsDraft(false);
      setEditing(false);
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
          <p role="alert" className="type-body text-destructive">
            {error}
          </p>
        )}
      </div>
      {surface?.existing != null && (
        <PullRequestReviewView project={project} agent={agent} methods={surface.merge_methods} />
      )}
      <Body
        loading={loading}
        // A read that came back refused is not still running, whatever it left `loading` saying.
        failed={error !== null && surface === null}
        readiness={surface?.readiness ?? null}
        head={surface?.head ?? null}
        proposable={surface?.existing?.state !== "open"}
        editing={editing}
        proposal={{
          base: surface?.base ?? null,
          suggestion,
          templated: templates.length > 0,
          busy: proposing,
        }}
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
        onProposeSuggestion={(offered, into) =>
          send({ title: offered.title, body: offered.body, base: into, draft: false })
        }
        onEdit={() => setEditing(true)}
        onTitleChange={setTypedTitle}
        onBaseChange={setTypedBase}
        onBodyChange={setTypedBody}
        onDraftChange={setAsDraft}
        // Choosing a shape gives the description back to it, since choosing one is asking for it.
        onTemplateChange={(name) => {
          setChosen(name);
          setTypedBody(null);
        }}
        onSubmit={() => send({ title, body, base, draft: asDraft })}
      />
    </SplitSurface>
  );
}

/** What the view is, said once in the split's header. */
function PaneTitle() {
  return (
    <p className="flex min-w-0 flex-1 items-center gap-2 type-body">
      <GitPullRequestIcon aria-hidden className="size-4 shrink-0 text-muted-foreground" />
      <span className="truncate">{PANE_LABEL}</span>
    </p>
  );
}

/** A quiet statement in the body's own rhythm, where what it is about is already on screen above. */
function PaneNote({ children }: { children: ReactNode }) {
  return <p className="max-w-[70ch] px-4 pb-4 type-body text-muted-foreground">{children}</p>;
}

/** What the view shows below the summary: a reason it can offer nothing, the proposal, or the form. */
function Body({
  loading,
  failed,
  readiness,
  head,
  proposable,
  editing,
  proposal,
  form,
  onProposeSuggestion,
  onEdit,
  onTitleChange,
  onBaseChange,
  onBodyChange,
  onDraftChange,
  onTemplateChange,
  onSubmit,
}: {
  loading: boolean;
  /** Whether the read came back refused with nothing to show, which the reason above already says. */
  failed: boolean;
  readiness: "missing" | "logged_out" | "ready" | null;
  head: string | null;
  /** Whether a new one is worth offering — a branch whose pull request is open already has one. */
  proposable: boolean;
  /** Whether the reader asked for the fields rather than the press. */
  editing: boolean;
  proposal: {
    base: string | null;
    suggestion: PullRequestSuggestion | null;
    templated: boolean;
    busy: boolean;
  };
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
  onProposeSuggestion: (suggestion: PullRequestSuggestion, base: string) => void;
  onEdit: () => void;
  onTitleChange: (title: string) => void;
  onBaseChange: (base: string) => void;
  onBodyChange: (body: string) => void;
  onDraftChange: (draft: boolean) => void;
  onTemplateChange: (name: string) => void;
  onSubmit: () => void;
}) {
  // The refusal is stated above this, and it is the whole answer: a surface that went on saying it
  // was reading would be claiming something is still coming that nothing will bring.
  if (failed) return null;
  // Says it is still reading rather than claiming anything about the branch, and rather than
  // showing nothing at all — a blank half of the split reads as a surface that failed.
  if (loading || readiness === null) return <SplitMessage>{LOADING}</SplitMessage>;
  if (readiness === "missing") return <SplitMessage>{TOOL_MISSING}</SplitMessage>;
  if (readiness === "logged_out") return <SplitMessage>{SIGNED_OUT}</SplitMessage>;
  if (head === null) return <SplitMessage>{DETACHED}</SplitMessage>;
  // Said in a line rather than a centred panel: what it is about is the summary already on screen
  // above it, and this only answers why no proposal is offered under it.
  if (!proposable) return <PaneNote>{ALREADY_OPEN}</PaneNote>;
  if (!editing) {
    return (
      <PullRequestProposal
        head={head}
        base={proposal.base}
        suggestion={proposal.suggestion}
        templated={proposal.templated}
        busy={proposal.busy}
        onPropose={() => {
          if (proposal.suggestion !== null && proposal.base !== null) {
            onProposeSuggestion(proposal.suggestion, proposal.base);
          }
        }}
        onEdit={onEdit}
      />
    );
  }
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
