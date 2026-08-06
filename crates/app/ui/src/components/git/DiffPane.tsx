import { useEffect, useState, type ReactNode } from "react";
import { Columns2Icon, Maximize2Icon, Minimize2Icon, Rows2Icon, XIcon } from "lucide-react";
import { DiffViewer, SIDE_BY_SIDE, UNIFIED, type DiffLayout } from "@/components/git/DiffViewer";
import { DiscardDialog } from "@/components/git/DiscardDialog";
import { FilePreview } from "@/components/git/FilePreview";
import { HunkActions } from "@/components/git/HunkActions";
import { PaneDivider } from "@/components/PaneDivider";
import { SegmentedControl } from "@/components/SegmentedControl";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { Option } from "@/lib/appearance";
import { cn } from "@/lib/utils";
import { isEditableTarget } from "@/lib/hotkeys";
import { useAppearance } from "@/store/appearanceContext";
import { CHANGE, type DiffSelection } from "@/store/git/useDiffSelection";
import { useFilePreview } from "@/store/git/useFilePreview";
import { useGitDiff } from "@/store/git/useGitDiff";
import { useGitWrite } from "@/store/git/useGitWrite";
import {
  SPLIT_MAX_HEIGHT,
  SPLIT_MIN_HEIGHT,
  SPLIT_RESIZE_STEP,
  useSplitLayout,
} from "@/store/git/useSplitLayout";
import { hunkKey } from "@/lib/git";
import type { DiffTarget, HunkRange } from "@/domain";

const PANE_LABEL = "Diff";
const RESIZE_LABEL = "Resize the diff";
const CLOSE_LABEL = "Close the diff";
const MAXIMIZE_LABEL = "Fill the area with the diff";
const RESTORE_LABEL = "Share the area with the terminal";
const SIDE_BY_SIDE_LABEL = "Show the two sides side by side";
const UNIFIED_LABEL = "Show the two sides in one column";
const LOAD_FULL_LABEL = "Load the whole diff";

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
 * The diff surface: a resizable split at the foot of the main area, so a change is read beside a
 * working agent rather than in place of it. The terminal above it is never unmounted — filling
 * the area hides it, and closing the split gives its height back.
 *
 * The one place in the split that reaches the core, so the viewer and the preview below stay
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
  const [layout, setLayout] = useSplitLayout();
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

  useEscapeToClose(onClose);

  const untracked = diff?.target === UNTRACKED;
  return (
    // Filling the area *covers* what is above rather than collapsing it, so the terminal keeps
    // its size and its scrollback — restoring the split puts the reader back on the same frame
    // instead of on one the emulator had to lay out again.
    <div
      className={cn("flex flex-col", layout.maximized ? "absolute inset-0 z-10" : "shrink-0")}
      style={layout.maximized ? undefined : { height: layout.height, maxHeight: "75%" }}
    >
      {!layout.maximized && (
        <PaneDivider
          orientation="horizontal"
          label={RESIZE_LABEL}
          size={layout.height}
          min={SPLIT_MIN_HEIGHT}
          max={SPLIT_MAX_HEIGHT}
          step={SPLIT_RESIZE_STEP}
          onResize={(height) => setLayout({ height })}
        />
      )}
      <section aria-label={PANE_LABEL} className="flex min-h-0 flex-1 flex-col bg-background">
        <div className="flex h-11 shrink-0 items-center gap-2 border-b border-border px-3">
          <p className="min-w-0 flex-1 truncate font-mono text-[0.8125rem]" title={selection.path}>
            {selection.path}
          </p>
          {showing && !untracked && (
            <SegmentedControl<DiffTarget>
              value={target}
              options={TARGET_OPTIONS}
              onChange={setTarget}
              ariaLabel="Comparison"
            />
          )}
          {showing && (
            <PaneButton
              label={split === SIDE_BY_SIDE ? UNIFIED_LABEL : SIDE_BY_SIDE_LABEL}
              icon={split === SIDE_BY_SIDE ? <Rows2Icon /> : <Columns2Icon />}
              onClick={() => setSplit(split === SIDE_BY_SIDE ? UNIFIED : SIDE_BY_SIDE)}
            />
          )}
          <PaneButton
            label={layout.maximized ? RESTORE_LABEL : MAXIMIZE_LABEL}
            icon={layout.maximized ? <Minimize2Icon /> : <Maximize2Icon />}
            onClick={() => setLayout({ maximized: !layout.maximized })}
          />
          <PaneButton label={CLOSE_LABEL} icon={<XIcon />} onClick={onClose} />
        </div>

        {diff?.truncated === true && (
          <PaneNotice
            action={
              <Button size="sm" onClick={loadFull}>
                {LOAD_FULL_LABEL}
              </Button>
            }
          >
            {TRUNCATED}
          </PaneNotice>
        )}
        {content?.truncated === true && <PaneNotice>{PREVIEW_TRUNCATED}</PaneNotice>}

        <ScrollArea className="min-h-0 flex-1">
          {showing ? (
            diff === null ? (
              <PaneMessage>{diffLoading ? "" : NOT_A_REPOSITORY}</PaneMessage>
            ) : diff.binary ? (
              <PaneMessage>{BINARY}</PaneMessage>
            ) : diff.patch === "" ? (
              <PaneMessage>{NOTHING_TO_SHOW}</PaneMessage>
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
            <PaneMessage>{contentLoading ? "" : GONE}</PaneMessage>
          ) : content.text === null ? (
            <PaneMessage>{BINARY}</PaneMessage>
          ) : (
            <FilePreview path={selection.path} content={content} dark={dark} />
          )}
        </ScrollArea>
      </section>
      <DiscardDialog
        discarding={discarding === null ? null : { path: selection.path, hunk: true }}
        onCancel={() => setDiscarding(null)}
        onConfirm={() => {
          if (discarding !== null) write.discardHunk(selection.path, discarding);
          setDiscarding(null);
        }}
      />
    </div>
  );
}

/**
 * Closes the split on Escape, but only while the key is not being typed into something — a
 * terminal owns its own Escape, and so does a field.
 */
function useEscapeToClose(onClose: () => void): void {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      if (isEditableTarget(event.target)) return;
      onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);
}

/** A compact control in the split's header; none of them changes the repository. */
function PaneButton({
  label,
  icon,
  onClick,
}: {
  label: string;
  icon: ReactNode;
  onClick: () => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="ghost" size="icon-xs" aria-label={label} onClick={onClick}>
          {icon}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

/** A quiet strip stating something about what is below it, with the action that answers it. */
function PaneNotice({ children, action }: { children: ReactNode; action?: ReactNode }) {
  return (
    <div className="flex shrink-0 items-center gap-3 border-b border-border bg-muted px-3 py-2">
      <p className="min-w-0 flex-1 text-[0.8125rem] text-muted-foreground">{children}</p>
      {action}
    </div>
  );
}

/** The quiet line the split shows when it has nothing to render. */
function PaneMessage({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full items-center justify-center px-6 py-10 text-center">
      <p className="text-[0.8125rem] text-muted-foreground">{children}</p>
    </div>
  );
}
