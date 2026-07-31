// @vitest-environment jsdom
/// <reference types="node" />
import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  ANNOUNCEMENT_SLOT,
  SORT_DURATION,
  SortableItem,
  SortableList,
} from "@/components/SortableList";
import { useSortableList } from "@/components/useSortableList";

afterEach(cleanup);

describe("the settle duration", () => {
  // dnd-kit builds the transition in script, so the duration cannot be `var(--dur-control)` and
  // has to be mirrored. This is what makes the token still the one place it is changed: move it
  // and this reddens.
  it("matches the --dur-control token it mirrors", () => {
    const css = readFileSync(`${process.cwd()}/src/index.css`, "utf8");
    const declared = css.match(/--dur-control:\s*(\d+)ms/)?.[1];

    expect(declared).toBeDefined();
    expect(Number(declared)).toBe(SORT_DURATION);
  });
});

// A consumer of the list's move actions — the shape any pointer-free affordance takes.
function MoveControls({ id }: { id: string }) {
  const list = useSortableList();
  if (!list) return null;
  const { moveItemBy, canMoveItemBy } = list;
  return (
    <>
      {canMoveItemBy(id, -1) && <button onClick={() => moveItemBy(id, -1)}>Move {id} up</button>}
      {canMoveItemBy(id, 1) && <button onClick={() => moveItemBy(id, 1)}>Move {id} down</button>}
    </>
  );
}

function renderList(ids: string[], onReorder: (ids: string[]) => void, disabled = false) {
  return render(
    <SortableList
      ids={ids}
      onReorder={onReorder}
      nameOf={(id) => `Project ${id}`}
      disabled={disabled}
    >
      {ids.map((id) => (
        <SortableItem key={id} id={id}>
          {({ handleProps }) => (
            <div {...handleProps}>
              <span>Item {id}</span>
              <MoveControls id={id} />
            </div>
          )}
        </SortableItem>
      ))}
    </SortableList>,
  );
}

describe("SortableList", () => {
  it("reports the whole new order when an item moves", () => {
    const onReorder = vi.fn();
    renderList(["a", "b", "c"], onReorder);

    fireEvent.click(screen.getByRole("button", { name: "Move c up" }));

    expect(onReorder).toHaveBeenCalledWith(["a", "c", "b"]);
  });

  it("offers only the moves an item can actually make", () => {
    renderList(["a", "b"], vi.fn());

    expect(screen.queryByRole("button", { name: "Move a up" })).toBeNull();
    expect(screen.getByRole("button", { name: "Move a down" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Move b up" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Move b down" })).toBeNull();
  });

  it("says where a moved item landed, so a move made without sight is not silent", () => {
    const { container } = renderList(["a", "b", "c"], vi.fn());

    fireEvent.click(screen.getByRole("button", { name: "Move a down" }));

    const announcement = container.querySelector(`[data-slot="${ANNOUNCEMENT_SLOT}"]`);
    expect(announcement?.textContent).toBe("Project a moved to 2 of 3.");
  });

  // `disabled` is the caller saying the list on screen is not the whole list. It has to stop every
  // way of moving an item, not just the drag — an order arranged from a partial view would be
  // filed as though it were the user's answer for all of it.
  it("withholds every move while the list is disabled", () => {
    const onReorder = vi.fn();
    renderList(["a", "b", "c"], onReorder, true);

    expect(screen.queryByRole("button", { name: "Move b up" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Move b down" })).toBeNull();
    expect(onReorder).not.toHaveBeenCalled();
  });

  it("refuses a move driven straight from the list actions while disabled", () => {
    const onReorder = vi.fn();
    function AlwaysMove() {
      const { moveItemBy } = useSortableList() ?? {};
      return <button onClick={() => moveItemBy?.("c", -1)}>force</button>;
    }
    render(
      <SortableList ids={["a", "b", "c"]} onReorder={onReorder} disabled>
        <AlwaysMove />
      </SortableList>,
    );

    fireEvent.click(screen.getByRole("button", { name: "force" }));

    expect(onReorder).not.toHaveBeenCalled();
  });

  it("hands a consumer outside a list nothing, rather than throwing at it", () => {
    function Standalone() {
      const list = useSortableList();
      return <span>{list === null ? "no list" : "in a list"}</span>;
    }

    expect(() => render(<Standalone />)).not.toThrow();
    expect(screen.getByText("no list")).toBeTruthy();
  });

  it("gives plain children the drag itself, so a simple list needs no handle plumbing", () => {
    render(
      <SortableList ids={["a"]} onReorder={vi.fn()}>
        <SortableItem id="a">
          <span>plain</span>
        </SortableItem>
      </SortableList>,
    );

    expect(screen.getByText("plain")).toBeTruthy();
  });
});
