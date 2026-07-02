// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CodeBlock } from "@/components/settings/controls/CodeBlock";

afterEach(() => {
  cleanup();
});

describe("CodeBlock copy button", () => {
  it("confirms a successful copy", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });

    render(<CodeBlock copy="snippet">snippet</CodeBlock>);
    fireEvent.click(screen.getByRole("button", { name: "Copy" }));

    await waitFor(() => expect(screen.getByRole("button", { name: "Copied" })).toBeTruthy());
    expect(writeText).toHaveBeenCalledWith("snippet");
  });

  it("shows the failed state when the clipboard write rejects", async () => {
    const writeText = vi.fn().mockRejectedValue(new Error("denied"));
    Object.assign(navigator, { clipboard: { writeText } });

    render(<CodeBlock copy="snippet">snippet</CodeBlock>);
    fireEvent.click(screen.getByRole("button", { name: "Copy" }));

    await waitFor(() => expect(screen.getByRole("button", { name: "Copy failed" })).toBeTruthy());
  });

  it("shows the failed state when the clipboard API is unavailable", () => {
    Object.assign(navigator, { clipboard: undefined });

    render(<CodeBlock copy="snippet">snippet</CodeBlock>);
    fireEvent.click(screen.getByRole("button", { name: "Copy" }));

    expect(screen.getByRole("button", { name: "Copy failed" })).toBeTruthy();
  });
});
