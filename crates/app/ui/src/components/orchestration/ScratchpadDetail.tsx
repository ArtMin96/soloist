import { Archive, ArchiveRestore, Check, Copy, Download, Link2, Pencil } from "lucide-react";
import {
  DETAIL_DONE_ATTRIBUTE,
  DetailBackButton,
  DetailBody,
  DetailNotice,
  DetailPaneHeader,
  LABEL_FLOOR,
  SQUARE_FLOOR,
} from "@/components/common/DetailPane";
import { DetailActions, type DetailAction } from "@/components/common/DetailActions";
import { LoadableRegion } from "@/components/common/LoadableRegion";
import { TagList } from "@/components/common/TagList";
import { MarkdownSkeleton } from "@/components/editor/MarkdownSkeleton";
import { MarkdownView } from "@/components/editor/MarkdownView";
import { ScratchpadEditor } from "@/components/orchestration/ScratchpadEditor";
import { ScratchpadMeta } from "@/components/orchestration/ScratchpadMeta";
import { ScratchpadTitle } from "@/components/orchestration/ScratchpadTitle";
import { Button } from "@/components/ui/button";
import { humanizeName } from "@/lib/humanize";
import { LoadStatus, type Loadable } from "@/store/loadable";
import type { SaveOutcome } from "@/store/saveOutcome";
import type { ScratchpadConflict } from "@/store/useScratchpadEditor";
import type { ScratchpadSummary, ScratchpadView } from "@/domain";

/** Names which scratchpad the pane is showing — the board's proof it landed on the right one. */
export const SCRATCHPAD_DETAIL_ATTRIBUTE = "data-scratchpad-detail";

/** Names the body read for the loading announcement and the failed read's notice. */
const DOCUMENT_LABEL = "scratchpad";
/** The list this pane returns to, as the back control words it. */
const BACK_DESTINATION = "scratchpads";

/**
 * How much prose the stand-in holds while the body is still being read. The length is unknown until
 * it lands, so this is the height a typical note occupies — enough that the pane does not visibly
 * grow around a short body, and not so much that a long one pushes the reading position.
 */
const BODY_STAND_IN_LINES = 6;

const EMPTY_BODY = "No content yet.";

// The edit surface's state for the open scratchpad, present only while it is being edited. The board
// owns the single edit session and hands it here so the pane swaps its read view for the editor.
export interface ScratchpadEditState {
  /** Bumped on every open and reload so the editor remounts with fresh content and a clean history. */
  mountKey: number;
  conflict: ScratchpadConflict | null;
  error: string | null;
  onSave: (markdown: string) => Promise<SaveOutcome>;
  onReload: () => void;
  /** Commits a rename; rejects with the core's refusal so the field keeps the typed name. */
  onRename: (to: string) => Promise<void>;
  onDone: () => void;
  /** Every keystroke's Markdown, so export and copy hand off text that is not saved yet. */
  onBodyChange: (markdown: string) => void;
}

interface ScratchpadDetailProps {
  /** The listing's record of the open scratchpad — its identity, tags, revision and recency. */
  pad: ScratchpadSummary;
  /** The body read: loading until it lands, failed when there is nothing to fall back on. */
  document: Loadable<ScratchpadView>;
  /** Re-runs a failed read. */
  onRetry: () => void;
  /** The clock reading the meta rail measures recency against. */
  now: number;
  onBack: () => void;
  onStartEdit: () => void;
  /** Archives the open scratchpad, or restores it when already archived. */
  onArchive: () => void;
  onCopyLink: () => void;
  /** Writes the scratchpad to a `.md` file the user picks. */
  onExport: () => void;
  onCopyMarkdown: () => void;
  /** A refusal of something the header just asked for, or null. */
  error: string | null;
  /** Non-null only while this scratchpad is being edited. */
  edit: ScratchpadEditState | null;
}

/**
 * One scratchpad, full width: the pinned header carries its title, its meta rail and its actions,
 * and the document scrolls beneath. Two modes share the pane — the rendered body, and, while
 * editing, the editor in its place with the title band become a rename field.
 *
 * Presentational: every action routes to the core through the board, so the pane never decides
 * whether a write is allowed and never re-derives what the read model already says.
 */
