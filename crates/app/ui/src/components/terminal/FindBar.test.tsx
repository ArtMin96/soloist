// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { FindBar } from "@/components/terminal/FindBar";

afterEach(cleanup);

function renderBar(props: Partial<Parameters<typeof FindBar>[0]> = {}) {
  const fns = {
    onChange: vi.fn(),
    onFindNext: vi.fn(),
    onFindPrevious: vi.fn(),
    onClose: vi.fn(),
  };
  render(<FindBar query="" {...fns} {...props} />);
  return fns;
}

describe("FindBar", () => {
  it("reports typed query changes", () => {
    const { onChange } = renderBar();
    fireEvent.change(screen.getByLabelText("Search query"), { target: { value: "err" } });
    expect(onChange).toHaveBeenCalledWith("err");
  });

  it("Enter finds the next match and Shift+Enter the previous", () => {
    const { onFindNext, onFindPrevious } = renderBar({ query: "err" });
    const input = screen.getByLabelText("Search query");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onFindNext).toHaveBeenCalledOnce();
    expect(onFindPrevious).not.toHaveBeenCalled();

    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
    expect(onFindPrevious).toHaveBeenCalledOnce();
  });

  it("Escape closes the bar", () => {
    const { onClose } = renderBar();
    fireEvent.keyDown(screen.getByLabelText("Search query"), { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("the toolbar buttons cycle matches and close", () => {
    const { onFindNext, onFindPrevious, onClose } = renderBar();
    fireEvent.click(screen.getByLabelText("Next match"));
    fireEvent.click(screen.getByLabelText("Previous match"));
    fireEvent.click(screen.getByLabelText("Close find"));
    expect(onFindNext).toHaveBeenCalledOnce();
    expect(onFindPrevious).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalledOnce();
  });
});
