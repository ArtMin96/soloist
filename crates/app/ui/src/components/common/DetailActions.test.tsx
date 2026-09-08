// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Link2, Pencil } from "lucide-react";
import { DetailActions, type DetailAction } from "@/components/common/DetailActions";
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

const menuTrigger = () => screen.getByRole("button", { name: "More todo actions" });

function openMenu() {
  fireEvent.pointerDown(menuTrigger(), { button: 0, ctrlKey: false });
}

const separator = () => document.querySelector('[data-slot="separator"]');

describe("DetailActions", () => {
  it("hands secondary actions from the menu to the inline cluster at 15rem", () => {
    cluster();

    const inline = screen.getByRole("button", { name: "Edit" }).parentElement as HTMLElement;

    expect(inline.classList.contains("hidden")).toBe(true);
    expect(inline.classList.contains("@min-[15rem]/detail-header:flex")).toBe(true);
    expect(menuTrigger().classList.contains("@min-[15rem]/detail-header:hidden")).toBe(true);
  });

  it("uses the subject-specific tooltip as the overflow control's name", async () => {
    cluster();

    const trigger = menuTrigger();
    fireEvent.focus(trigger);

    expect((await screen.findByRole("tooltip")).textContent).toBe("More todo actions");
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

    const rule = separator() as HTMLElement;
    expect(rule).not.toBeNull();
    expect(rule.className).toContain("self-center");
    expect(rule.parentElement?.className).toContain("items-center");
  });

  it("draws no rule when the subject offers no primary action", () => {
    cluster();

    // A rule with nothing after it points at nothing.
    expect(separator()).toBeNull();
  });

  it("presents every secondary action as an equally compact named control", async () => {
    cluster();

    const edit = screen.getByRole("button", { name: "Edit" });
    const copy = screen.getByRole("button", { name: "Copy link to todo" });
    expect(edit.textContent).toBe("");
    expect(copy.textContent).toBe("");
    expect(edit.className.split(/\s+/)).toContain("text-icon-muted");
    expect(edit.className.split(/\s+/)).toContain("hover:text-toolbar-control-foreground");

    fireEvent.focus(edit);

    expect((await screen.findByRole("tooltip")).textContent).toBe("Edit");

    fireEvent.focus(copy);

    expect((await screen.findByRole("tooltip")).textContent).toBe("Copy link to todo");
  });

  it("refuses an action the subject cannot take right now, in both forms", async () => {
    const onSelect = vi.fn();
    cluster({ actions: [action({ onSelect, disabled: true })] });

    const inline = screen.getByRole("button", { name: "Edit" }) as HTMLButtonElement;
    expect(inline.disabled).toBe(true);
    expect(inline.className.split(/\s+/)).toEqual(
      expect.arrayContaining([
        "disabled:border-border",
        "disabled:bg-muted",
        "disabled:text-muted-foreground",
      ]),
    );
    expect(inline.className.split(/\s+/)).not.toContain("disabled:opacity-50");

    openMenu();

    const item = await screen.findByRole("menuitem", { name: "Edit todo" });
    expect(item.className.split(/\s+/)).toEqual(
      expect.arrayContaining(["data-disabled:opacity-100", "data-disabled:text-text-muted"]),
    );
    fireEvent.click(item);
    expect(onSelect).not.toHaveBeenCalled();
  });
});
