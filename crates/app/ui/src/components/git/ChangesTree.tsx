import { forwardRef, useMemo } from "react";
import { RepositoryTree, type RepositoryTreeHandle } from "@/components/git/RepositoryTree";
import { StatusLetter } from "@/components/git/StatusLetter";
import { TreeItemLabel } from "@/components/ui/tree";
import { CHANGE } from "@/lib/git";
import { cn } from "@/lib/utils";
import { buildChangesTree } from "@/store/git/tree";
import type { FileChange } from "@/domain";

/** Names the tree for a screen reader. */
const LABEL = "Changed files";

/**
 * Every path that differs from the last commit, grouped by folder, each row carrying the letter
 * version control prints for it. A folder shows its strongest child's change, so a closed folder
 * still says whether something under it needs attention.
 */
export const ChangesTree = forwardRef<
  RepositoryTreeHandle,
  { changes: FileChange[]; onExpansionChange?: (allExpanded: boolean) => void }
>(function ChangesTree({ changes, onExpansionChange }, ref) {
  const data = useMemo(() => buildChangesTree(changes), [changes]);

  return (
    <RepositoryTree
      ref={ref}
      data={data}
      label={LABEL}
      autoExpand
      onExpansionChange={onExpansionChange}
      row={(node) => (
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
          {node.change !== null && <StatusLetter change={node.change} className="ms-auto" />}
        </>
      )}
    />
  );
});
