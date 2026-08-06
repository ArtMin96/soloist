import { useRef, useState, type ReactNode } from "react";
import {
  FoldVerticalIcon,
  GitBranchIcon,
  PanelRightCloseIcon,
  PanelRightOpenIcon,
  UnfoldVerticalIcon,
} from "lucide-react";
import { BranchHeader } from "@/components/git/BranchHeader";
import { ChangesTree } from "@/components/git/ChangesTree";
import { FilesTree } from "@/components/git/FilesTree";
import type { RepositoryTreeHandle } from "@/components/git/RepositoryTree";
import { PaneDivider } from "@/components/PaneDivider";
import { SegmentedControl } from "@/components/SegmentedControl";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { Option } from "@/lib/appearance";
import { CHANGE, FILE, type DiffSelection } from "@/store/git/useDiffSelection";
import { useGitFiles } from "@/store/git/useGitFiles";
import { useGitStatus } from "@/store/git/useGitStatus";
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
const EXPAND_FOLDERS_LABEL = "Expand all folders";
const COLLAPSE_FOLDERS_LABEL = "Collapse all folders";

/** What the rail says when a tab has nothing in it, per state. */
const NOT_A_REPOSITORY = "Not a git repository";
const NOTHING_CHANGED = "No changes";
const NO_FILES = "No files";

/**
 * The version-control rail: what is checked out, and what has changed under it, kept beside the
 * terminal rather than in place of it. Read-only — everything here reports.
 *
 * The one place in the rail that reaches the core, so the trees below stay presentational.
 */
export function GitRail({
  project,
  onOpen,
}: {
  project: number;
  /** A path in either tree was chosen; the diff split shows it. */
  onOpen?: (selection: DiffSelection) => void;
}) {
  const [layout, setLayout] = useRailLayout();
  const [tab, setTab] = useState<RailTab>(CHANGES_TAB);
  const [changesFoldersExpanded, setChangesFoldersExpanded] = useState(false);
  const [filesFoldersExpanded, setFilesFoldersExpanded] = useState(false);
  const changesTree = useRef<RepositoryTreeHandle>(null);
  const filesTree = useRef<RepositoryTreeHandle>(null);
  const status = useGitStatus(project);
  const files = useGitFiles(project, !layout.collapsed && tab === FILES_TAB);
  const changes = status.status?.changes ?? [];

  if (layout.collapsed) {
    return (
      <aside
        aria-label={RAIL_LABEL}
        className="flex shrink-0 flex-col items-center gap-2 border-s border-sidebar-border bg-sidebar py-2"
      >
        <RailButton
          label={EXPAND_RAIL_LABEL}
          icon={<PanelRightOpenIcon />}
          onClick={() => setLayout({ collapsed: false })}
        />
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
        <div className="flex items-stretch">
          <div className="min-w-0 flex-1">
            {status.status === null ? (
              <div className="h-11 shrink-0 border-b border-sidebar-border" />
            ) : (
              <BranchHeader branch={status.status.branch} />
            )}
          </div>
          <div className="flex items-center border-b border-sidebar-border pe-1">
            <RailButton
              label={COLLAPSE_RAIL_LABEL}
              icon={<PanelRightCloseIcon />}
              onClick={() => setLayout({ collapsed: true })}
            />
          </div>
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
            <div className="flex h-11 shrink-0 items-center gap-2.5 border-b border-sidebar-border px-3">
              <SegmentedControl<RailTab>
                value={tab}
                options={RAIL_TAB_OPTIONS}
                onChange={setTab}
                ariaLabel="Git views"
                counts={{ changes: changes.length }}
              />
              {tab === CHANGES_TAB && changes.length > 0 && (
                <TreeExpansionButton
                  expanded={changesFoldersExpanded}
                  onClick={() =>
                    changesTree.current?.setAllFoldersExpanded(!changesFoldersExpanded)
                  }
                />
              )}
              {tab === FILES_TAB && files.files !== null && (
                <TreeExpansionButton
                  expanded={filesFoldersExpanded}
                  onClick={() => filesTree.current?.setAllFoldersExpanded(!filesFoldersExpanded)}
                />
              )}
            </div>
            <div className="min-h-0 flex-1">
              <div hidden={tab !== CHANGES_TAB} className="h-full">
                {changes.length === 0 ? (
                  <RailEmpty>{NOTHING_CHANGED}</RailEmpty>
                ) : (
                  <ScrollArea className="h-full">
                    <ChangesTree
                      ref={changesTree}
                      changes={changes}
                      onExpansionChange={setChangesFoldersExpanded}
                      onOpen={(path) => onOpen?.({ kind: CHANGE, path })}
                    />
                  </ScrollArea>
                )}
              </div>
              <div hidden={tab !== FILES_TAB} className="h-full">
                {files.files === null ? (
                  <RailEmpty>{files.loading ? "" : NO_FILES}</RailEmpty>
                ) : (
                  <ScrollArea className="h-full">
                    <FilesTree
                      ref={filesTree}
                      files={files.files}
                      onExpansionChange={setFilesFoldersExpanded}
                      onOpen={(path) => onOpen?.({ kind: FILE, path })}
                    />
                  </ScrollArea>
                )}
              </div>
            </div>
          </>
        )}
      </aside>
    </div>
  );
}

/** A compact tree control for either tab; it never changes the version-control rail. */
function TreeExpansionButton({ expanded, onClick }: { expanded: boolean; onClick: () => void }) {
  const label = expanded ? COLLAPSE_FOLDERS_LABEL : EXPAND_FOLDERS_LABEL;
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-xs"
          className="ml-auto"
          aria-label={label}
          onClick={onClick}
        >
          {expanded ? <FoldVerticalIcon /> : <UnfoldVerticalIcon />}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

/** The existing rail-level control remains separate from the Files tree disclosure control. */
function RailButton({
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

function RailMessage({ children }: { children: ReactNode }) {
  return <p className="text-[0.8125rem] text-muted-foreground">{children}</p>;
}

/** The quiet line a tab shows when it has nothing in it. */
function RailEmpty({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full items-center justify-center px-6 text-center">
      <RailMessage>{children}</RailMessage>
    </div>
  );
}
