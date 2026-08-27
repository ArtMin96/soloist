import { useId, useState } from "react";
import { ChevronDownIcon, SparklesIcon } from "lucide-react";
import { ASSIST_SETTINGS_TAB } from "@/components/settings/tabs";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { GLASS_CONTROL_SURFACE } from "@/components/ui/glass";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ASSIST_SETUP_HINT } from "@/lib/agents";
import { cn } from "@/lib/utils";
import { useOpenSettings } from "@/store/settingsContext";
import type { FileChange } from "@/domain";

const PLACEHOLDER = "Message";
const COMMIT_LABEL = "Commit";
const AMEND_LABEL = "Amend";
const AMEND_HINT = "Replace the last commit instead of adding one";
const DRAFT_LABEL = "Draft";
const DRAFT_HINT = "Describe the staged change with your assist tool, to edit before committing";
const COMPOSER_LABEL = "Commit changes";
/** The same control where no tool is picked yet, which is where every install starts. The ellipsis
 *  is the promise it keeps: it leads somewhere before it drafts anything. */
const SETUP_LABEL = "Draft…";

/** What the box says about what a press would record, one state at a time. */
const DRAFTING = "Drafting a message…";
const AMENDING = "Amending the last commit";
const NOTHING_STAGED = "Nothing is staged to commit";

/** …and how much there is to record, agreeing with the count. */
function stagedFiles(staged: number): string {
  return staged === 1 ? "1 file staged" : `${staged} files staged`;
}

function stagedSummary(staged: number): string {
  return staged === 0 ? NOTHING_STAGED : `${staged} staged`;
}

/**
 * The message, and what to do with it. Whether a commit is allowed is the core's answer — this
 * only declines to ask a question whose answer is already known: an empty message, and a first
 * commit with nothing to record. Amending needs neither, because it is how a message is
 * corrected.
 *
 * A drafted message lands in the box like any other and is committed by the same button, because
 * it is a draft: nothing here treats it as more finished than what somebody typed.
 *
 * The repository's own template is followed rather than copied in: until somebody types, the box
 * *is* the template, so one arriving after the box was rendered shows up and one that changed
 * cannot be left stale by state that was seeded once. Typing takes the box over from it, and a
 * recorded commit hands it back — which is what makes the next commit start from the template
 * again.
 *
 * Presentational: it holds the message being typed and nothing else.
 */
export function CommitBox({
  changes,
  busy,
  template,
  draft,
  onCommit,
}: {
  changes: FileChange[];
  busy: boolean;
  /** What a message starts as where the repository configures one, or `null` where it does not. */
  template: string | null;
  /** Asking for a message, or `null` when no tool is configured to draft one. */
  draft: { drafting: boolean; request: () => Promise<string | null> } | null;
  /** Resolves true when the commit was recorded, which is when the message is cleared. */
  onCommit: (message: string, amend: boolean) => Promise<boolean>;
}) {
  const amendId = useId();
  const openSettings = useOpenSettings();
  const [open, setOpen] = useState(false);
  const [typed, setTyped] = useState<string | null>(null);
  const [amend, setAmend] = useState(false);
  const message = typed ?? template ?? "";
  const staged = changes.filter((change) => change.status.staged !== null).length;
  const ready = message.trim() !== "" && (amend || staged > 0);
  const drafting = draft?.drafting === true;

  const commit = () => {
    void onCommit(message, amend).then((recorded) => {
      if (recorded) {
        setTyped(null);
        setAmend(false);
      }
    });
  };

  const requestDraft = () => {
    void draft?.request().then((drafted) => {
      if (drafted !== null) setTyped(drafted);
    });
  };

  // Present whether or not a tool is picked, because a control nobody can find is a feature nobody
  // has. Not disabled either: what it does where none is picked is take the reader to the one
  // setting that changes that, which is a real action and is named as one. The pull request's form
  // offers the same door on the same terms.
  const assist =
    draft === null
      ? {
          label: SETUP_LABEL,
          hint: ASSIST_SETUP_HINT,
          unavailable: false,
          act: () => openSettings(ASSIST_SETTINGS_TAB),
        }
      : {
          label: DRAFT_LABEL,
          hint: DRAFT_HINT,
          unavailable: staged === 0 || drafting || busy,
          act: requestDraft,
        };

  const disclosureLabel = open ? "Hide commit composer" : `Show commit composer: ${COMPOSER_LABEL}`;

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="shrink-0 border-t border-sidebar-border"
    >
      <CollapsibleTrigger asChild>
        <Button
          variant="ghost"
          aria-label={disclosureLabel}
          className="h-auto w-full justify-start rounded-none px-3 py-2.5 text-start"
        >
          <span className="flex min-w-0 flex-1 flex-col items-start gap-0.5">
            <span className="type-body font-medium text-foreground">{COMPOSER_LABEL}</span>
            {!open && (
              <span className="type-label whitespace-normal text-muted-foreground">
                {stagedSummary(staged)}
              </span>
            )}
          </span>
          <ChevronDownIcon
            aria-hidden
            className={cn(
              "ms-auto transition-transform duration-[var(--dur-control)] ease-spring-settle motion-reduce:transition-none",
              open && "rotate-180",
            )}
          />
        </Button>
      </CollapsibleTrigger>
      <CollapsibleContent className="flex flex-col gap-2 px-3 pb-3">
        <Textarea
          value={message}
          aria-label="Commit message"
          placeholder={PLACEHOLDER}
          rows={4}
          className={cn("min-h-24 max-h-[40vh] resize-y overflow-y-auto", GLASS_CONTROL_SURFACE)}
          onChange={(event) => setTyped(event.target.value)}
        />
        <p
          aria-live="polite"
          className="type-label whitespace-normal leading-snug text-muted-foreground"
        >
          {recording({ drafting, amend, staged })}
        </p>
        <div className="flex items-center gap-2">
          <Tooltip>
            <TooltipTrigger asChild>
              <span className="flex items-center gap-2">
                <Checkbox
                  id={amendId}
                  checked={amend}
                  onCheckedChange={(checked) => setAmend(checked === true)}
                />
                <Label htmlFor={amendId} className="type-body font-normal">
                  {AMEND_LABEL}
                </Label>
              </span>
            </TooltipTrigger>
            <TooltipContent>{AMEND_HINT}</TooltipContent>
          </Tooltip>
          <div className="ms-auto flex items-center gap-1.5">
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={assist.unavailable}
                  onClick={assist.act}
                >
                  <SparklesIcon className={drafting ? "motion-safe:animate-pulse" : undefined} />
                  {assist.label}
                </Button>
              </TooltipTrigger>
              <TooltipContent>{assist.hint}</TooltipContent>
            </Tooltip>
            <Button size="sm" disabled={!ready || busy || drafting} onClick={commit}>
              {COMMIT_LABEL}
            </Button>
          </div>
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}

/** What a press would record, said in one line: the draft being written, the commit being replaced,
 *  or how much is staged — including the honest nothing. */
function recording({
  drafting,
  amend,
  staged,
}: {
  drafting: boolean;
  amend: boolean;
  staged: number;
}): string {
  if (drafting) return DRAFTING;
  if (amend) return AMENDING;
  if (staged === 0) return NOTHING_STAGED;
  return stagedFiles(staged);
}
