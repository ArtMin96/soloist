import { forwardRef, useMemo } from "react";
import { Undo2Icon } from "lucide-react";
import { RepositoryTree, type RepositoryTreeHandle } from "@/components/git/RepositoryTree";
import { StageCheckbox } from "@/components/git/StageCheckbox";
import { StatusLetter } from "@/components/git/StatusLetter";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { TreeItemLabel } from "@/components/ui/tree";
import { CHANGE } from "@/lib/git";
import { cn } from "@/lib/utils";
import { buildChangesTree } from "@/store/git/tree";
import type { FileChange } from "@/domain";

/** Names the tree for a screen reader. */
const LABEL = "Changed files";

const DISCARD_LABEL = "Discard the changes to";

export interface ChangeActions {
  /** Records everything the working tree holds for a path, or takes it back out again. */
  onStage: (path: string, stage: boolean) => void;
  /** Throws away what the working tree holds beyond the index — asked about first. */
  onDiscard: (path: string) => void;
  /** Whether an action on this path is still running. */
  busy: (path: string) => boolean;
}

/**
 * Every path that differs from the last commit, grouped by folder, each row carrying the letter
 * version control prints for it. A folder shows its strongest child's change, so a closed folder
 * still says whether something under it needs attention.
 *
 * A file's row also carries what can be done with it — recorded for the next commit, or thrown
 * away. Those are absent, not disabled, until the project has been trusted to be changed: an
 * action nobody may take is not an action.
 */
export const ChangesTree = forwardRef<
  RepositoryTreeHandle,
  {
    changes: FileChange[];
    /** The actions each file row offers, or null while the project may not be changed. */
    actions: ChangeActions | null;
    onExpansionChange?: (allExpanded: boolean) => void;
    onOpen?: (path: string) => void;
  }
>(function ChangesTree({ changes, actions, onExpansionChange, onOpen }, ref) {
  const data = useMemo(() => buildChangesTree(changes), [changes]);
  const byPath = useMemo(() => new Map(changes.map((change) => [change.path, change])), [changes]);

  return (
    <RepositoryTree
      ref={ref}
      data={data}
      label={LABEL}
      autoExpand
      onExpansionChange={onExpansionChange}
      onOpen={(node) => onOpen?.(node.path)}
      row={(node) => {
        const change = byPath.get(node.path);
        return (
          <>
            {/* Only the file itself is struck through: a folder holding a deleted file is still
                there, and striking it would say otherwise. */}
            <TreeItemLabel
              className={cn(
                !node.folder && node.change !== null && CHANGE[node.change].gone && "line-through",
              )}
            >
              {node.name}
            </TreeItemLabel>
            <div className="ms-auto flex items-center gap-0.5">
              {actions !== null && change !== undefined && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon-xs"
                      aria-label={`${DISCARD_LABEL} ${node.path}`}
                      disabled={actions.busy(node.path)}
                      className="opacity-0 focus-visible:opacity-100 group-hover/tree-item:opacity-100"
                      onClick={(event) => {
                        event.stopPropagation();
                        actions.onDiscard(node.path);
                      }}
                    >
                      <Undo2Icon />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{`${DISCARD_LABEL} ${node.path}`}</TooltipContent>
                </Tooltip>
              )}
              {node.change !== null && <StatusLetter change={node.change} />}
              {actions !== null && change !== undefined && (
                <StageCheckbox
                  path={node.path}
                  status={change.status}
                  disabled={actions.busy(node.path)}
                  onChange={(stage) => actions.onStage(node.path, stage)}
                />
              )}
            </div>
          </>
        );
      }}
    />
  );
});
