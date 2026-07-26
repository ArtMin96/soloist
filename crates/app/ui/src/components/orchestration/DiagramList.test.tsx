// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DiagramList } from "@/components/orchestration/DiagramList";
import type { DiagramSummary } from "@/domain";

afterEach(cleanup);

const diagram = (id: number, name: string, gist = ""): DiagramSummary => ({
  id,
  name,
  tags: [],
  archived: false,
  revision: 1,
  gist,
  updated_at: 0,
});

const diagrams = [
  diagram(1, "auth-flow", "the login handshake"),
  diagram(2, "data-model"),
  diagram(3, "deploy"),
];

describe("DiagramList", () => {
  it("puts the roving cursor on the selected option", () => {
    render(<DiagramList diagrams={diagrams} selected="data-model" onSelect={vi.fn()} />);
    const options = screen.getAllByRole("option");
    expect(options.map((o) => o.tabIndex)).toEqual([-1, 0, -1]);
    expect(screen.getByRole("option", { selected: true }).textContent).toContain("Data model");
  });

  it("moves the roving cursor with the arrow keys and clamps at the ends", () => {
    render(<DiagramList diagrams={diagrams} selected={null} onSelect={vi.fn()} />);
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

  it("keeps the roving cursor on the same document when the roster changes live", () => {
    const { rerender } = render(
      <DiagramList diagrams={diagrams} selected={null} onSelect={vi.fn()} />,
    );
    fireEvent.keyDown(screen.getByRole("listbox"), { key: "End" }); // cursor on "deploy" (index 2)
    expect(screen.getByRole("option", { name: /deploy/i }).tabIndex).toBe(0);
    // The first diagram is removed live: "deploy" is now index 1, but the cursor stays on it.
    rerender(<DiagramList diagrams={diagrams.slice(1)} selected={null} onSelect={vi.fn()} />);
    expect(screen.getByRole("option", { name: /deploy/i }).tabIndex).toBe(0);
    expect(screen.getAllByRole("option").map((o) => o.tabIndex)).toEqual([-1, 0]);
  });

  it("opens the clicked option by its raw handle", () => {
    const onSelect = vi.fn();
    render(<DiagramList diagrams={diagrams} selected={null} onSelect={onSelect} />);
    fireEvent.click(screen.getAllByRole("option")[0]);
    expect(onSelect).toHaveBeenCalledWith("auth-flow");
  });

  it("renders a hint (and no listbox) when there are no diagrams", () => {
    render(<DiagramList diagrams={[]} selected={null} onSelect={vi.fn()} />);
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(screen.getByText(/No diagrams yet/)).toBeTruthy();
  });
});
