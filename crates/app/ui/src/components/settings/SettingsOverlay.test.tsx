// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { DeferredOverlay } from "@/components/DeferredOverlay";
import { SettingsOverlay } from "@/components/settings/SettingsOverlay";
import { ASSIST_SETTINGS_TAB, type SettingsTabId } from "@/components/settings/tabs";
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

function renderSettings(onOpenChange: (open: boolean) => void = () => {}) {
  render(
    <AppearanceProvider>
      <SettingsOverlay open onOpenChange={onOpenChange} project={null} />
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

describe("Settings — dismissal", () => {
  // The resizable split's divider calls preventDefault on pointerdown from a document-level capture
  // listener, before React dispatches. Radix skips its own "the pointer went down inside me" capture
  // handler for an already-prevented event, so without a containment guard a press on the divider
  // read as an outside click and closed Settings the instant the user grabbed it.
  it("stays open when a press inside it arrives already default-prevented", async () => {
    const onOpenChange = vi.fn();
    mockSettings(DEFAULT_APPEARANCE);
    renderSettings(onOpenChange);
    const inside = await screen.findByRole("tab", { name: "Appearance" });

    const preventOnTheWayDown = (event: Event) => event.preventDefault();
    document.addEventListener("pointerdown", preventOnTheWayDown, true);
    try {
      fireEvent.pointerDown(inside);
    } finally {
      document.removeEventListener("pointerdown", preventOnTheWayDown, true);
    }

    expect(onOpenChange).not.toHaveBeenCalled();
    expect(screen.getByRole("tab", { name: "Appearance" })).toBeTruthy();
  });

  // The guard is a containment check, not a blanket refusal to dismiss — a genuine press outside
  // the overlay must still close it.
  it("still closes on a press that genuinely lands outside it", async () => {
    const onOpenChange = vi.fn();
    mockSettings(DEFAULT_APPEARANCE);
    renderSettings(onOpenChange);
    await screen.findByRole("tab", { name: "Appearance" });

    fireEvent.pointerDown(document.body);

    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
  });
});

/**
 * The shell's side of opening Settings, mounted the way the app mounts it: behind the deferral latch,
 * so the overlay does not exist until it is first opened and then stays mounted for the rest of the
 * session. Both halves matter to a deep link — the first opening arrives at a brand-new mount that
 * already carries the tab that was asked for, and every later one arrives at an overlay still showing
 * wherever it was last left. A harness that mounts the overlay unconditionally has neither, and can
 * only prove things about itself.
 */
function SettingsShell() {
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState<SettingsTabId | null>(null);
  const openOn = (wanted: SettingsTabId | null) => {
    setTab(wanted);
    setOpen(true);
  };
  return (
    <AppearanceProvider>
      <button onClick={() => openOn(null)}>open settings</button>
      <button onClick={() => openOn(ASSIST_SETTINGS_TAB)}>open the assist setting</button>
      <DeferredOverlay open={open}>
        <SettingsOverlay
          open={open}
          onOpenChange={(next) => {
            setOpen(next);
            if (!next) setTab(null);
          }}
          project={null}
          tab={tab}
        />
      </DeferredOverlay>
    </AppearanceProvider>
  );
}

function selectedTab(): string | null {
  return screen.getByRole("tab", { selected: true }).textContent;
}

/** Appearance plus what the Agents tab reads, so a deep link can land on a real panel. */
function mockSettingsAndAgents() {
  mockIPC((cmd) => {
    if (cmd === "appearance") return DEFAULT_APPEARANCE;
    if (cmd === "agent_list") return [];
    if (cmd === "agent_detect" || cmd === "agent_redetect") return [];
    if (cmd === "assist_settings") return { tool: null };
    if (cmd === "hotkeys") return {};
    return undefined;
  });
}

describe("Settings — opening on a named tab", () => {
  it("opens on the tab the caller asked for on the very first opening of the session", async () => {
    // The first opening is the one the deep link exists for, and the hardest case: the overlay is
    // mounted by that same opening, so its first render already sees the named tab. Treating "what
    // the overlay arrived holding" as a request already honoured leaves Settings on its default
    // panel — the reader is sent to the setting they could not find and shown Appearance.
    mockSettingsAndAgents();
    render(<SettingsShell />);

    fireEvent.click(screen.getByRole("button", { name: "open the assist setting" }));

    await waitFor(() => expect(selectedTab()).toBe("Agents"));
    expect(screen.getByText("Agent tools"), "the tab it named is the one on screen").toBeTruthy();
  });

  it("follows the same link again after the reader has moved off that tab", async () => {
    // The overlay is never torn down between openings, so a tab that is only seeded once is
    // whichever one was last looked at — the second visit would land on the wrong panel.
    mockSettingsAndAgents();
    render(<SettingsShell />);

    fireEvent.click(screen.getByRole("button", { name: "open the assist setting" }));
    await waitFor(() => expect(selectedTab()).toBe("Agents"));
    fireEvent.click(screen.getByRole("tab", { name: "Hotkeys" }));
    await waitFor(() => expect(selectedTab()).toBe("Hotkeys"));
    fireEvent.click(screen.getByRole("button", { name: "Close settings" }));

    fireEvent.click(screen.getByRole("button", { name: "open the assist setting" }));

    await waitFor(() => expect(selectedTab()).toBe("Agents"));
  });

  it("leaves an opening that named no tab wherever the reader last left it", async () => {
    mockSettingsAndAgents();
    render(<SettingsShell />);

    fireEvent.click(screen.getByRole("button", { name: "open settings" }));
    await waitFor(() => expect(selectedTab()).toBe("Appearance"));
    fireEvent.click(screen.getByRole("tab", { name: "Hotkeys" }));
    await waitFor(() => expect(selectedTab()).toBe("Hotkeys"));
    fireEvent.click(screen.getByRole("button", { name: "Close settings" }));

    fireEvent.click(screen.getByRole("button", { name: "open settings" }));

    await waitFor(() => expect(selectedTab()).toBe("Hotkeys"));
  });

  it("leaves the tab alone while it is open, so a named one is not re-imposed on the reader", async () => {
    mockSettingsAndAgents();
    render(<SettingsShell />);

    fireEvent.click(screen.getByRole("button", { name: "open the assist setting" }));
    await waitFor(() => expect(selectedTab()).toBe("Agents"));

    fireEvent.click(screen.getByRole("tab", { name: "Sidebar" }));

    await waitFor(() => expect(selectedTab()).toBe("Sidebar"));
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
