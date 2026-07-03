// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ScratchpadList } from "@/components/orchestration/ScratchpadList";
import type { ScratchpadSummary } from "@/domain";

afterEach(cleanup);

const pad = (id: number, name: string, objective = ""): ScratchpadSummary => ({
  id,
  name,
  tags: [],
  archived: false,
  revision: 1,
  objective,
});

const pads = [pad(1, "plan", "the plan"), pad(2, "research"), pad(3, "risks")];

describe("ScratchpadList", () => {
  it("puts the roving cursor on the selected option", () => {
    render(<ScratchpadList scratchpads={pads} selected="research" onSelect={vi.fn()} />);
    const options = screen.getAllByRole("option");
    expect(options.map((o) => o.tabIndex)).toEqual([-1, 0, -1]);
    expect(screen.getByRole("option", { selected: true }).textContent).toContain("research");
  });

  it("moves the roving cursor with the arrow keys and clamps at the ends", () => {
    render(<ScratchpadList scratchpads={pads} selected={null} onSelect={vi.fn()} />);
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

  it("opens the clicked option", () => {
    const onSelect = vi.fn();
    render(<ScratchpadList scratchpads={pads} selected={null} onSelect={onSelect} />);
    fireEvent.click(screen.getAllByRole("option")[2]);
    expect(onSelect).toHaveBeenCalledWith("risks");
  });

  it("renders a hint (and no listbox) when there are no scratchpads", () => {
    render(<ScratchpadList scratchpads={[]} selected={null} onSelect={vi.fn()} />);
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(screen.getByText(/No scratchpads yet/)).toBeTruthy();
  });
});
