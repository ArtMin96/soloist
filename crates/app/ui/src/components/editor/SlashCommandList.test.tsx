// @vitest-environment jsdom
import { createRef } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import {
  SlashCommandList,
  type SlashCommandListHandle,
} from "@/components/editor/SlashCommandList";
import type { SlashItem } from "@/components/editor/slashItems";

function item(title: string): SlashItem {
  return { title, hint: `${title} hint`, keywords: [], run: () => {} };
}

const HEADINGS = [item("Heading 1"), item("Heading 2"), item("Heading 3")];
const LISTS = [item("Bullet list"), item("Ordered list")];

afterEach(cleanup);

// Drives the menu the way the suggestion plugin does: props pushed in, keys forwarded to the handle.
function renderMenu(items: SlashItem[], query: string) {
  const ref = createRef<SlashCommandListHandle>();
  const command = vi.fn();
  const view = render(<SlashCommandList ref={ref} items={items} command={command} query={query} />);
  const press = (key: string) =>
    act(() => void ref.current?.onKeyDown({ event: { key } as KeyboardEvent }));
  const rerender = (nextItems: SlashItem[], nextQuery: string) =>
    view.rerender(
      <SlashCommandList ref={ref} items={nextItems} command={command} query={nextQuery} />,
    );
  const highlighted = () =>
    screen.getByRole("option", { selected: true }).querySelector(".slash-item-title")?.textContent;
  return { ref, command, press, rerender, highlighted };
}

describe("SlashCommandList", () => {
  it("walks the highlight with the arrows, wrapping at both ends", () => {
    const { press, highlighted } = renderMenu(HEADINGS, "h");

    expect(highlighted()).toBe("Heading 1");
    press("ArrowDown");
    expect(highlighted()).toBe("Heading 2");
    press("ArrowUp");
    press("ArrowUp");
    expect(highlighted()).toBe("Heading 3");
  });

  it("runs the highlighted item on Enter", () => {
    const { press, command } = renderMenu(HEADINGS, "h");

    press("ArrowDown");
    press("Enter");

    expect(command).toHaveBeenCalledWith(HEADINGS[1]);
  });

  it("drops the highlight back to the first row when a new query re-filters the list", () => {
    const { press, rerender, highlighted } = renderMenu(HEADINGS, "h");

    press("ArrowDown");
    press("ArrowDown");
    expect(highlighted()).toBe("Heading 3");

    rerender(LISTS, "list");

    expect(highlighted()).toBe("Bullet list");
  });

  // The remount that resets the highlight also swaps the imperative handle. If the new one did not
  // reach the plugin, arrows and Enter would go dead the moment the user typed another character.
  it("keeps driving the new list after a re-filter", () => {
    const { press, rerender, command, highlighted } = renderMenu(HEADINGS, "h");

    press("ArrowDown");
    rerender(LISTS, "list");

    press("ArrowDown");
    expect(highlighted()).toBe("Ordered list");

    press("Enter");
    expect(command).toHaveBeenCalledWith(LISTS[1]);
  });

  // Reset and new items land in the same commit, so Enter can never pair a stale index with the
  // freshly filtered list.
  it("runs the first item of the new list on an Enter straight after a re-filter", () => {
    const { press, rerender, command } = renderMenu(HEADINGS, "h");

    press("ArrowDown");
    press("ArrowDown");
    rerender(LISTS, "list");
    press("Enter");

    expect(command).toHaveBeenCalledWith(LISTS[0]);
  });

  it("reports keys unhandled when nothing matched, so the plugin keeps them", () => {
    const { ref } = renderMenu([], "zzz");

    expect(ref.current?.onKeyDown({ event: { key: "Enter" } as KeyboardEvent })).toBe(false);
    expect(screen.getByText("No matching blocks")).toBeTruthy();
  });
});
