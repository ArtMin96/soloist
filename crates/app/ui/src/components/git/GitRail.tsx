import { useState, type ReactNode } from "react";
import { GitBranchIcon, PanelRightCloseIcon, PanelRightOpenIcon } from "lucide-react";
import { BranchHeader } from "@/components/git/BranchHeader";
import { ChangesTree } from "@/components/git/ChangesTree";
import { FilesTree } from "@/components/git/FilesTree";
import { RailDivider } from "@/components/git/RailDivider";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useGitFiles } from "@/store/git/useGitFiles";
import { useGitStatus } from "@/store/git/useGitStatus";
import { useRailLayout } from "@/store/git/useRailLayout";

/** The two things the rail can show. The values key the tab panels. */
const CHANGES_TAB = "changes";
const FILES_TAB = "files";

const RAIL_LABEL = "Version control";
const COLLAPSE_LABEL = "Hide version control";
const EXPAND_LABEL = "Show version control";

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
export function GitRail({ project }: { project: number }) {
  const [layout, setLayout] = useRailLayout();
  const [tab, setTab] = useState<string>(CHANGES_TAB);
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
          label={EXPAND_LABEL}
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
      <RailDivider width={layout.width} onResize={(width) => setLayout({ width })} />
      <aside
        aria-label={RAIL_LABEL}
        className="flex min-w-0 flex-1 flex-col bg-sidebar text-sidebar-foreground"
      >
        <div className="flex items-stretch">
          <div className="min-w-0 flex-1">
            {status.status === null ? (
              <div className="h-9 border-b border-sidebar-border" />
            ) : (
              <BranchHeader branch={status.status.branch} />
            )}
          </div>
          <div className="flex items-center border-b border-sidebar-border pe-1">
            <RailButton
              label={COLLAPSE_LABEL}
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
          <Tabs value={tab} onValueChange={setTab} className="min-h-0 flex-1 gap-0">
            <TabsList variant="line" className="h-8 w-full justify-start gap-2 px-2">
              <TabsTrigger value={CHANGES_TAB} className="flex-none gap-1.5">
                Changes
                {changes.length > 0 && (
                  <Badge variant="muted" className="tabular-nums">
                    {changes.length}
                  </Badge>
                )}
              </TabsTrigger>
              <TabsTrigger value={FILES_TAB} className="flex-none">
                Files
              </TabsTrigger>
            </TabsList>
            <TabsContent value={CHANGES_TAB} className="min-h-0">
              {changes.length === 0 ? (
                <RailEmpty>{NOTHING_CHANGED}</RailEmpty>
              ) : (
                <ScrollArea className="h-full">
                  <ChangesTree changes={changes} />
                </ScrollArea>
              )}
            </TabsContent>
            <TabsContent value={FILES_TAB} className="min-h-0">
              {files.files === null ? (
                <RailEmpty>{files.loading ? "" : NO_FILES}</RailEmpty>
              ) : (
                <ScrollArea className="h-full">
                  <FilesTree files={files.files} />
                </ScrollArea>
              )}
            </TabsContent>
          </Tabs>
        )}
      </aside>
    </div>
  );
}

/** A ghost icon button sized for the rail's chrome. */
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
