// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SortableItem, SortableList, useSortableList } from "@/components/ui/sortable-list";

afterEach(cleanup);

// A consumer of the list's move actions — the shape any pointer-free affordance takes.
function MoveControls({ id }: { id: string }) {
  const { moveItemBy, canMoveItemBy } = useSortableList();
  return (
    <>
      {canMoveItemBy(id, -1) && (
        <button onClick={() => moveItemBy(id, -1)}>Move {id} up</button>
      )}
      {canMoveItemBy(id, 1) && (
        <button onClick={() => moveItemBy(id, 1)}>Move {id} down</button>
      )}
    </>
  );
}

function renderList(ids: string[], onReorder: (ids: string[]) => void) {
  return render(
    <SortableList ids={ids} onReorder={onReorder} nameOf={(id) => `Project ${id}`}>
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

    const announcement = container.querySelector('[data-slot="sortable-announcement"]');
    expect(announcement?.textContent).toBe("Project a moved to 2 of 3.");
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
