// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { PaneErrorBoundary } from "@/components/PaneErrorBoundary";

afterEach(cleanup);

// Throws on its first render only, so a re-mount after "Try again" succeeds — lets a test drive
// the boundary through catch and recovery without a second, differently-shaped component.
function FlakyPane({ shouldThrow }: { shouldThrow: boolean }) {
  if (shouldThrow) throw new Error("boom");
  return <div>Pane content</div>;
}

describe("PaneErrorBoundary", () => {
  it("shows a recovery notice naming the pane instead of letting the error unmount the app", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <div>
        <span>Sibling content</span>
        <PaneErrorBoundary label="Diff view">
          <FlakyPane shouldThrow />
        </PaneErrorBoundary>
      </div>,
    );

    expect(screen.getByText("Diff view ran into a problem.")).not.toBeNull();
    expect(screen.getByText("Sibling content")).not.toBeNull();
    spy.mockRestore();
  });

  it("re-mounts the pane when Try again is clicked after the fault clears", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    const { rerender } = render(
      <PaneErrorBoundary label="Diff view">
        <FlakyPane shouldThrow />
      </PaneErrorBoundary>,
    );
    expect(screen.getByText("Diff view ran into a problem.")).not.toBeNull();

    rerender(
      <PaneErrorBoundary label="Diff view">
        <FlakyPane shouldThrow={false} />
      </PaneErrorBoundary>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));

    expect(screen.getByText("Pane content")).not.toBeNull();
    expect(screen.queryByText("Diff view ran into a problem.")).toBeNull();
    spy.mockRestore();
  });
});
