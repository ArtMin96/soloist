// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SettingsOverlay } from "@/components/settings/SettingsOverlay";
import { DEFAULT_APPEARANCE } from "@/lib/appearance";
import { AppearanceProvider } from "@/store/AppearanceProvider";
import type { Appearance } from "@/domain";

// Stub the lazy rich editor so the overlay test never mounts TipTap — the editor is covered on its
// own, and here only the column it sits in is under test.
vi.mock("@/components/editor/LazyRichTextEditor", () => ({
  LazyRichTextEditor: (props: { ariaLabel?: string }) => <textarea aria-label={props.ariaLabel} />,
}));

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

// The one scratchpad template the layout tests drill into. Listed without a description so the row
// that opens it is named for the template alone.
const DAILY_SUMMARY = {
  id: 1,
  kind: "scratchpad" as const,
  name: "daily",
  description: null,
  placeholders: [],
  scope: "global" as const,
  revision: 2,
};

const DAILY = { ...DAILY_SUMMARY, description: "notes", body: "## Plan" };

// Serves appearance plus the scratchpad library, so the Templates tab can be browsed and drilled
// into through the real overlay rather than a stubbed panel.
function mockTemplatesBackend() {
  mockIPC((cmd, args) => {
    if (cmd === "appearance") return DEFAULT_APPEARANCE;
    if (cmd === "templates") {
      return (args as { kind: string }).kind === "scratchpad" ? [DAILY_SUMMARY] : [];
    }
    if (cmd === "template_defaults") return { scratchpad: null, todo: null };
    if (cmd === "template_read") return DAILY;
    return undefined;
  });
}

// The element the active panel renders into. Layout is only observable through what is actually on
// screen, so these tests read the rendered container rather than any signal a panel reports upward.
function panelContainer(): HTMLElement {
  return screen.getByRole("tabpanel").firstElementChild as HTMLElement;
}

describe("Settings — panel width", () => {
  it("keeps the standard centered column for a browse view", async () => {
    mockTemplatesBackend();
    renderSettings();

    fireEvent.click(screen.getByRole("tab", { name: "Templates" }));
    await screen.findByRole("button", { name: "Duplicate daily" });

    expect(panelContainer().className).toContain("max-w-2xl");
  });

  it("goes full width when a template is opened, and back on return to the list", async () => {
    mockTemplatesBackend();
    renderSettings();

    fireEvent.click(screen.getByRole("tab", { name: "Templates" }));
    await screen.findByRole("button", { name: "Duplicate daily" });
    fireEvent.click(screen.getByRole("button", { name: "daily" }));
    await screen.findByRole("button", { name: "Delete template" });

    expect(panelContainer().className).not.toContain("max-w-2xl");
    expect(panelContainer().className).toContain("h-full");

    fireEvent.click(screen.getByRole("button", { name: "Templates" }));
    await screen.findByRole("button", { name: "Duplicate daily" });
    expect(panelContainer().className).toContain("max-w-2xl");
  });

  // The width follows what the panel renders, so re-selecting the tab a builder is already open in
  // cannot desync the two — the editor stays on screen at the width it needs.
  it("keeps the builder width when its own tab is re-selected", async () => {
    mockTemplatesBackend();
    renderSettings();

    fireEvent.click(screen.getByRole("tab", { name: "Templates" }));
    await screen.findByRole("button", { name: "Duplicate daily" });
    fireEvent.click(screen.getByRole("button", { name: "daily" }));
    await screen.findByRole("button", { name: "Delete template" });

    fireEvent.click(screen.getByRole("tab", { name: "Templates" }));

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Delete template" })).toBeTruthy(),
    );
    expect(panelContainer().className).not.toContain("max-w-2xl");
    expect(panelContainer().className).toContain("h-full");
  });
});
