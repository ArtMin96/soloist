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
});
