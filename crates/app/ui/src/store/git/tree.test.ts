import { describe, expect, it } from "vitest";
import { buildChangesTree, buildFilesTree, folderPaths, type Tree } from "@/store/git/tree";
import type { ChangeKind, FileChange, ProjectFile } from "@/domain";

function change(
  path: string,
  unstaged: ChangeKind | null,
  staged: ChangeKind | null = null,
): FileChange {
  return { path, status: { staged, unstaged }, original_path: null };
}

function file(path: string, ignored = false): ProjectFile {
  return { path, ignored };
}

/** The paths a tree holds, each with the change its row would show. */
function shape(tree: Tree): Record<string, ChangeKind | null> {
  return Object.fromEntries(Object.values(tree.nodes).map((node) => [node.path, node.change]));
}

describe("buildChangesTree", () => {
  it("groups paths under the folders that contain them", () => {
    const tree = buildChangesTree([
      change("src/app/main.rs", "modified"),
      change("src/lib.rs", "modified"),
      change("README.md", "modified"),
    ]);

    expect(tree.roots).toEqual(["src", "README.md"]);
    expect(tree.nodes["src"].children).toEqual(["src/app", "src/lib.rs"]);
    expect(tree.nodes["src/app"].children).toEqual(["src/app/main.rs"]);
    expect(tree.nodes["src"].folder).toBe(true);
    expect(tree.nodes["src/lib.rs"].folder).toBe(false);
  });

  it("gives a folder the strongest change beneath it, however deep", () => {
    const tree = buildChangesTree([
      change("src/a.rs", "untracked"),
      change("src/deep/b.rs", "conflicted"),
      change("src/c.rs", "modified"),
    ]);

    expect(tree.nodes["src"].change).toBe("conflicted");
    expect(tree.nodes["src/deep"].change).toBe("conflicted");
    expect(tree.nodes["src/a.rs"].change).toBe("untracked");
  });

  it("shows the working tree's change for a path changed on both sides of the index", () => {
    const tree = buildChangesTree([change("a.rs", "modified", "added")]);

    expect(tree.nodes["a.rs"].change).toBe("modified");
  });

  it("files a rename where the file is now, not where it came from", () => {
    const tree = buildChangesTree([
      {
        path: "new/name.rs",
        status: { staged: "renamed", unstaged: null },
        original_path: "old/name.rs",
      },
    ]);

    expect(Object.keys(shape(tree))).toEqual(["new", "new/name.rs"]);
  });

  it("orders folders before files, each alphabetically, whatever order they arrived in", () => {
    const tree = buildChangesTree([
      change("zeta.rs", "modified"),
      change("alpha.rs", "modified"),
      change("src/b.rs", "modified"),
    ]);

    expect(tree.roots).toEqual(["src", "alpha.rs", "zeta.rs"]);
  });

  it("leaves out a path version control reported without a change on either side", () => {
    const tree = buildChangesTree([change("a.rs", null, null)]);

    expect(tree.roots).toEqual([]);
  });
});

describe("buildFilesTree", () => {
  it("marks the paths version control was told to ignore, and only those", () => {
    const tree = buildFilesTree([
      file("src/main.rs"),
      file("src/notes.log", true),
      file("README.md"),
    ]);

    expect(tree.nodes["src/notes.log"].ignored).toBe(true);
    expect(tree.nodes["src/main.rs"].ignored).toBe(false);
    expect(tree.nodes["src"].ignored).toBe(false);
  });

  it("keeps an ignored directory as one folder row rather than an empty file row", () => {
    const tree = buildFilesTree([file("src/main.rs"), file("target/", true)]);

    expect(tree.nodes["target"].folder).toBe(true);
    expect(tree.nodes["target"].ignored).toBe(true);
    expect(tree.nodes["target"].children).toEqual([]);
    expect(tree.nodes["target/"]).toBeUndefined();
  });

  it("reports every folder, which is what a tree opens when it opens itself", () => {
    const tree = buildFilesTree([file("src/deep/main.rs"), file("README.md")]);

    expect(folderPaths(tree).sort()).toEqual(["src", "src/deep"]);
  });
});
