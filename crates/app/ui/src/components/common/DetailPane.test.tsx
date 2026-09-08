// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { DetailBackButton, DetailNotice } from "@/components/common/DetailPane";

afterEach(cleanup);

describe("DetailBackButton", () => {
  it("stays inside the detail content edge", () => {
    render(<DetailBackButton destination="todos" onClick={vi.fn()} />);

    const back = screen.getByRole("button", { name: "Back to todos" });
    expect(back.className).not.toContain("-ml-");
    expect(back.className.split(/\s+/)).toContain("text-secondary-label");
    expect(back.className.split(/\s+/)).toContain("hover:text-toolbar-control-foreground");
  });
});

describe("DetailNotice", () => {
  it("uses the error surface and its paired foreground without translucent text", () => {
    render(<DetailNotice message="Could not save todo." />);

    const alert = screen.getByRole("alert");
    const description = screen.getByText("Could not save todo.");
    expect(alert.className.split(/\s+/)).toEqual(
      expect.arrayContaining([
        "border-error",
        "bg-error-surface",
        "text-error-foreground",
        "*:data-[slot=alert-description]:text-error-foreground",
      ]),
    );
    expect(description.className.split(/\s+/)).not.toContain("text-destructive/90");
  });
});