export function ScratchpadDetail({
  pad,
  document,
  onRetry,
  now,
  onBack,
  onStartEdit,
  onArchive,
  onCopyLink,
  onExport,
  onCopyMarkdown,
  error,
  edit,
}: ScratchpadDetailProps) {
  // Every action but the read itself works on the body, so none of them is offered until it is here:
  // an export of a document nobody has read yet would write an empty file.
  const ready = document.status === LoadStatus.Ready;

  return (
    <article
      {...{ [SCRATCHPAD_DETAIL_ATTRIBUTE]: pad.name }}
      className="flex h-full min-h-0 flex-col"
    >
      <DetailPaneHeader
        back={<DetailBackButton destination={BACK_DESTINATION} onClick={onBack} />}
        title={
          edit ? (
            <ScratchpadTitle name={pad.name} onRename={edit.onRename} />
          ) : (
            humanizeName(pad.name)
          )
        }
        meta={
          <div className="flex min-h-6 flex-wrap items-center gap-x-2.5 gap-y-1.5">
            <ScratchpadMeta pad={pad} now={now} />
            <TagList tags={pad.tags} wrap />
          </div>
        }
        actions={
          <DetailActions
            actions={secondaryActions({
              archived: pad.archived,
              disabled: !ready,
              onArchive,
              onCopyLink,
              onExport,
              onCopyMarkdown,
            })}
            primary={
              edit ? (
                // Band 1 stays occupied while editing, so the way out is never below the fold on a
                // long document. `useAutosave` commits on unmount, which this triggers, so leaving
                // needs no flush of its own.
                <Button
                  {...{ [DETAIL_DONE_ATTRIBUTE]: "" }}
                  size="sm"
                  onClick={edit.onDone}
                  className={SQUARE_FLOOR}
                >
                  <Check aria-hidden data-icon="inline-start" />
                  <span className={LABEL_FLOOR}>Done</span>
                </Button>
              ) : (
                <Button size="sm" onClick={onStartEdit} disabled={!ready} className={SQUARE_FLOOR}>
                  <Pencil aria-hidden data-icon="inline-start" />
                  <span className={LABEL_FLOOR}>Edit</span>
                </Button>
              )
            }
            menuTooltip="More scratchpad actions"
          />
        }
      />

      {error && <DetailNotice message={error} />}

      <DetailBody>
        <LoadableRegion
          state={document}
          label={DOCUMENT_LABEL}
          skeleton={<MarkdownSkeleton lines={BODY_STAND_IN_LINES} />}
          onRetry={onRetry}
        >
          {(view) =>
            edit ? (
              <ScratchpadEditor
                key={edit.mountKey}
                name={pad.name}
                initialBody={view.body}
                conflict={edit.conflict}
                error={edit.error}
                onSave={edit.onSave}
                onReload={edit.onReload}
                onBodyChange={edit.onBodyChange}
              />
            ) : view.body ? (
              // The renderer reads its Markdown once, so it is remounted whenever the document
              // itself moves — a write is the only thing that bumps the revision.
              <MarkdownView
                key={`${pad.id}:${view.revision}`}
                markdown={view.body}
                ariaLabel={`${humanizeName(pad.name)} body`}
              />
            ) : (
              <p className="type-body text-muted-foreground">{EMPTY_BODY}</p>
            )
          }
        </LoadableRegion>
      </DetailBody>
    </article>
  );
}

interface SecondaryActionsOptions {
  archived: boolean;
  disabled: boolean;
  onArchive: () => void;
  onCopyLink: () => void;
  onExport: () => void;
  onCopyMarkdown: () => void;
}

// The actions offered in both modes, in one list so read and edit cannot drift into offering
// different sets — the document is the same document either way.
function secondaryActions({
  archived,
  disabled,
  onArchive,
  onCopyLink,
  onExport,
  onCopyMarkdown,
}: SecondaryActionsOptions): DetailAction[] {
  return [
    {
      icon: Link2,
      label: "Copy link to scratchpad",
      menuLabel: "Copy link to scratchpad",
      iconOnly: true,
      onSelect: onCopyLink,
      disabled,
    },
    {
      icon: archived ? ArchiveRestore : Archive,
      label: archived ? "Restore" : "Archive",
      menuLabel: archived ? "Restore scratchpad" : "Archive scratchpad",
      onSelect: onArchive,
      disabled,
    },
    {
      icon: Download,
      label: "Export .md",
      menuLabel: "Export as Markdown",
      onSelect: onExport,
      disabled,
    },
    {
      icon: Copy,
      label: "Copy Markdown",
      menuLabel: "Copy as Markdown",
      onSelect: onCopyMarkdown,
      disabled,
    },
  ];
}
