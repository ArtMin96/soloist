// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ScratchpadTitle } from "@/components/orchestration/ScratchpadTitle";

afterEach(cleanup);

/** Opens the rename field by clicking the resting title, and returns it. */
function startRename(): HTMLInputElement {
  fireEvent.click(screen.getByRole("button"));
  return screen.getByRole("textbox", { name: "Scratchpad name" }) as HTMLInputElement;
}

describe("ScratchpadTitle", () => {
  it("rests as the humanized title and edits the raw handle", () => {
    render(<ScratchpadTitle name="rich-editor-design" onRename={vi.fn()} />);
    expect(screen.getByRole("button").textContent).toBe("Rich editor design");
    expect(startRename().value).toBe("rich-editor-design");
  });

  it("commits on Enter", async () => {
    const onRename = vi.fn().mockResolvedValue(undefined);
    render(<ScratchpadTitle name="release-plan" onRename={onRename} />);
    const field = startRename();

    fireEvent.change(field, { target: { value: "Release plan" } });
    fireEvent.keyDown(field, { key: "Enter" });

    await waitFor(() => expect(onRename).toHaveBeenCalledWith("Release plan"));
  });

  it("cancels on Escape, restoring the title and writing nothing", () => {
    const onRename = vi.fn();
    render(<ScratchpadTitle name="release-plan" onRename={onRename} />);
    const field = startRename();

    fireEvent.change(field, { target: { value: "something else" } });
    fireEvent.keyDown(field, { key: "Escape" });

    expect(onRename).not.toHaveBeenCalled();
    expect(screen.getByRole("button").textContent).toBe("Release plan");
  });

  it("keeps the typed name and names the refusal when the core rejects it", async () => {
    const onRename = vi.fn().mockRejectedValue("a scratchpad named that already exists");
    render(<ScratchpadTitle name="release-plan" onRename={onRename} />);
    const field = startRename();

    fireEvent.change(field, { target: { value: "research" } });
    fireEvent.keyDown(field, { key: "Enter" });

    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("already exists"));
    // The field is still open, still holding what the user typed, and still the focused element —
    // nothing was lost to the error and the correction can be typed straight away.
    const after = screen.getByRole("textbox", { name: "Scratchpad name" });
    expect(after).toHaveProperty("value", "research");
    expect(document.activeElement).toBe(after);
    expect(after.getAttribute("aria-invalid")).toBe("true");
  });

  it("treats an unchanged or blank name as nothing to do", () => {
    const onRename = vi.fn();
    render(<ScratchpadTitle name="release-plan" onRename={onRename} />);

    fireEvent.keyDown(startRename(), { key: "Enter" });
    expect(onRename).not.toHaveBeenCalled();

    const field = startRename();
    fireEvent.change(field, { target: { value: "   " } });
    fireEvent.keyDown(field, { key: "Enter" });
    expect(onRename).not.toHaveBeenCalled();
  });

  it("commits once when the blur follows the Enter that already committed", async () => {
    const onRename = vi.fn().mockResolvedValue(undefined);
    render(<ScratchpadTitle name="release-plan" onRename={onRename} />);
    const field = startRename();

    fireEvent.change(field, { target: { value: "shipping" } });
    fireEvent.keyDown(field, { key: "Enter" });
    fireEvent.blur(field);

    await waitFor(() => expect(onRename).toHaveBeenCalledTimes(1));
  });
});
