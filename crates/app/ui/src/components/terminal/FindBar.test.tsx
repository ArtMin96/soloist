// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { FindBar } from "@/components/terminal/FindBar";
import { NO_MATCHES } from "@/components/terminal/terminalSearch";

afterEach(cleanup);

function renderBar(props: Partial<Parameters<typeof FindBar>[0]> = {}) {
  const fns = {
    onChange: vi.fn(),
    onFindNext: vi.fn(),
    onFindPrevious: vi.fn(),
    onClose: vi.fn(),
  };
  render(<FindBar query="" matches={NO_MATCHES} {...fns} {...props} />);
  return fns;
}

// The tally the bar reports, read the way a user does — from the live region, not from a prop.
const tally = () => screen.getByRole("status").textContent;

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

  it("counts from one, so the first match reads as 1 and the last as the total", () => {
    renderBar({ query: "err", matches: { index: 0, count: 17 } });
    expect(tally()).toBe("1 of 17");

    cleanup();
    renderBar({ query: "err", matches: { index: 16, count: 17 } });
    expect(tally()).toBe("17 of 17");
  });

  it("says so when the query matches nothing", () => {
    renderBar({ query: "nowhere", matches: { index: -1, count: 0 } });
    expect(tally()).toBe("No results");
  });

  it("reports the total without a position once no match is current", () => {
    renderBar({ query: "e", matches: { index: -1, count: 1000 } });
    expect(tally()).toBe("1000 matches");
  });

  it("says nothing until something has been typed", () => {
    renderBar({ query: "", matches: { index: 4, count: 9 } });
    expect(tally()).toBe("");
  });

  it("keeps the tally in a live region that is present before the first count arrives", () => {
    renderBar({ query: "", matches: NO_MATCHES });
    expect(screen.getByRole("status").getAttribute("aria-live")).toBe("polite");
  });
});
