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
});
