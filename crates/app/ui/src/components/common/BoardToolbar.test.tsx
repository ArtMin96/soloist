// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  BOARD_COUNT_ATTRIBUTE,
  BoardToolbar,
  type BoardToolbarProps,
} from "@/components/common/BoardToolbar";

afterEach(cleanup);

function toolbar(overrides: Partial<BoardToolbarProps> = {}) {
  const onSearchChange = vi.fn();
  const onTagChange = vi.fn();
  render(
    <BoardToolbar
      subject="scratchpads"
      search=""
      onSearchChange={onSearchChange}
      shown={2}
      total={5}
      tags={[]}
      tag={null}
      onTagChange={onTagChange}
      {...overrides}
    />,
  );
  return { onSearchChange, onTagChange };
}

/** Owns the filter itself, so a keystroke's or a chip's round trip through the toolbar is
 *  observable as a real DOM state change rather than as a mocked callback's arguments. */
function Harness({ tags = [] }: { tags?: string[] }) {
  const [search, setSearch] = useState("");
  const [tag, setTag] = useState<string | null>(null);
  return (
    <BoardToolbar
      subject="scratchpads"
      search={search}
      onSearchChange={setSearch}
      shown={2}
      total={5}
      tags={tags}
      tag={tag}
      onTagChange={setTag}
    />
  );
}

describe("BoardToolbar", () => {
  it("names its searchbox after the subject the board holds", () => {
    toolbar({ subject: "scratchpads" });

    const searchbox = screen.getByRole("searchbox", { name: "Search scratchpads" });
    expect(searchbox.getAttribute("placeholder")).toBe("Search scratchpads…");
  });

  it("tracks the keystroke it reports", () => {
    render(<Harness />);
    const searchbox = screen.getByRole("searchbox") as HTMLInputElement;

    fireEvent.change(searchbox, { target: { value: "gate" } });

    expect(searchbox.value).toBe("gate");
  });

  it("reports how many rows survive the filter, not the total", () => {
    toolbar({ shown: 2, total: 5 });

    expect(document.querySelector(`[${BOARD_COUNT_ATTRIBUTE}]`)?.textContent).toBe("2 of 5");
  });

  it("sets the board's facet controls between the searchbox and the count", () => {
    toolbar({ facets: <button type="button">Sort scratchpads</button> });

    const facet = screen.getByRole("button", { name: "Sort scratchpads" });
    const count = document.querySelector(`[${BOARD_COUNT_ATTRIBUTE}]`) as HTMLElement;
    const searchbox = screen.getByRole("searchbox");

    expect(
      searchbox.compareDocumentPosition(facet) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(facet.compareDocumentPosition(count) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("hides the primary action when the board offers none", () => {
    toolbar({ primary: undefined });

    expect(screen.queryByRole("button", { name: "New scratchpad" })).toBeNull();
  });

  it("renders the primary action the board hands it", () => {
    const onCreate = vi.fn();
    toolbar({
      primary: (
        <button type="button" onClick={onCreate}>
          New scratchpad
        </button>
      ),
    });

    fireEvent.click(screen.getByRole("button", { name: "New scratchpad" }));

    expect(onCreate).toHaveBeenCalled();
  });

  it("filters by a tag chip, and clears it on a second press", () => {
    render(<Harness tags={["release"]} />);
    const chip = screen.getByRole("button", { name: "release" });

    fireEvent.click(chip);
    expect(chip.getAttribute("aria-pressed")).toBe("true");

    fireEvent.click(chip);
    expect(chip.getAttribute("aria-pressed")).toBe("false");
  });
});
