// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TodoToolbar } from "@/components/orchestration/TodoToolbar";
import { EMPTY_TODO_FILTER, type TodoFilter } from "@/store/todoFilter";

afterEach(cleanup);

function toolbar(overrides: Partial<React.ComponentProps<typeof TodoToolbar>> = {}) {
  const onChange = vi.fn();
  const onViewChange = vi.fn();
  render(
    <TodoToolbar
      filter={EMPTY_TODO_FILTER}
      tags={[]}
      onChange={onChange}
      view="grouped"
      onViewChange={onViewChange}
      shown={2}
      total={5}
      {...overrides}
    />,
  );
  return { onChange, onViewChange };
}

describe("TodoToolbar", () => {
  it("reports how many todos survive the filter, not the total", () => {
    toolbar({ shown: 2, total: 5 });
    expect(document.querySelector("[data-todo-count]")?.textContent).toBe("2 of 5");
  });

  it("reports the other view when the group control switches", () => {
    const { onViewChange } = toolbar({ view: "grouped" });
    fireEvent.click(screen.getByRole("radio", { name: "All" }));
    expect(onViewChange).toHaveBeenCalledWith("all");
  });

  it("emits the filter with only the search facet changed", () => {
    const { onChange } = toolbar();
    fireEvent.change(screen.getByRole("searchbox", { name: "Search todos" }), {
      target: { value: "ship" },
    });
    expect(onChange).toHaveBeenCalledWith({ ...EMPTY_TODO_FILTER, search: "ship" });
  });

  it("emits the filter with only the status facet changed", () => {
    const filter: TodoFilter = { ...EMPTY_TODO_FILTER, search: "ship" };
    const { onChange } = toolbar({ filter });
    fireEvent.click(screen.getByRole("combobox", { name: "Filter by status" }));
    fireEvent.click(screen.getByRole("option", { name: "Done" }));
    expect(onChange).toHaveBeenCalledWith({ ...filter, status: "done" });
  });

  it("emits the filter with only the tag facet changed", () => {
    const filter: TodoFilter = { ...EMPTY_TODO_FILTER, search: "ship" };
    const { onChange } = toolbar({ filter, tags: ["release"] });
    fireEvent.click(screen.getByRole("button", { name: "release" }));
    expect(onChange).toHaveBeenCalledWith({ ...filter, tag: "release" });
  });

  it("exposes exactly one search, one status select, and one group control", () => {
    toolbar();
    expect(screen.getAllByRole("searchbox", { name: "Search todos" })).toHaveLength(1);
    expect(screen.getAllByRole("combobox", { name: "Filter by status" })).toHaveLength(1);
    expect(screen.getAllByRole("group", { name: "Group todos" })).toHaveLength(1);
  });

  it("hides the primary action when there is no onCreate", () => {
    toolbar({ onCreate: undefined });
    expect(screen.queryByRole("button", { name: /New todo/ })).toBeNull();
  });

  it("offers New todo when onCreate is given", () => {
    const onCreate = vi.fn();
    toolbar({ onCreate });
    fireEvent.click(screen.getByRole("button", { name: /New todo/ }));
    expect(onCreate).toHaveBeenCalled();
  });
});
