// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Link2, Pencil } from "lucide-react";
import {
  DetailActions,
  MENU_TRIGGER_LABEL,
  type DetailAction,
} from "@/components/common/DetailActions";
import { INLINE_ABOVE, MENU_BELOW } from "@/components/common/DetailPane";
import { Button } from "@/components/ui/button";
import { TooltipProvider } from "@/components/ui/tooltip";

afterEach(cleanup);

function action(overrides: Partial<DetailAction> = {}): DetailAction {
  return {
    icon: Pencil,
    label: "Edit",
    menuLabel: "Edit todo",
    onSelect: vi.fn(),
    ...overrides,
  };
}

const copyLink = (overrides: Partial<DetailAction> = {}) =>
  action({
    icon: Link2,
    label: "Copy link to todo",
    menuLabel: "Copy link to todo",
    iconOnly: true,
    ...overrides,
  });

function cluster({
  actions = [action(), copyLink()],
  primary,
}: {
  actions?: DetailAction[];
  primary?: React.ReactNode;
} = {}) {
  return render(
    <TooltipProvider delayDuration={0}>
      <DetailActions actions={actions} primary={primary} menuTooltip="More todo actions" />
    </TooltipProvider>,
  );
}

const menuTrigger = () => screen.getByRole("button", { name: MENU_TRIGGER_LABEL });

function openMenu() {
  fireEvent.pointerDown(menuTrigger(), { button: 0, ctrlKey: false });
}

const separator = () => document.querySelector('[data-slot="separator"]');

describe("DetailActions", () => {
  // Below a 15rem container the secondary actions become menu items. Both forms exist in the DOM
  // under mutually exclusive container queries, so exactly one of them is ever reachable — jsdom
  // applies no CSS, which is why this asserts the queries rather than the visibility.
  it("offers the secondary actions inline or in a menu, never as two live copies", () => {
    cluster();

    const inline = screen.getByRole("button", { name: "Edit" }).parentElement as HTMLElement;

    expect(inline.className).toContain(INLINE_ABOVE);
    expect(menuTrigger().className).toContain(MENU_BELOW);
  });

  it("names an action by its full phrase in the menu, and runs it from there", async () => {
    const onSelect = vi.fn();
    cluster({ actions: [action({ onSelect }), copyLink()] });

    openMenu();

    const item = await screen.findByRole("menuitem", { name: "Edit todo" });
    expect(await screen.findByRole("menuitem", { name: "Copy link to todo" })).toBeTruthy();

    fireEvent.click(item);
    expect(onSelect).toHaveBeenCalledTimes(1);
  });

  it("rules the secondary actions off from the primary control", () => {
    cluster({ primary: <Button size="sm">Complete</Button> });

    expect(separator()).not.toBeNull();
  });

  it("draws no rule when the subject offers no primary action", () => {
    cluster();

    // A rule with nothing after it points at nothing.
    expect(separator()).toBeNull();
  });

  it("names an icon-only action and explains it when it takes focus", async () => {
    cluster();

    const copy = screen.getByRole("button", { name: "Copy link to todo" });
    expect(copy.textContent).toBe("");

    fireEvent.focus(copy);

    expect((await screen.findByRole("tooltip")).textContent).toBe("Copy link to todo");
  });

  it("refuses an action the subject cannot take right now, in both forms", async () => {
    const onSelect = vi.fn();
    cluster({ actions: [action({ onSelect, disabled: true })] });

    expect((screen.getByRole("button", { name: "Edit" }) as HTMLButtonElement).disabled).toBe(true);

    openMenu();

    const item = await screen.findByRole("menuitem", { name: "Edit todo" });
    fireEvent.click(item);
    expect(onSelect).not.toHaveBeenCalled();
  });
});
