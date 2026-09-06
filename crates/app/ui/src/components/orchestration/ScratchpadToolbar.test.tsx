// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { BOARD_COUNT_ATTRIBUTE } from "@/components/common/BoardToolbar";
import { ScratchpadToolbar } from "@/components/orchestration/ScratchpadToolbar";
import { EMPTY_SCRATCHPAD_FILTER, type ScratchpadFilter } from "@/store/scratchpadFilter";

afterEach(cleanup);

function toolbar(overrides: Partial<React.ComponentProps<typeof ScratchpadToolbar>> = {}) {
  const onChange = vi.fn();
  const onSortChange = vi.fn();
  render(
    <ScratchpadToolbar
      filter={EMPTY_SCRATCHPAD_FILTER}
      tags={[]}
      onChange={onChange}
      sort="updated"
      onSortChange={onSortChange}
      shown={2}
      total={5}
      {...overrides}
    />,
  );
  return { onChange, onSortChange };
}

describe("ScratchpadToolbar", () => {
  it("reports how many scratchpads survive the filter, not the total", () => {
    toolbar({ shown: 2, total: 5 });
    expect(document.querySelector(`[${BOARD_COUNT_ATTRIBUTE}]`)?.textContent).toBe("2 of 5");
  });

  it("emits the filter with only the search facet changed", () => {
    const { onChange } = toolbar();
    fireEvent.change(screen.getByRole("searchbox", { name: "Search scratchpads" }), {
      target: { value: "editor" },
    });
    expect(onChange).toHaveBeenCalledWith({ ...EMPTY_SCRATCHPAD_FILTER, search: "editor" });
  });

  it("emits the filter with only the archived facet changed", () => {
    const filter: ScratchpadFilter = { ...EMPTY_SCRATCHPAD_FILTER, search: "editor" };
    const { onChange } = toolbar({ filter });
    fireEvent.click(screen.getByRole("combobox", { name: "Filter by archived state" }));
    fireEvent.click(screen.getByRole("option", { name: "Archived" }));
    expect(onChange).toHaveBeenCalledWith({ ...filter, archived: "archived" });
  });

  it("emits the filter with only the tag facet changed", () => {
    const filter: ScratchpadFilter = { ...EMPTY_SCRATCHPAD_FILTER, search: "editor" };
    const { onChange } = toolbar({ filter, tags: ["design"] });
    fireEvent.click(screen.getByRole("button", { name: "design" }));
    expect(onChange).toHaveBeenCalledWith({ ...filter, tag: "design" });
  });

  it("reports the chosen order when the sort changes, leaving the filter alone", () => {
    const { onSortChange, onChange } = toolbar();
    fireEvent.click(screen.getByRole("combobox", { name: "Sort scratchpads" }));
    fireEvent.click(screen.getByRole("option", { name: "Name" }));
    expect(onSortChange).toHaveBeenCalledWith("name");
    expect(onChange).not.toHaveBeenCalled();
  });

  it("hides the primary action when there is no onCreate", () => {
    toolbar({ onCreate: undefined });
    expect(screen.queryByRole("button", { name: /New scratchpad/ })).toBeNull();
  });

  it("offers New scratchpad when onCreate is given", () => {
    const onCreate = vi.fn();
    toolbar({ onCreate });
    fireEvent.click(screen.getByRole("button", { name: /New scratchpad/ }));
    expect(onCreate).toHaveBeenCalled();
  });
});
