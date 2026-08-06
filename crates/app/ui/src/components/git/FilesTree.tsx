import { forwardRef, useMemo } from "react";
import { RepositoryTree, type RepositoryTreeHandle } from "@/components/git/RepositoryTree";
import { TreeItemLabel } from "@/components/ui/tree";
import { IGNORED_TONE_CLASS } from "@/lib/git";
import { cn } from "@/lib/utils";
import { buildFilesTree } from "@/store/git/tree";
import type { ProjectFile } from "@/domain";

/** Names the tree for a screen reader. */
const LABEL = "Project files";

/** The word a screen reader hears in place of the dimming an ignored row shows. */
const IGNORED = "ignored";

/**
 * The whole project, as version control sees it. A path it was told to ignore is dimmed and says
 * so; an ignored folder is one row rather than the thousands of files beneath it, because that
 * is how the listing reports it.
 */
export const FilesTree = forwardRef<
  RepositoryTreeHandle,
  { files: ProjectFile[]; onExpansionChange?: (allExpanded: boolean) => void }
>(function FilesTree({ files, onExpansionChange }, ref) {
  const data = useMemo(() => buildFilesTree(files), [files]);

  return (
    <RepositoryTree
      ref={ref}
      data={data}
      label={LABEL}
      autoExpand={false}
      onExpansionChange={onExpansionChange}
      row={(node) => (
        <TreeItemLabel className={cn(node.ignored && IGNORED_TONE_CLASS)}>
          {node.name}
          {node.ignored && <span className="sr-only"> ({IGNORED})</span>}
        </TreeItemLabel>
      )}
    />
  );
});
