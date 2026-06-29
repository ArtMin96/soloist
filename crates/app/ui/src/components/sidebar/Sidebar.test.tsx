// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { Sidebar } from "@/components/sidebar/Sidebar";
import { DEFAULT_SIDEBAR } from "@/lib/sidebar";
import { SidebarSettingsContext } from "@/store/sidebarSettingsContext";
import type { Sidebar as SidebarSettings } from "@/domain";

const noop = () => {};

function renderSidebar(settings: SidebarSettings) {
  render(
    <SidebarSettingsContext value={{ sidebar: settings, setSidebar: noop }}>
      <Sidebar
        projects={[]}
        processes={[]}
        selectedId={null}
        onSelect={noop}
        onStart={noop}
        onStop={noop}
        onRestart={noop}
        onTrust={noop}
        onStartAll={noop}
        onRestartRunning={noop}
        onStopAll={noop}
        onOpenSettings={noop}
        onOpenProjectSettings={noop}
        onOpenOrchestration={noop}
      />
    </SidebarSettingsContext>,
  );
}

afterEach(cleanup);

describe("Sidebar footer", () => {
  it("shows the Settings footer button when the setting is on", () => {
    renderSidebar({ ...DEFAULT_SIDEBAR, show_settings_footer: true });
    expect(screen.getByRole("button", { name: "Settings" })).toBeTruthy();
  });

  it("hides the Settings footer button when the setting is off", () => {
    renderSidebar({ ...DEFAULT_SIDEBAR, show_settings_footer: false });
    expect(screen.queryByRole("button", { name: "Settings" })).toBeNull();
  });
});
