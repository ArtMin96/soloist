// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DocumentList, type DocumentRow } from "@/components/orchestration/DocumentList";

afterEach(cleanup);

const doc = (id: number, name: string, gist = "", tags: string[] = []): DocumentRow => ({
  id,
  name,
  revision: 1,
  gist,
  tags,
});

const docs = [doc(1, "plan", "the plan"), doc(2, "research"), doc(3, "risks")];

const EMPTY_HINT = "No documents yet.";

describe("DocumentList", () => {
  it("puts the roving cursor on the selected option", () => {
    render(
      <DocumentList
        items={docs}
        selected="research"
        onSelect={vi.fn()}
        label="Docs"
        emptyHint={EMPTY_HINT}
        kind="scratchpad"
      />,
    );
    const options = screen.getAllByRole("option");
    expect(options.map((o) => o.tabIndex)).toEqual([-1, 0, -1]);
    expect(screen.getByRole("option", { selected: true }).textContent).toContain("research");
  });

  it("moves the roving cursor with the arrow keys and clamps at the ends", () => {
    render(
      <DocumentList
        items={docs}
        selected={null}
        onSelect={vi.fn()}
        label="Docs"
        emptyHint={EMPTY_HINT}
        kind="scratchpad"
      />,
    );
    const listbox = screen.getByRole("listbox");
    const options = screen.getAllByRole("option");
    expect(options[0].tabIndex).toBe(0); // defaults to the first
    fireEvent.keyDown(listbox, { key: "ArrowDown" });
    expect(options[1].tabIndex).toBe(0);
    expect(document.activeElement).toBe(options[1]);
    fireEvent.keyDown(listbox, { key: "End" });
    expect(options[2].tabIndex).toBe(0);
    fireEvent.keyDown(listbox, { key: "ArrowDown" }); // already at the end
    expect(options[2].tabIndex).toBe(0);
    fireEvent.keyDown(listbox, { key: "Home" });
    expect(options[0].tabIndex).toBe(0);
    expect(document.activeElement).toBe(options[0]);
  });

  it("keeps the roving cursor on the same document when the list changes live", () => {
    const { rerender } = render(
      <DocumentList
        items={docs}
        selected={null}
        onSelect={vi.fn()}
        label="Docs"
        emptyHint={EMPTY_HINT}
        kind="scratchpad"
      />,
    );
    fireEvent.keyDown(screen.getByRole("listbox"), { key: "End" }); // cursor on "risks" (index 2)
    expect(screen.getByRole("option", { name: /risks/ }).tabIndex).toBe(0);
    // The first row is removed live: "risks" is now index 1, but the cursor stays on it.
    rerender(
      <DocumentList
        items={docs.slice(1)}
        selected={null}
        onSelect={vi.fn()}
        label="Docs"
        emptyHint={EMPTY_HINT}
        kind="scratchpad"
      />,
    );
    expect(screen.getByRole("option", { name: /risks/ }).tabIndex).toBe(0);
    expect(screen.getAllByRole("option").map((o) => o.tabIndex)).toEqual([-1, 0]);
  });

  it("opens the clicked option", () => {
    const onSelect = vi.fn();
    render(
      <DocumentList
        items={docs}
        selected={null}
        onSelect={onSelect}
        label="Docs"
        emptyHint={EMPTY_HINT}
        kind="scratchpad"
      />,
    );
    fireEvent.click(screen.getAllByRole("option")[2]);
    expect(onSelect).toHaveBeenCalledWith("risks");
  });

  it("renders the hint (and no listbox) when there are no documents", () => {
    render(
      <DocumentList
        items={[]}
        selected={null}
        onSelect={vi.fn()}
        label="Docs"
        emptyHint={EMPTY_HINT}
        kind="scratchpad"
      />,
    );
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(screen.getByText(EMPTY_HINT)).toBeTruthy();
  });

  it("renders the humanized title, the revision and the gist for a row", () => {
    const slugged = [doc(9, "release-plan", "ships next week")];
    render(
      <DocumentList
        items={slugged}
        selected={null}
        onSelect={vi.fn()}
        label="Docs"
        emptyHint={EMPTY_HINT}
        kind="scratchpad"
      />,
    );
    const row = screen.getByRole("option");
    expect(row.textContent).toContain("Release plan");
    expect(row.textContent).toContain("r1");
    expect(row.textContent).toContain("ships next week");
  });

  it("stamps each row with the scratchpad handle attribute when kind is scratchpad, never a diagram's", () => {
    render(
      <DocumentList
        items={docs}
        selected={null}
        onSelect={vi.fn()}
        label="Docs"
        emptyHint={EMPTY_HINT}
        kind="scratchpad"
      />,
    );
    const options = screen.getAllByRole("option");
    expect(options.map((o) => o.getAttribute("data-scratchpad-name"))).toEqual([
      "plan",
      "research",
      "risks",
    ]);
    expect(document.querySelector("[data-diagram-name]")).toBeNull();
  });

  it("shows a row's tags, and shows no tag chip on an untagged row", () => {
    const rows = [doc(1, "plan", "the plan", ["a", "b"]), doc(2, "research")];
    render(
      <DocumentList
        items={rows}
        selected={null}
        onSelect={vi.fn()}
        label="Docs"
        emptyHint={EMPTY_HINT}
        kind="scratchpad"
      />,
    );
    const options = screen.getAllByRole("option");
    expect(options[0].textContent).toContain("a");
    expect(options[0].textContent).toContain("b");
    expect(options[0].querySelectorAll("[data-tag]").length).toBe(2);
    expect(options[1].querySelectorAll("[data-tag]").length).toBe(0);
  });

  it("stamps each row with the diagram handle attribute when kind is diagram, never a scratchpad's", () => {
    render(
      <DocumentList
        items={docs}
        selected={null}
        onSelect={vi.fn()}
        label="Docs"
        emptyHint={EMPTY_HINT}
        kind="diagram"
      />,
    );
    const options = screen.getAllByRole("option");
    expect(options.map((o) => o.getAttribute("data-diagram-name"))).toEqual([
      "plan",
      "research",
      "risks",
    ]);
    expect(document.querySelector("[data-scratchpad-name]")).toBeNull();
  });
});
