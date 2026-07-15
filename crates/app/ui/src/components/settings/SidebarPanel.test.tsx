// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SidebarPanel } from "@/components/settings/SidebarPanel";
import { DEFAULT_SIDEBAR } from "@/lib/sidebar";
import { SidebarSettingsProvider } from "@/store/SidebarSettingsProvider";
import type { Sidebar } from "@/domain";

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("Settings — Sidebar", () => {
  it("toggling 'Hide empty sections' persists through set_sidebar_settings", async () => {
    let saved: Sidebar | null = null;
    mockIPC((cmd, args) => {
      if (cmd === "sidebar_settings") return DEFAULT_SIDEBAR;
      if (cmd === "set_sidebar_settings") {
        saved = (args as { sidebar: Sidebar }).sidebar;
        return saved;
      }
      return undefined;
    });

    render(
      <SidebarSettingsProvider>
        <SidebarPanel />
      </SidebarSettingsProvider>,
    );

    const toggle = await screen.findByRole("switch", { name: "Hide empty sections" });
    await waitFor(() => expect(toggle.getAttribute("aria-checked")).toBe("false"));
    fireEvent.click(toggle);
    await waitFor(() => expect(saved?.hide_empty_sections).toBe(true));
  });

  it("shows the process usage thresholds and the filter toggle, and no project-header controls", async () => {
    mockIPC((cmd) => (cmd === "sidebar_settings" ? DEFAULT_SIDEBAR : undefined));

    render(
      <SidebarSettingsProvider>
        <SidebarPanel />
      </SidebarSettingsProvider>,
    );

    // The controls that now drive the live sidebar are present.
    expect(await screen.findByRole("switch", { name: "Show filter input" })).toBeTruthy();
    expect(screen.getByLabelText("Process CPU usage threshold")).toBeTruthy();
    expect(screen.getByLabelText("Process memory usage threshold")).toBeTruthy();

    // The removed decorative project-header controls are gone.
    expect(screen.queryByLabelText("Project CPU usage threshold")).toBeNull();
    expect(screen.queryByLabelText("Open in editor")).toBeNull();
    expect(screen.queryByLabelText("Open in terminal")).toBeNull();
    expect(screen.queryByLabelText("Show in file manager")).toBeNull();
  });
});
