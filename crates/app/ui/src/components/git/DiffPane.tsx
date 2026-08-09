import { useState } from "react";
import { Columns2Icon, ExternalLinkIcon, Rows2Icon } from "lucide-react";
import { DiffViewer, SIDE_BY_SIDE, UNIFIED, type DiffLayout } from "@/components/git/DiffViewer";
import { DiscardDialog } from "@/components/git/DiscardDialog";
import { FilePreview } from "@/components/git/FilePreview";
import { HunkActions } from "@/components/git/HunkActions";
import {
  SplitButton,
  SplitMessage,
  SplitNotice,
  SplitSurface,
} from "@/components/git/SplitSurface";
import { SegmentedControl } from "@/components/SegmentedControl";
import { Button } from "@/components/ui/button";
import type { Option } from "@/lib/appearance";
import { useAppearance } from "@/store/appearanceContext";
import { CHANGE, type DiffSelection } from "@/store/git/useDiffSelection";
import { useFilePreview } from "@/store/git/useFilePreview";
import { useGitDiff } from "@/store/git/useGitDiff";
import { useGitWrite } from "@/store/git/useGitWrite";
import { hunkKey } from "@/lib/git";
import type { DiffTarget, HunkRange } from "@/domain";

const PANE_LABEL = "Diff";
const SIDE_BY_SIDE_LABEL = "Show the two sides side by side";
const UNIFIED_LABEL = "Show the two sides in one column";
const LOAD_FULL_LABEL = "Load the whole diff";
const OPEN_LABEL = "Open in the default application";

/** What the split says when there is nothing in it to render, per state. */
const NOT_A_REPOSITORY = "Not a git repository";
const NOTHING_TO_SHOW = "No changes at this comparison";
const BINARY = "This file holds bytes rather than text, so there is nothing to show";
const TRUNCATED = "Showing the first part of this diff";
const GONE = "This file is no longer there";
const PREVIEW_TRUNCATED = "Showing the beginning of this file";

/** The comparisons a reader can choose between. An untracked path has only one, so the choice
 *  is not offered for it. */
const STAGED: DiffTarget = "staged";
const UNSTAGED: DiffTarget = "unstaged";
const HEAD: DiffTarget = "head";
const UNTRACKED: DiffTarget = "untracked";

const TARGET_OPTIONS: Option<DiffTarget>[] = [
  { value: UNSTAGED, label: "Working" },
  { value: STAGED, label: "Staged" },
  { value: HEAD, label: "HEAD" },
];

/**
 * The diff view of the split surface, so a change is read beside a working agent rather than in
 * place of it.
 *
 * The one place in the view that reaches the core, so the viewer and the preview below stay
 * presentational.
 */
export function DiffPane({
  project,
  selection,
  onClose,
}: {
  project: number;
  selection: DiffSelection;
  onClose: () => void;
}) {
  const [split, setSplit] = useState<DiffLayout>(SIDE_BY_SIDE);
  const [target, setTarget] = useState<DiffTarget>(UNSTAGED);
  const { dark } = useAppearance();
  const showing = selection.kind === CHANGE;
  const {
    diff,
    loading: diffLoading,
    loadFull,
  } = useGitDiff(showing ? project : null, showing ? selection.path : null, target);
  const { content, loading: contentLoading } = useFilePreview(
    showing ? null : project,
    showing ? null : selection.path,
  );
  const write = useGitWrite(project);
  const [discarding, setDiscarding] = useState<HunkRange | null>(null);

  const untracked = diff?.target === UNTRACKED;
  return (
    <SplitSurface
      label={PANE_LABEL}
      title={
        <p className="min-w-0 flex-1 truncate font-mono text-[0.8125rem]" title={selection.path}>
          {selection.path}
        </p>
      }
      controls={
        <>
          {showing && !untracked && (
            <SegmentedControl<DiffTarget>
              value={target}
              options={TARGET_OPTIONS}
              onChange={setTarget}
              ariaLabel="Comparison"
            />
          )}
          {showing && (
            <SplitButton
              label={split === SIDE_BY_SIDE ? UNIFIED_LABEL : SIDE_BY_SIDE_LABEL}
              icon={split === SIDE_BY_SIDE ? <Rows2Icon /> : <Columns2Icon />}
              onClick={() => setSplit(split === SIDE_BY_SIDE ? UNIFIED : SIDE_BY_SIDE)}
            />
          )}
          {/* Absent rather than disabled until the project is trusted: what this starts is a
              program the desktop picks from the file's own name, which the core refuses to do on
              a project the user has not authorised Soloist to act within. */}
          {write.trusted === true && (
            <SplitButton
              label={OPEN_LABEL}
              icon={<ExternalLinkIcon />}
              onClick={() => write.open(selection.path)}
            />
          )}
        </>
      }
      notices={
        <>
          {diff?.truncated === true && (
            <SplitNotice
              action={
                <Button size="sm" onClick={loadFull}>
                  {LOAD_FULL_LABEL}
                </Button>
              }
            >
              {TRUNCATED}
            </SplitNotice>
          )}
          {content?.truncated === true && <SplitNotice>{PREVIEW_TRUNCATED}</SplitNotice>}
          {/* One place a refused action is stated, whichever asked for it — the same shape the
              rail uses. Cleared when the next action starts, so it never outlives what it is
              about. */}
          {write.error !== null && <SplitNotice>{write.error}</SplitNotice>}
        </>
      }
      onClose={onClose}
    >
      {showing ? (
        diff === null ? (
          <SplitMessage>{diffLoading ? "" : NOT_A_REPOSITORY}</SplitMessage>
        ) : diff.binary ? (
          <SplitMessage>{BINARY}</SplitMessage>
        ) : diff.patch === "" ? (
          <SplitMessage>{NOTHING_TO_SHOW}</SplitMessage>
        ) : (
          <DiffViewer
            diff={diff}
            layout={split}
            dark={dark}
            actions={
              // A hunk can only be acted on where there is something to act on: an untracked
              // path is not in the index yet, and a project that has not been trusted may not
              // be changed at all. Both are absent rather than disabled.
              write.trusted === true && !untracked
                ? (hunk: HunkRange) => (
                    <HunkActions
                      hunk={hunk}
                      staged={target === STAGED}
                      busy={write.busy(hunkKey(selection.path, hunk))}
                      onStage={(it) => write.stageHunk(selection.path, it)}
                      onUnstage={(it) => write.unstageHunk(selection.path, it)}
                      onDiscard={setDiscarding}
                    />
                  )
                : undefined
            }
          />
        )
      ) : content === null ? (
        <SplitMessage>{contentLoading ? "" : GONE}</SplitMessage>
      ) : content.text === null ? (
        <SplitMessage>{BINARY}</SplitMessage>
      ) : (
        <FilePreview path={selection.path} content={content} dark={dark} />
      )}
      <DiscardDialog
        discarding={discarding === null ? null : { path: selection.path, hunk: true }}
        onCancel={() => setDiscarding(null)}
        onConfirm={() => {
          if (discarding !== null) write.discardHunk(selection.path, discarding);
          setDiscarding(null);
        }}
      />
    </SplitSurface>
  );
}
