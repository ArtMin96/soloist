// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { HotkeysPanel } from "@/components/settings/HotkeysPanel";
import { HotkeysContext, type HotkeysState } from "@/store/hotkeysContext";
import type { Binding, HotkeyBindingView } from "@/domain";

const CTRL_K: Binding = { ctrl: true, alt: false, shift: false, super: false, key: "K" };

function view(
  over: Partial<HotkeyBindingView> & Pick<HotkeyBindingView, "action" | "scope">,
): HotkeyBindingView {
  return { binding: CTRL_K, is_default: true, conflict: false, ...over };
}

function renderPanel(bindings: HotkeyBindingView[], over: Partial<HotkeysState> = {}) {
  const state: HotkeysState = {
    bindings,
    remap: vi.fn(),
    disable: vi.fn(),
    reset: vi.fn(),
    resetAll: vi.fn(),
    ...over,
  };
  render(
    <HotkeysContext value={state}>
      <HotkeysPanel />
    </HotkeysContext>,
  );
  return state;
}

afterEach(cleanup);

describe("Settings — Hotkeys", () => {
  it("renders bindings grouped by scope with their chord", () => {
    renderPanel([view({ action: "open_command_palette", scope: "general" })]);
    expect(screen.getByText("General")).toBeTruthy();
    expect(screen.getByText("Open command palette")).toBeTruthy();
    expect(screen.getByText("Ctrl")).toBeTruthy();
    expect(screen.getByText("K")).toBeTruthy();
  });

  it("captures a pressed chord on click and remaps the action", () => {
    const state = renderPanel([view({ action: "open_command_palette", scope: "general" })]);

    fireEvent.click(screen.getByRole("button", { name: "Change Open command palette shortcut" }));
    fireEvent.keyDown(window, { key: "j", ctrlKey: true });

    expect(state.remap).toHaveBeenCalledWith("open_command_palette", {
      ctrl: true,
      alt: false,
      shift: false,
      super: false,
      key: "J",
    });
  });

  it("disables a binding through its × control", () => {
    const state = renderPanel([view({ action: "open_command_palette", scope: "general" })]);
    fireEvent.click(screen.getByRole("button", { name: "Disable Open command palette shortcut" }));
    expect(state.disable).toHaveBeenCalledWith("open_command_palette");
  });

  it("resets all to defaults", () => {
    const state = renderPanel([view({ action: "open_command_palette", scope: "general" })]);
    fireEvent.click(screen.getByRole("button", { name: "Reset all" }));
    expect(state.resetAll).toHaveBeenCalled();
  });

  it("flags a within-scope conflict", () => {
    renderPanel([view({ action: "open_command_palette", scope: "general", conflict: true })]);
    expect(screen.getByText("Conflict")).toBeTruthy();
  });

  it("filters the keymap by the search query", () => {
    renderPanel([
      view({ action: "open_command_palette", scope: "general" }),
      view({
        action: "restart_selection",
        scope: "sidebar",
        binding: { ctrl: false, alt: false, shift: false, super: false, key: "R" },
      }),
    ]);

    fireEvent.change(screen.getByLabelText("Search shortcuts"), { target: { value: "restart" } });
    expect(screen.queryByText("Open command palette")).toBeNull();
    expect(screen.getByText("Restart")).toBeTruthy();
  });
});
