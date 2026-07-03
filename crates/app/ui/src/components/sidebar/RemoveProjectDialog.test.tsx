// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { RemoveProjectDialog } from "@/components/sidebar/RemoveProjectDialog";

afterEach(cleanup);

function renderDialog(overrides: Partial<Parameters<typeof RemoveProjectDialog>[0]> = {}) {
  const onOpenChange = vi.fn();
  const onConfirm = vi.fn();
  render(
    <RemoveProjectDialog
      open
      onOpenChange={onOpenChange}
      projectName="Storefront"
      runningCount={2}
      onConfirm={onConfirm}
      {...overrides}
    />,
  );
  return { onOpenChange, onConfirm };
}

describe("RemoveProjectDialog", () => {
  it("names the project and states every consequence, including the disk guarantee", () => {
    renderDialog();
    expect(screen.getByRole("heading", { name: "Remove “Storefront”?" })).toBeTruthy();
    // The grouped well: what stops, what is forgotten, and that no file is touched.
    expect(screen.getByText("its 2 running processes")).toBeTruthy();
    expect(
      screen.getByText("trust decisions, todos, scratchpads, and project settings"),
    ).toBeTruthy();
    expect(screen.getByText("the project folder and solo.yml on disk, untouched")).toBeTruthy();
  });

  it("omits the stop row when nothing is running", () => {
    renderDialog({ runningCount: 0 });
    expect(screen.queryByText("Stops")).toBeNull();
    expect(
      screen.getByText("trust decisions, todos, scratchpads, and project settings"),
    ).toBeTruthy();
  });

  it("only the destructive action confirms; it also closes the dialog", () => {
    const { onOpenChange, onConfirm } = renderDialog();
    fireEvent.click(screen.getByRole("button", { name: "Remove project" }));
    expect(onConfirm).toHaveBeenCalledOnce();
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("cancelling closes without removing", () => {
    const { onOpenChange, onConfirm } = renderDialog();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onConfirm).not.toHaveBeenCalled();
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });
});
