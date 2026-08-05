import { useMemo } from "react";
import { RepositoryTree } from "@/components/git/RepositoryTree";
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
export function ChangesTree({ changes }: { changes: FileChange[] }) {
  const data = useMemo(() => buildChangesTree(changes), [changes]);

  return (
    <RepositoryTree
      data={data}
      label={LABEL}
      autoExpand
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
}
