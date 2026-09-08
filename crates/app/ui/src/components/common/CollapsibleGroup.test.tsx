// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CollapsibleGroup } from "@/components/common/CollapsibleGroup";

afterEach(cleanup);

describe("CollapsibleGroup", () => {
  it("presents the group name and item count as one clear disclosure", () => {
    const onOpenChange = vi.fn();
    render(
      <CollapsibleGroup label="Release plan" count={3} open={false} onOpenChange={onOpenChange}>
        <span>Ship the release</span>
      </CollapsibleGroup>,
    );

    const trigger = screen.getByRole("button", { name: "Release plan, 3 items" });
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    expect(trigger.querySelector('[data-slot="badge"]')?.textContent).toBe("3");

    fireEvent.click(trigger);

    expect(onOpenChange).toHaveBeenCalledWith(true);
  });

  it("uses singular count copy and exposes expanded content", () => {
    render(
      <CollapsibleGroup label="Inbox" count={1} open onOpenChange={vi.fn()}>
        <span>Review request</span>
      </CollapsibleGroup>,
    );

    expect(
      screen.getByRole("button", { name: "Inbox, 1 item" }).getAttribute("aria-expanded"),
    ).toBe("true");
    expect(screen.getByText("Review request")).toBeTruthy();
  });

  it("keeps the disclosure label and icon paired with the ghost control hover surface", () => {
    render(
      <CollapsibleGroup label="Inbox" count={1} open={false} onOpenChange={vi.fn()}>
        <span>Review request</span>
      </CollapsibleGroup>,
    );

    const trigger = screen.getByRole("button", { name: "Inbox, 1 item" });
    const icon = trigger.querySelector("svg") as SVGElement;
    const label = screen.getByText("Inbox");
    expect(trigger.className.split(/\s+/)).toContain("hover:text-toolbar-control-foreground");
    expect(icon.getAttribute("class")?.split(/\s+/)).toContain(
      "group-hover/collapsible-group:text-toolbar-control-foreground",
    );
    expect(label.className.split(/\s+/)).toContain(
      "group-hover/collapsible-group:text-toolbar-control-foreground",
    );
  });
});
