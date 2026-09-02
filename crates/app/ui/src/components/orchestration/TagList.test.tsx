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
    expect(container.querySelectorAll("[data-tag]")).toHaveLength(2);
  });

  it("renders nothing for an untagged item", () => {
    const { container } = render(<TagList tags={[]} />);
    expect(container.querySelector("[data-tags]")).toBeNull();
  });
});
