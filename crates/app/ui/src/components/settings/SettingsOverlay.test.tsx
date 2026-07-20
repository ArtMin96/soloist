// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SettingsOverlay } from "@/components/settings/SettingsOverlay";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { AppearanceProvider } from "@/store/AppearanceProvider";
import type { Appearance } from "@/domain";

// Stand in for the settings backend: serve an initial appearance and echo what a setter saves
// (the facade auto-saves and returns the stored value), capturing it for assertions.
function mockSettings(initial: Appearance, onSave?: (next: Appearance) => void) {
  mockIPC((cmd, args) => {
    if (cmd === "appearance") return initial;
    if (cmd === "set_appearance") {
      const next = (args as { appearance: Appearance }).appearance;
      onSave?.(next);
      return next;
    }
    return undefined;
  });
}

function renderSettings() {
  render(
    <AppearanceProvider>
      <SettingsOverlay open onOpenChange={() => {}} project={null} />
    </AppearanceProvider>,
  );
}

afterEach(() => {
  cleanup();
  clearMocks();
  document.documentElement.classList.remove("dark");
  window.localStorage?.clear();
});

describe("Settings — Appearance", () => {
  it("applies the stored theme to the document root", async () => {
    mockSettings({ ...DEFAULT_APPEARANCE, theme: "dark" });
    renderSettings();

    await waitFor(() => expect(document.documentElement.classList.contains("dark")).toBe(true));
  });

  it("persists a theme change and restyles the app immediately", async () => {
    let saved: Appearance | null = null;
    mockSettings({ ...DEFAULT_APPEARANCE, theme: "dark" }, (next) => {
      saved = next;
    });
    renderSettings();
    await waitFor(() => expect(document.documentElement.classList.contains("dark")).toBe(true));

    // The Appearance tab is selected by default; choosing Light writes the document and the
    // root sheds the dark class without a reload.
    fireEvent.click(screen.getByText("Light"));

    await waitFor(() => expect(saved?.theme).toBe("light"));
    await waitFor(() => expect(document.documentElement.classList.contains("dark")).toBe(false));
  });

  it("stubs an undefined tab with a to-be-defined state, inventing no fields", async () => {
    mockSettings(DEFAULT_APPEARANCE);
    renderSettings();

    fireEvent.click(screen.getByRole("tab", { name: "Account" }));

    expect(screen.getByText(/have not been defined yet/i)).toBeTruthy();
  });

  it("moves the selection with arrow keys so the rail is keyboard-operable", async () => {
    mockSettings(DEFAULT_APPEARANCE);
    renderSettings();

    const appearanceTab = screen.getByRole("tab", { name: "Appearance" });
    expect(appearanceTab.getAttribute("aria-selected")).toBe("true");

    fireEvent.keyDown(appearanceTab, { key: "ArrowDown" });
    expect(screen.getByRole("tab", { name: "Sidebar" }).getAttribute("aria-selected")).toBe("true");
    expect(appearanceTab.getAttribute("aria-selected")).toBe("false");

    fireEvent.keyDown(screen.getByRole("tab", { name: "Sidebar" }), { key: "Home" });
    expect(screen.getByRole("tab", { name: "Appearance" }).getAttribute("aria-selected")).toBe(
      "true",
    );
  });
});
