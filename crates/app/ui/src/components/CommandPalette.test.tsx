// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CommandPalette } from "@/components/CommandPalette";
import type { ProcessView, ProjectView } from "@/domain";

const STOREFRONT: ProjectView = { id: 1, name: "Storefront", root: "/p/storefront", icon: null };
const WEB: ProcessView = {
  id: 10,
  project: 1,
  kind: "Command",
  label: "Web",
  status: "Running",
  exit_code: null,
  requires_trust: false,
  resumable: false,
  ports: [],
  ready: "Ungated",
};

afterEach(cleanup);

function renderPalette(props: Partial<Parameters<typeof CommandPalette>[0]> = {}) {
  const wiring = {
    newAgentOrTerminal: vi.fn(),
    openProject: vi.fn(),
    openSettings: vi.fn(),
    selectProcess: vi.fn(),
    openProjectSettings: vi.fn(),
    openOrchestration: vi.fn(),
    startAll: vi.fn(),
    stopAll: vi.fn(),
    restartRunning: vi.fn(),
  };
  const onOpenChange = vi.fn();
  render(
    <CommandPalette
      open
      onOpenChange={onOpenChange}
      processes={[WEB]}
      projects={[STOREFRONT]}
      process={{
        onTrust: vi.fn(),
        onResume: vi.fn(),
        onStart: vi.fn(),
        onStop: vi.fn(),
        onRestart: vi.fn(),
      }}
      {...wiring}
      {...props}
    />,
  );
  return { onOpenChange, ...wiring };
}

describe("CommandPalette", () => {
  it("lists app-wide, per-project, and per-process commands", () => {
    renderPalette();
    expect(screen.getByText("New agent or terminal")).toBeTruthy();
    expect(screen.getByText("Start all — Storefront")).toBeTruthy();
    expect(screen.getByText("Focus Web")).toBeTruthy();
  });

  it("runs the selected command and closes the palette", () => {
    const { newAgentOrTerminal, onOpenChange } = renderPalette();
    fireEvent.click(screen.getByText("New agent or terminal"));
    expect(newAgentOrTerminal).toHaveBeenCalledOnce();
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("filters the registry as the user types", () => {
    renderPalette();
    fireEvent.change(screen.getByPlaceholderText("Type a command…"), {
      target: { value: "orchestration" },
    });
    expect(screen.getByText("Open orchestration — Storefront")).toBeTruthy();
    expect(screen.queryByText("New agent or terminal")).toBeNull();
  });
});
