// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { DocumentRoster, type DocumentRosterCopy } from "@/components/orchestration/DocumentRoster";

afterEach(cleanup);

type Sort = "updated" | "name";

const COPY: DocumentRosterCopy = {
  label: "Docs",
  archivedLabel: "Archived docs",
  searchPlaceholder: "Search docs…",
  searchAriaLabel: "Search docs",
  sortAriaLabel: "Sort docs",
  firstRunHint: "No docs yet.",
  noResultsHint: "No docs match your search.",
};

interface Doc {
  id: number;
  name: string;
  tags: string[];
  archived: boolean;
  revision: number;
  gist: string;
}

const doc = (id: number, name: string, opts: Partial<Doc> = {}): Doc => ({
  id,
  name,
  tags: [],
  archived: false,
  revision: 1,
  gist: "",
  ...opts,
});

function renderRoster(items: Doc[]) {
  return render(
    <DocumentRoster<Doc, Sort>
      items={items}
      selected={null}
      onSelect={vi.fn()}
      copy={COPY}
      initialSort="updated"
      sortOrder={["updated", "name"]}
      sortLabels={{ updated: "Recent", name: "Name" }}
      sortItems={(rows) => rows}
      kind="scratchpad"
    />,
  );
}

describe("DocumentRoster", () => {
  it("shows the first-run hint and no listbox when there are no documents", () => {
    renderRoster([]);
    expect(screen.getByText("No docs yet.")).toBeTruthy();
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("groups an archived document under its own labelled list, apart from the active one", () => {
    renderRoster([doc(1, "plan"), doc(2, "old-plan", { archived: true })]);
    const activeList = screen.getByRole("listbox", { name: "Docs" });
    const archivedList = screen.getByRole("listbox", { name: "Archived docs" });
    expect(archivedList.textContent).toContain("Old plan");
    expect(activeList.textContent).not.toContain("Old plan");
  });

  it("renders no archived section when nothing is archived", () => {
    renderRoster([doc(1, "plan")]);
    expect(screen.queryByRole("listbox", { name: "Archived docs" })).toBeNull();
  });

  it("narrows the active list to the search match and shows the no-results hint otherwise", () => {
    renderRoster([doc(1, "release-plan"), doc(2, "research")]);
    fireEvent.change(screen.getByRole("searchbox", { name: "Search docs" }), {
      target: { value: "nothing matches this" },
    });
    expect(screen.getByText("No docs match your search.")).toBeTruthy();
    expect(screen.queryByRole("option")).toBeNull();
  });

  it("filters the active list through the tag chip group, toggling off on a second click", () => {
    renderRoster([doc(1, "api", { tags: ["backend"] }), doc(2, "ui", { tags: ["frontend"] })]);
    const chips = within(screen.getByRole("group", { name: "Filter by tag" }));
    const backend = chips.getByRole("button", { name: "backend" });
    expect(backend.getAttribute("aria-pressed")).toBe("false");

    fireEvent.click(backend);
    expect(backend.getAttribute("aria-pressed")).toBe("true");
    const filtered = screen.getByRole("listbox", { name: "Docs" });
    expect(filtered.textContent).toContain("api");
    expect(filtered.textContent).not.toContain("ui");

    fireEvent.click(backend);
    expect(backend.getAttribute("aria-pressed")).toBe("false");
    expect(screen.getByRole("listbox", { name: "Docs" }).textContent).toContain("ui");
  });
});
