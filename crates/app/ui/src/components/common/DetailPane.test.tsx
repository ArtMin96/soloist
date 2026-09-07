// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { DetailBackButton } from "@/components/common/DetailPane";

afterEach(cleanup);

describe("DetailBackButton", () => {
  it("stays inside the detail content edge", () => {
    render(<DetailBackButton destination="todos" onClick={vi.fn()} />);

    expect(screen.getByRole("button", { name: "Back to todos" }).className).not.toContain("-ml-");
  });
});
