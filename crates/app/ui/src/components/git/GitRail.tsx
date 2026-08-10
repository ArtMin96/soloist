import { useEffect, useMemo, useState } from "react";
import { GitBranchIcon, PanelRightCloseIcon, PanelRightOpenIcon } from "lucide-react";
import { ChangesTree, type ChangeActions } from "@/components/git/ChangesTree";
import { CommitBox } from "@/components/git/CommitBox";
import { ConfirmDialog } from "@/components/git/ConfirmDialog";
import { ConflictNotice } from "@/components/git/ConflictNotice";
import { DiscardDialog, type Discardable } from "@/components/git/DiscardDialog";
import { FilesTree } from "@/components/git/FilesTree";
import {
  RailEmpty,
  RailError,
  RailMessage,
  TreeExpansionButton,
} from "@/components/git/RailChrome";
import { raiseRefusal } from "@/components/git/refusalToast";
import { TrustNotice } from "@/components/git/TrustNotice";
import { IconButton } from "@/components/IconButton";
import { PaneDivider } from "@/components/PaneDivider";
import { SegmentedControl } from "@/components/SegmentedControl";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { Option } from "@/lib/appearance";
import type { FileChange } from "@/domain";
import { publishBranchCluster, type BranchClusterView } from "@/store/git/branchCluster";
import { CHANGE, FILE, type DiffSelection } from "@/store/git/useDiffSelection";
import { useGitFiles } from "@/store/git/useGitFiles";
import { useGitStatus } from "@/store/git/useGitStatus";
import { useCommitMessageDraft } from "@/store/git/useCommitMessageDraft";
import { useCommitTemplate } from "@/store/git/useCommitTemplate";
import { useTreeExpansion } from "@/store/git/useTreeExpansion";
import { buildChangesTree, buildFilesTree } from "@/store/git/tree";
import {
  BRANCH as BRANCH_ACTION,
  EXCHANGE as EXCHANGE_ACTION,
  MERGE as MERGE_ACTION,
  STASH as STASH_ACTION,
  useGitSync,
} from "@/store/git/useGitSync";
import { useGitWrite } from "@/store/git/useGitWrite";
import {
  RAIL_MAX_WIDTH,
  RAIL_MIN_WIDTH,
  RAIL_RESIZE_STEP,
  useRailLayout,
} from "@/store/git/useRailLayout";

/** The two things the rail can show. The values key the selected view. */
const CHANGES_TAB = "changes" as const;
const FILES_TAB = "files" as const;
type RailTab = typeof CHANGES_TAB | typeof FILES_TAB;

const RAIL_TAB_OPTIONS: Option<RailTab>[] = [
  { value: CHANGES_TAB, label: "Changes" },
  { value: FILES_TAB, label: "Files" },
];

const RAIL_LABEL = "Version control";
const COLLAPSE_RAIL_LABEL = "Hide version control";
const EXPAND_RAIL_LABEL = "Show version control";
const RESIZE_RAIL_LABEL = "Resize the version control rail";

/** The question the rail asks before it destroys anything. */
const ABANDON_MERGE_TITLE = "Abandon this merge?";
const ABANDON_MERGE_CONFIRM = "Abandon";
const ABANDON_MERGE_CANCEL = "Keep merging";

/** Stands in for a project with nothing changed, so "no changes" is one list rather than a new one
 *  every render — which is what keeps the tree built from it from being rebuilt for nothing. */
const NO_CHANGES: FileChange[] = [];
const NO_PATHS: string[] = [];

/** What the rail says when a tab has nothing in it, per state. */
const NOT_A_REPOSITORY = "Not a git repository";
const NOTHING_CHANGED = "No changes";
const NO_FILES = "No files";

/**
 * The version-control rail: what has changed under what is checked out, and what can be done about
 * it — kept beside the terminal rather than in place of it.
 *
 * The one place any repository surface reaches the core, so the trees and the commit box below stay
 * presentational and the branch cluster in the window chrome reads the same status this does rather
 * than asking for its own. Whether a change is allowed at all is the core's answer, asked once here
 * so the rail can offer the trust affordance rather than let an action fail.
 */
