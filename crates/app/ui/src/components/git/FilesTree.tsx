import { RepositoryTree } from "@/components/git/RepositoryTree";
import { TreeItemLabel } from "@/components/ui/tree";
import { IGNORED_TONE_CLASS } from "@/lib/git";
import { cn } from "@/lib/utils";
import type { Tree } from "@/store/git/tree";

/** Names the tree for a screen reader. */
const LABEL = "Project files";

/** The word a screen reader hears in place of the dimming an ignored row shows. */
const IGNORED = "ignored";

/**
 * The whole project, as version control sees it. A path it was told to ignore is dimmed and says
 * so; an ignored folder is one row rather than the thousands of files beneath it, because that
 * is how the listing reports it.
 */
export function FilesTree({
  tree,
  expanded,
  onExpandedChange,
  onOpen,
}: {
  /** The shape the rows hang on, built by whoever also owns which folders are open. */
  tree: Tree;
  expanded: string[];
  onExpandedChange: (paths: string[] | ((open: string[]) => string[])) => void;
  onOpen?: (path: string) => void;
}) {
  return (
    <RepositoryTree
      data={tree}
      label={LABEL}
      expanded={expanded}
      onExpandedChange={onExpandedChange}
      onOpen={(node) => onOpen?.(node.path)}
      row={(node) => (
        <TreeItemLabel className={cn(node.ignored && IGNORED_TONE_CLASS)}>
          {node.name}
          {node.ignored && <span className="sr-only"> ({IGNORED})</span>}
        </TreeItemLabel>
      )}
    />
  );
}
