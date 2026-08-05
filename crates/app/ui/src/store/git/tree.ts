import type { ChangeKind, FileChange, ProjectFile } from "@/domain";
import { primaryChange, strongerChange } from "@/lib/git";

/**
 * One node of a repository tree. A folder's `change` is the strongest of its descendants', so a
 * collapsed folder still says whether something under it needs attention; a file's is its own.
 */
export interface TreeNode {
  /** The full path from the repository root — unique, so it doubles as the node's identity. */
  path: string;
  /** The last segment, which is what the row shows. */
  name: string;
  /** Child paths in display order, empty for a file. */
  children: string[];
  /** Whether this node holds other nodes. */
  folder: boolean;
  /** The change this node reports, or null when there is none (a Files-tab entry). */
  change: ChangeKind | null;
  /** Whether version control was told to ignore this path, or everything under it. */
  ignored: boolean;
}

/** A whole tree: its nodes by path, and the paths at the top level. */
export interface Tree {
  nodes: Record<string, TreeNode>;
  roots: string[];
}

/** The separator version control reports paths in, on every platform. */
const SEPARATOR = "/";

/**
 * Groups changed paths into a tree, each folder inheriting the strongest change beneath it.
 *
 * A rename is filed under where the file is *now*; where it came from is the row's business,
 * not the tree's.
 */
export function buildChangesTree(changes: FileChange[]): Tree {
  const builder = newBuilder();
  for (const change of changes) {
    const kind = primaryChange(change.status);
    if (kind === null) continue;
    insert(builder, change.path, { change: kind, ignored: false });
  }
  return finish(builder);
}

/**
 * Groups a project's file listing into a tree. An ignored entry marks itself and, when version
 * control reported a whole folder, everything the folder would contain — the listing names the
 * folder rather than walking it, so the subtree is the folder.
 */
export function buildFilesTree(files: ProjectFile[]): Tree {
  const builder = newBuilder();
  for (const file of files) {
    insert(builder, file.path, {
      change: null,
      ignored: file.ignored,
      // A trailing separator is how a whole ignored directory is reported: it names a folder,
      // and the listing stops there rather than walking into it.
      folder: file.path.endsWith(SEPARATOR),
    });
  }
  return finish(builder);
}

interface Builder {
  nodes: Map<string, TreeNode>;
  roots: string[];
}

interface Leaf {
  change: ChangeKind | null;
  ignored: boolean;
  folder?: boolean;
}

function newBuilder(): Builder {
  return { nodes: new Map(), roots: [] };
}

// Files an entry at `path`, creating the folders above it and folding the entry's change into
// each of them. Insertion order is preserved, which is the order version control listed the
// paths in.
function insert(builder: Builder, path: string, leaf: Leaf): void {
  const segments = path.split(SEPARATOR).filter((segment) => segment !== "");
  let parentPath: string | null = null;
  let walked = "";
  for (let index = 0; index < segments.length; index++) {
    const segment = segments[index];
    const last = index === segments.length - 1;
    walked = walked === "" ? segment : `${walked}${SEPARATOR}${segment}`;
    let node = builder.nodes.get(walked);
    if (!node) {
      node = {
        path: walked,
        name: segment,
        children: [],
        folder: !last || leaf.folder === true,
        change: null,
        ignored: false,
      };
      builder.nodes.set(walked, node);
      const parent = parentPath === null ? null : builder.nodes.get(parentPath);
      if (parent) parent.children.push(walked);
      else builder.roots.push(walked);
    }
    // Every folder above the entry inherits its change, so a collapsed folder still reports
    // what is happening beneath it.
    node.change = merge(node.change, leaf.change);
    // Only the entry itself is ignored. A folder version control ignores is reported as itself
    // rather than walked, so its subtree is the row — there is nothing below it to mark.
    if (last) node.ignored = node.ignored || leaf.ignored;
    parentPath = walked;
  }
}

function merge(current: ChangeKind | null, incoming: ChangeKind | null): ChangeKind | null {
  if (incoming === null) return current;
  return current === null ? incoming : strongerChange(current, incoming);
}

// Folders lead, then files, each group alphabetical and case-insensitive — the order a file
// browser is read in, rather than whichever order the tool happened to print.
function finish(builder: Builder): Tree {
  const nodes: Record<string, TreeNode> = {};
  for (const node of builder.nodes.values()) {
    nodes[node.path] = { ...node, children: sortPaths(node.children, builder) };
  }
  return { nodes, roots: sortPaths(builder.roots, builder) };
}

// One collator for every comparison. `localeCompare` builds one per call, which dominates the
// cost of ordering a large repository — measurably, which is why it is hoisted.
const BY_NAME = new Intl.Collator(undefined, { numeric: true });

function sortPaths(paths: string[], builder: Builder): string[] {
  const rank = (path: string) => (builder.nodes.get(path)?.folder === true ? 0 : 1);
  const name = (path: string) => builder.nodes.get(path)?.name ?? path;
  return [...paths].sort((a, b) => rank(a) - rank(b) || BY_NAME.compare(name(a), name(b)));
}

/** Every folder path in `tree` — what a tree opens to when the user asks to expand it all. */
export function folderPaths(tree: Tree): string[] {
  const folders: string[] = [];
  for (const node of Object.values(tree.nodes)) {
    if (node.folder) folders.push(node.path);
  }
  return folders;
}