export function GitRail({
  project,
  onOpen,
  onOpenPullRequest,
}: {
  project: number;
  /** A path in either tree was chosen; the split shows it. */
  onOpen?: (selection: DiffSelection) => void;
  /** The pull-request view was asked for; the split shows it instead. */
  onOpenPullRequest?: () => void;
}) {
  const [layout, setLayout] = useRailLayout();
  const [tab, setTab] = useState<RailTab>(CHANGES_TAB);
  const [discarding, setDiscarding] = useState<Discardable | null>(null);
  const [abandoningMerge, setAbandoningMerge] = useState(false);
  const [switcherOpen, setSwitcherOpen] = useState(false);
  const status = useGitStatus(project);
  const files = useGitFiles(project, !layout.collapsed && tab === FILES_TAB);
  const write = useGitWrite(project);
  const sync = useGitSync(project, switcherOpen);
  const draft = useCommitMessageDraft(project);
  const template = useCommitTemplate(project, write.trusted === true);
  const changes = status.status?.changes ?? NO_CHANGES;
  const discardablePaths = status.status?.capabilities.discardablePaths ?? NO_PATHS;
  const discardable = useMemo(() => new Set(discardablePaths), [discardablePaths]);
  // Built here rather than inside each tree, because whoever owns which folders are open needs the
  // same shape the rows hang on — one fact, one owner.
  const changesTree = useMemo(() => buildChangesTree(changes), [changes]);
  const filesTree = useMemo(
    () => (files.files === null ? null : buildFilesTree(files.files)),
    [files.files],
  );
  const changesFolders = useTreeExpansion(changesTree, true);
  const filesFolders = useTreeExpansion(filesTree, false);
  // One place a refused change to a path is stated, whichever asked for it. What the remote refused
  // is not here: those controls live in the window chrome now, which has no line to say it in.
  const refusal = write.error ?? draft.error;
  const actions: ChangeActions | null =
    write.trusted === true
      ? {
          onStage: (path, stage) => (stage ? write.stage(path) : write.unstage(path)),
          onDiscard: (path) => setDiscarding({ path, hunk: false }),
          discardable,
          busy: write.busy,
        }
      : null;
  // What the window chrome shows about the checked-out branch, handed over from here so the two
  // surfaces are one read. Until the project is trusted nothing may change it, so neither the
  // switcher, the exchange with the remote, nor the pull request is offered.
  const branch = status.status?.branch ?? null;
  const capabilities = status.status?.capabilities ?? null;
  const changeCounts = status.status?.changeCounts ?? null;
  const trusted = write.trusted === true;
  const cluster = useMemo<BranchClusterView | null>(
    () =>
      branch === null || capabilities === null || changeCounts === null
        ? null
        : {
            branch,
            capabilities,
            changeCounts,
            branches: sync.branches,
            exchanging: sync.exchanging,
            busy: sync.busy(BRANCH_ACTION) || sync.busy(STASH_ACTION) || sync.busy(EXCHANGE_ACTION),
            exchange: trusted
              ? { fetch: sync.fetch, pull: sync.pull, push: sync.push, stop: sync.stopExchange }
              : null,
            branchActions: trusted
              ? {
                  switchTo: sync.switchBranch,
                  create: sync.createBranch,
                  remove: sync.deleteBranch,
                  stash: capabilities.stash ? sync.stash : null,
                  popStash: sync.popStash,
                }
              : null,
            // Proposing one pushes the branch and runs the repository's own configuration, so it is
            // offered exactly where every other change is: once the project is trusted, and not
            // before.
            openPullRequest: trusted && onOpenPullRequest !== undefined ? onOpenPullRequest : null,
            onBranchesOpen: setSwitcherOpen,
          },
    [branch, capabilities, changeCounts, onOpenPullRequest, sync, trusted],
  );

  useEffect(() => {
    publishBranchCluster(cluster);
    return () => publishBranchCluster(null);
  }, [cluster]);

  // A refusal from a control that has left the rail has nowhere in the rail to be read, so every
  // exchange with the remote reports through the alert stack. Cleared once it has been said, so the
  // next failure of the same action is announced again rather than swallowed as unchanged.
  const { error: exchangeError, dismissError: dismissExchangeError } = sync;
  useEffect(() => {
    if (exchangeError === null) return;
    raiseRefusal(exchangeError);
    dismissExchangeError();
  }, [exchangeError, dismissExchangeError]);

  if (layout.collapsed) {
    return (
      <aside
        aria-label={RAIL_LABEL}
        className="flex shrink-0 flex-col items-center gap-2 border-s border-sidebar-border bg-sidebar py-2"
      >
        <IconButton
          label={EXPAND_RAIL_LABEL}
          icon={<PanelRightOpenIcon />}
          onClick={() => setLayout({ collapsed: false })}
        />
        {/* What is checked out is in the window chrome whether the rail is open or not, so all a
            closed rail still owes is how much has changed under it. */}
        {changes.length > 0 && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Badge variant="muted" className="tabular-nums">
                {changes.length}
              </Badge>
            </TooltipTrigger>
            <TooltipContent>{`${changes.length} changed`}</TooltipContent>
          </Tooltip>
        )}
      </aside>
    );
  }

  return (
    <div className="flex shrink-0" style={{ width: `${layout.width}px` }}>
      <PaneDivider
        orientation="vertical"
        label={RESIZE_RAIL_LABEL}
        size={layout.width}
        min={RAIL_MIN_WIDTH}
        max={RAIL_MAX_WIDTH}
        step={RAIL_RESIZE_STEP}
        onResize={(width) => setLayout({ width })}
      />
      <aside
        aria-label={RAIL_LABEL}
        className="flex min-w-0 flex-1 flex-col bg-sidebar text-sidebar-foreground"
      >
        {/* One chrome row, whatever the rail is showing: the view switch when there is a repository
            to switch views of, and — always — the control that closes the rail. */}
        <div className="flex h-11 shrink-0 items-center gap-2.5 border-b border-sidebar-border px-3">
          {status.status !== null && (
            <>
              <SegmentedControl<RailTab>
                value={tab}
                options={RAIL_TAB_OPTIONS}
                onChange={setTab}
                ariaLabel="Git views"
                counts={{ changes: changes.length }}
              />
              {tab === CHANGES_TAB && changes.length > 0 && (
                <TreeExpansionButton
                  expanded={changesFolders.allExpanded}
                  onClick={changesFolders.toggleAll}
                />
              )}
              {tab === FILES_TAB && filesTree !== null && (
                <TreeExpansionButton
                  expanded={filesFolders.allExpanded}
                  onClick={filesFolders.toggleAll}
                />
              )}
            </>
          )}
          <IconButton
            className="ms-auto"
            label={COLLAPSE_RAIL_LABEL}
            icon={<PanelRightCloseIcon />}
            onClick={() => setLayout({ collapsed: true })}
          />
        </div>

        {/* A project kept out of version control is a choice, not a fault, so the rail says so
            once and stays quiet — and says nothing at all until the first read has answered,
            rather than claiming it while the answer is still being fetched. */}
        {status.status === null ? (
          status.loading ? (
            <div className="min-h-0 flex-1" />
          ) : (
            <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-2 px-6 text-center">
              <GitBranchIcon aria-hidden className="size-5 text-muted-foreground/60" />
              <RailMessage>{NOT_A_REPOSITORY}</RailMessage>
            </div>
          )
        ) : (
          <>
            <div className="min-h-0 flex-1">
              <div hidden={tab !== CHANGES_TAB} className="h-full">
                {changes.length === 0 ? (
                  <RailEmpty>{NOTHING_CHANGED}</RailEmpty>
                ) : (
                  <ScrollArea className="h-full" constrainContent>
                    <ChangesTree
                      tree={changesTree}
                      changes={changes}
                      actions={actions}
                      expanded={changesFolders.expanded}
                      onExpandedChange={changesFolders.setExpanded}
                      onOpen={(path) => onOpen?.({ kind: CHANGE, path })}
                    />
                  </ScrollArea>
                )}
              </div>
              <div hidden={tab !== FILES_TAB} className="h-full">
                {filesTree === null ? (
                  <RailEmpty>{files.loading ? "" : NO_FILES}</RailEmpty>
                ) : (
                  <ScrollArea className="h-full" constrainContent>
                    <FilesTree
                      tree={filesTree}
                      expanded={filesFolders.expanded}
                      onExpandedChange={filesFolders.setExpanded}
                      onOpen={(path) => onOpen?.({ kind: FILE, path })}
                    />
                  </ScrollArea>
                )}
              </div>
            </div>
            <ConflictNotice
              changes={changes}
              merging={status.status?.merging === true}
              busy={sync.busy(MERGE_ACTION)}
              onAbandon={() => setAbandoningMerge(true)}
            />
            {refusal !== null && <RailError message={refusal} />}
            {write.trusted === false ? (
              <TrustNotice onTrust={write.trust} />
            ) : (
              write.trusted === true && (
                <CommitBox
                  changes={changes}
                  busy={write.committing}
                  template={template}
                  draft={
                    draft.available ? { drafting: draft.drafting, request: draft.draft } : null
                  }
                  onCommit={write.commit}
                />
              )
            )}
          </>
        )}
      </aside>
      <DiscardDialog
        discarding={discarding}
        onCancel={() => setDiscarding(null)}
        onConfirm={() => {
          if (discarding !== null) write.discard(discarding.path);
          setDiscarding(null);
        }}
      />
      <ConfirmDialog
        open={abandoningMerge}
        title={ABANDON_MERGE_TITLE}
        confirm={ABANDON_MERGE_CONFIRM}
        cancel={ABANDON_MERGE_CANCEL}
        onCancel={() => setAbandoningMerge(false)}
        onConfirm={() => {
          sync.abortMerge();
          setAbandoningMerge(false);
        }}
      >
        The working tree goes back to what was checked out before the merge began. Any conflict you
        have already resolved goes with it, and this cannot be undone.
      </ConfirmDialog>
    </div>
  );
}
