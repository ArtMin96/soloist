// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { TagFilterChips } from "@/components/orchestration/TagFilterChips";

afterEach(cleanup);

/** Owns the active tag itself, so a click's round trip through `onToggle` is observable as a real
 *  DOM state change rather than a mocked callback argument. */
function Harness({ tags }: { tags: string[] }) {
  const [active, setActive] = useState<string | null>(null);
  return <TagFilterChips tags={tags} active={active} onToggle={setActive} />;
}

describe("TagFilterChips", () => {
  it("renders nothing when there are no tags", () => {
    const { container } = render(<TagFilterChips tags={[]} active={null} onToggle={vi.fn()} />);
    expect(container.firstChild).toBeNull();
  });

  it("exposes each chip inside the labelled group, reporting its pressed state", () => {
    render(<TagFilterChips tags={["alpha", "beta"]} active="beta" onToggle={vi.fn()} />);
    const group = within(screen.getByRole("group", { name: "Filter by tag" }));
    expect(group.getByRole("button", { name: "alpha" }).getAttribute("aria-pressed")).toBe("false");
    expect(group.getByRole("button", { name: "beta" }).getAttribute("aria-pressed")).toBe("true");
  });

  it("activates an inactive chip on click, and clears it on a second click", () => {
    render(<Harness tags={["alpha", "beta"]} />);
    const alpha = screen.getByRole("button", { name: "alpha" });

    fireEvent.click(alpha);
    expect(alpha.getAttribute("aria-pressed")).toBe("true");

    fireEvent.click(alpha);
    expect(alpha.getAttribute("aria-pressed")).toBe("false");
  });

  it("switches the pressed chip when a different tag is clicked", () => {
    render(<Harness tags={["alpha", "beta"]} />);
    fireEvent.click(screen.getByRole("button", { name: "alpha" }));
    fireEvent.click(screen.getByRole("button", { name: "beta" }));

    expect(screen.getByRole("button", { name: "alpha" }).getAttribute("aria-pressed")).toBe(
      "false",
    );
    expect(screen.getByRole("button", { name: "beta" }).getAttribute("aria-pressed")).toBe("true");
  });
});
