// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  CARD_ROW_ATTRIBUTE,
  CARD_TRIGGER_ATTRIBUTE,
  CardRow,
  CardRowStandIn,
} from "@/components/common/CardRow";

afterEach(cleanup);

function row(overrides: Partial<React.ComponentProps<typeof CardRow>> = {}) {
  const onOpen = vi.fn();
  render(<CardRow onOpen={onOpen} children={<span>Release plan</span>} {...overrides} />);
  return { onOpen };
}

function trigger(): HTMLElement {
  return document.querySelector(`[${CARD_TRIGGER_ATTRIBUTE}]`) as HTMLElement;
}

describe("CardRow", () => {
  it("keeps the row's own button free of any interactive descendant", () => {
    row({ aside: <button type="button">Open worker-a terminal</button> });

    expect(trigger().querySelector('button, [role="button"], a[href]')).toBeNull();
  });

  it("offers the aside as its own tab stop beside the trigger, not inside it", () => {
    row({ aside: <button type="button">Open worker-a terminal</button> });

    const aside = screen.getByRole("button", { name: "Open worker-a terminal" });
    expect(aside.tagName).toBe("BUTTON");
    expect(aside.tabIndex).not.toBe(-1);
    expect(trigger().contains(aside)).toBe(false);
  });

  it("opens the row when its trigger is activated", () => {
    const { onOpen } = row();

    fireEvent.click(trigger());

    expect(onOpen).toHaveBeenCalled();
  });

  it("leaves the row closed when the aside beside it is activated", () => {
    const onAside = vi.fn();
    const { onOpen } = row({
      aside: (
        <button type="button" onClick={onAside}>
          Open worker-a terminal
        </button>
      ),
    });

    fireEvent.click(screen.getByRole("button", { name: "Open worker-a terminal" }));

    expect(onAside).toHaveBeenCalled();
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("uses one flush, geometry-stable surface for the row's action", () => {
    row();

    const card = document.querySelector(`[${CARD_ROW_ATTRIBUTE}]`) as HTMLElement;
    const content = document.querySelector('[data-slot="card-content"]') as HTMLElement;
    expect(card.getAttribute("data-slot")).toBe("card");
    expect(card.className).toContain("border-border");
    expect(card.className).toContain("ring-0");
    expect(content.className).toContain("p-0");
    expect(content.className).toContain("gap-0");
    expect(content.className).toContain("items-center");
    expect(trigger().className).toContain("rounded-none");
    expect(trigger().className).toContain("p-3");
    expect(trigger().className).toContain("cursor-pointer");
    expect(trigger().className).toContain("hover:bg-sidebar-row-hover");
    expect(trigger().className).toContain("active:bg-sidebar-row-active");
    expect(trigger().className).toContain("hover:[box-shadow:none]");
    expect(trigger().className).not.toContain("hover:[box-shadow:var(--glass-control-shadow)]");
    expect(trigger().className).toContain("supports-backdrop-filter:hover:backdrop-blur-none");
    expect(trigger().className).toContain("active:not-aria-[haspopup]:scale-100");
    expect(trigger().className).toContain("focus-visible:ring-inset");
    expect(trigger().className).toContain("motion-reduce:transition-none");
  });
});

describe("CardRowStandIn", () => {
  it("draws the row's box with nothing in it to activate", () => {
    render(
      <CardRowStandIn>
        <span>stand-in</span>
      </CardRowStandIn>,
    );

    expect(document.querySelector('[data-slot="card"]')).not.toBeNull();
    expect(screen.getByText("stand-in")).toBeTruthy();
    expect(document.querySelector('[data-slot="card-content"]')?.className).toContain("p-3");
    expect(document.querySelector("button")).toBeNull();
    expect(document.querySelector(`[${CARD_TRIGGER_ATTRIBUTE}]`)).toBeNull();
  });
});
