// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { TagList } from "@/components/orchestration/TagList";

afterEach(cleanup);

describe("TagList", () => {
  it("renders every tag as its own chip", () => {
    const { container } = render(<TagList tags={["notifications", "xterm"]} />);
    expect(screen.getByText("notifications").getAttribute("data-tag")).toBe("notifications");
    expect(screen.getByText("xterm").getAttribute("data-tag")).toBe("xterm");
    expect(screen.getByText("notifications").getAttribute("title")).toBeNull();
    expect(screen.getByText("notifications").querySelector(".lucide-tag")).not.toBeNull();
    expect(container.querySelectorAll("[data-tag]")).toHaveLength(2);
  });

  it("wraps whole chips when requested instead of squeezing their text away", () => {
    const { container } = render(<TagList tags={["notifications", "xterm"]} wrap />);

    const list = container.querySelector("[data-tags]") as HTMLElement;
    expect(list.className).toContain("flex-wrap");
    expect(screen.getByText("notifications").className).toContain("shrink-0");
  });

  it("renders nothing for an untagged item", () => {
    const { container } = render(<TagList tags={[]} />);
    expect(container.querySelector("[data-tags]")).toBeNull();
  });
});
