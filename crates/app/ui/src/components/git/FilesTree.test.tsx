// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { FilesTree } from "@/components/git/FilesTree";
import { buildFilesTree } from "@/store/git/tree";
import { useTreeExpansion } from "@/store/git/useTreeExpansion";
import type { ProjectFile } from "@/domain";

function file(path: string, ignored = false): ProjectFile {
  return { path, ignored };
}

/**
 * The tree as the rail composes it — the same builder and the same expansion owner — so what these
 * exercise is the whole of what a reader sees, not a tree handed a list somebody made up.
 */
function Listing({ files }: { files: ProjectFile[] }) {
  const tree = buildFilesTree(files);
  const folders = useTreeExpansion(tree, false);
  return (
    <FilesTree tree={tree} expanded={folders.expanded} onExpandedChange={folders.setExpanded} />
  );
}

function rows(): HTMLElement[] {
  return within(screen.getByRole("tree")).getAllByRole("treeitem");
}

function row(name: string): HTMLElement {
  const found = rows().find((item) => within(item).queryByText(name) !== null);
  if (!found) throw new Error(`no row named ${name}`);
  return found;
}

afterEach(cleanup);

describe("FilesTree", () => {
  it("presents the project as a tree a screen reader can walk", () => {
    render(<Listing files={[file("src/main.rs")]} />);

    const tree = screen.getByRole("tree", { name: "Project files" });
    expect(within(tree).getAllByRole("treeitem")).toHaveLength(1);
    expect(row("src").getAttribute("aria-expanded")).toBe("false");
  });

  it("opens a folder on demand rather than unfolding the whole project on sight", () => {
    render(<Listing files={[file("src/main.rs"), file("src/lib.rs")]} />);
    expect(rows()).toHaveLength(1);

    const tree = screen.getByRole("tree");
    fireEvent.keyDown(tree, { key: "ArrowRight" });
    fireEvent.keyUp(tree, { key: "ArrowRight" });

    expect(rows()).toHaveLength(3);
  });

  it("says which paths version control ignores, not only by dimming them", () => {
    render(<Listing files={[file("README.md"), file("run.log", true)]} />);

    expect(row("run.log").textContent).toContain("ignored");
    expect(row("README.md").textContent).not.toContain("ignored");
  });

  it("shows an ignored folder as one row, with nothing beneath it to open", () => {
    render(<Listing files={[file("src/main.rs"), file("target/", true)]} />);
    const tree = screen.getByRole("tree");

    // Down to the ignored folder (the tree starts focused on the first row), then open it.
    fireEvent.keyDown(tree, { key: "ArrowDown" });
    fireEvent.keyUp(tree, { key: "ArrowDown" });
    fireEvent.keyDown(tree, { key: "ArrowRight" });
    fireEvent.keyUp(tree, { key: "ArrowRight" });

    expect(row("target").getAttribute("aria-expanded")).toBe("true");
    expect(rows()).toHaveLength(2);
  });
});
