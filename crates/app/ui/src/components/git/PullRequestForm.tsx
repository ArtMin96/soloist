import { useId } from "react";
import { SparklesIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { PullRequestTemplate } from "@/domain";

const TITLE_LABEL = "Title";
const TITLE_PLACEHOLDER = "What this proposes";
const BASE_LABEL = "Merge into";
const BODY_LABEL = "Description";
const BODY_PLACEHOLDER = "What it changes, and why";
const TEMPLATE_LABEL = "Starting shape";
const DRAFT_LABEL = "Open as a draft";
const DRAFT_HINT = "Propose it without asking for review yet";
const SUBMIT_LABEL = "Open pull request";
const SUBMITTING_LABEL = "Opening…";
const ASSIST_LABEL = "Draft a description";
const ASSIST_HINT = "Fill this shape in with your assist tool, to edit before proposing";
const DRAFTING = "Drafting a description…";

/**
 * What a new pull request is asked for with: what it proposes, where it is going, and the
 * description — seeded from whichever shape the repository or the user brought, and the user's to
 * edit from there.
 *
 * A drafted description lands in the box like any other and is proposed by the same button,
 * because it is a draft: nothing here treats it as more finished than what somebody typed.
 *
 * Presentational: props in, callbacks out. Whether the branch has to be pushed first, and whether
 * the project may be acted on at all, are the core's answers.
 */
export function PullRequestForm({
  head,
  title,
  base,
  body,
  draft,
  templates,
  template,
  busy,
  assist,
  onTitleChange,
  onBaseChange,
  onBodyChange,
  onDraftChange,
  onTemplateChange,
  onSubmit,
}: {
  head: string;
  title: string;
  base: string;
  body: string;
  draft: boolean;
  /** The shapes on offer; a choice is presented only where there is more than one. */
  templates: PullRequestTemplate[];
  /** Which of them the description was last seeded from. */
  template: string | null;
  busy: boolean;
  /** Asking for a description, or `null` when no tool is configured to draft one. */
  assist: { drafting: boolean; request: () => void } | null;
  onTitleChange: (title: string) => void;
  onBaseChange: (base: string) => void;
  onBodyChange: (body: string) => void;
  onDraftChange: (draft: boolean) => void;
  onTemplateChange: (name: string) => void;
  onSubmit: () => void;
}) {
  const titleId = useId();
  const baseId = useId();
  const bodyId = useId();
  const templateId = useId();
  const draftId = useId();
  const drafting = assist?.drafting === true;
  const ready = title.trim() !== "" && base.trim() !== "";

  return (
    <div className="flex flex-col gap-4 p-4">
      <div className="flex flex-col gap-1.5">
        <FieldLabel htmlFor={titleId}>{TITLE_LABEL}</FieldLabel>
        <Input
          id={titleId}
          value={title}
          placeholder={TITLE_PLACEHOLDER}
          onChange={(event) => onTitleChange(event.target.value)}
        />
      </div>

      <div className="flex flex-wrap items-end gap-4">
        <div className="flex min-w-0 flex-1 flex-col gap-1.5">
          <FieldLabel htmlFor={baseId}>{BASE_LABEL}</FieldLabel>
          <Input
            id={baseId}
            value={base}
            className="font-mono"
            onChange={(event) => onBaseChange(event.target.value)}
          />
        </div>
        <p className="min-w-0 flex-1 truncate pb-2 font-mono text-[0.8125rem] text-muted-foreground">
          {`← ${head}`}
        </p>
      </div>

      {templates.length > 1 && (
        <div className="flex flex-col gap-1.5">
          <FieldLabel htmlFor={templateId}>{TEMPLATE_LABEL}</FieldLabel>
          <Select value={template ?? undefined} onValueChange={onTemplateChange}>
            <SelectTrigger id={templateId} className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {templates.map((offered) => (
                <SelectItem key={offered.name} value={offered.name}>
                  {offered.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      )}

      <div className="flex flex-col gap-1.5">
        <FieldLabel htmlFor={bodyId}>{BODY_LABEL}</FieldLabel>
        <Textarea
          id={bodyId}
          value={body}
          placeholder={BODY_PLACEHOLDER}
          rows={12}
          className="resize-y font-mono"
          onChange={(event) => onBodyChange(event.target.value)}
        />
      </div>

      <div className="flex items-center gap-2">
        <Checkbox
          id={draftId}
          checked={draft}
          onCheckedChange={(checked) => onDraftChange(checked === true)}
        />
        <label htmlFor={draftId} className="text-[0.8125rem]" title={DRAFT_HINT}>
          {DRAFT_LABEL}
        </label>
        {drafting && (
          <p className="min-w-0 flex-1 truncate type-label text-muted-foreground">{DRAFTING}</p>
        )}
        {/* Absent rather than disabled where no tool is configured: an action nobody may take is
            not an action. Disabled is kept for the one that is momentarily pending. */}
        {assist !== null && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-xs"
                className="ms-auto"
                aria-label={ASSIST_LABEL}
                disabled={drafting || busy || base.trim() === ""}
                onClick={assist.request}
              >
                <SparklesIcon className={drafting ? "motion-safe:animate-pulse" : undefined} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{ASSIST_HINT}</TooltipContent>
          </Tooltip>
        )}
        <Button
          size="sm"
          className={assist === null ? "ms-auto" : undefined}
          disabled={!ready || busy || drafting}
          onClick={onSubmit}
        >
          {busy ? SUBMITTING_LABEL : SUBMIT_LABEL}
        </Button>
      </div>
    </div>
  );
}

/** A field's quiet sentence-case label, sitting above its control. */
function FieldLabel({ htmlFor, children }: { htmlFor: string; children: string }) {
  return (
    <label htmlFor={htmlFor} className="type-label text-muted-foreground">
      {children}
    </label>
  );
}
