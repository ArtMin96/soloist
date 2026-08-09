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
import {
  BASE_LABEL,
  BODY_LABEL,
  PROPOSE_LABEL,
  PROPOSING_LABEL,
  TITLE_LABEL,
} from "@/components/git/pullRequestCopy";
import { ASSIST_SETTINGS_TAB } from "@/components/settings/tabs";
import { ASSIST_SETUP_HINT } from "@/lib/agents";
import { useOpenSettings } from "@/store/settingsContext";
import type { PullRequestTemplate } from "@/domain";

const TITLE_PLACEHOLDER = "What this proposes";
const BODY_PLACEHOLDER = "What it changes, and why";
const TEMPLATE_LABEL = "Starting shape";
const DRAFT_LABEL = "Open as a draft";
const DRAFT_HINT = "Propose it without asking for review yet";
const ASSIST_LABEL = "Draft a description";
const ASSIST_HINT = "Fill this shape in with your assist tool, to edit before proposing";
/** The same control where no tool is picked yet. The ellipsis is the promise it keeps: it leads
 *  somewhere before it drafts anything. */
const SETUP_LABEL = "Draft a description…";
const DRAFTING = "Drafting a description…";

/**
 * What a new pull request is asked for with: what it proposes, where it is going, and the
 * description — seeded from whichever shape the repository or the user brought, and the user's to
 * edit from there.
 *
 * A drafted description lands in the box like any other and is proposed by the same button,
 * because it is a draft: nothing here treats it as more finished than what somebody typed.
 *
 * Presentational: props in, callbacks out, plus the shell's own action for reaching Settings — the
 * one setting that switches drafting on is best reached from the surface that wants it. Whether the
 * branch has to be pushed first, and whether the project may be acted on at all, are the core's
 * answers.
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
  const openSettings = useOpenSettings();
  const drafting = assist?.drafting === true;
  const ready = title.trim() !== "" && base.trim() !== "";
  // Present whether or not a tool is picked, on the same terms the commit box offers it: a control
  // nobody can find is a feature nobody has, and where none is picked the press takes the reader to
  // the one setting that changes that — a real action, named as one. Disabled stays for the live
  // control while a run is in flight or there is nothing to compare against.
  const drafter =
    assist === null
      ? {
          label: SETUP_LABEL,
          hint: ASSIST_SETUP_HINT,
          unavailable: false,
          act: () => openSettings(ASSIST_SETTINGS_TAB),
        }
      : {
          label: ASSIST_LABEL,
          hint: ASSIST_HINT,
          unavailable: drafting || busy || base.trim() === "",
          act: assist.request,
        };

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
        <p className="min-w-0 flex-1 truncate pb-2 font-mono type-body text-muted-foreground">
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
        <Tooltip>
          <TooltipTrigger asChild>
            <span className="flex items-center gap-2">
              <Checkbox
                id={draftId}
                checked={draft}
                onCheckedChange={(checked) => onDraftChange(checked === true)}
              />
              <label htmlFor={draftId} className="type-body">
                {DRAFT_LABEL}
              </label>
            </span>
          </TooltipTrigger>
          <TooltipContent>{DRAFT_HINT}</TooltipContent>
        </Tooltip>
        {drafting && (
          <p className="min-w-0 flex-1 truncate type-label text-muted-foreground">{DRAFTING}</p>
        )}
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon-xs"
              className="ms-auto"
              aria-label={drafter.label}
              disabled={drafter.unavailable}
              onClick={drafter.act}
            >
              <SparklesIcon className={drafting ? "motion-safe:animate-pulse" : undefined} />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{drafter.hint}</TooltipContent>
        </Tooltip>
        <Button size="sm" disabled={!ready || busy || drafting} onClick={onSubmit}>
          {busy ? PROPOSING_LABEL : PROPOSE_LABEL}
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
