// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { QuickActionsPalette } from "@/components/QuickActionsPalette";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { ProcessView, ProjectView } from "@/domain";

const STOREFRONT: ProjectView = { id: 1, name: "Storefront", root: "/p/storefront", icon: null };
const WEB_RUNNING: ProcessView = {
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

function renderActions(props: Partial<Parameters<typeof QuickActionsPalette>[0]> = {}) {
  const handlers = {
    onStart: vi.fn(),
    onStop: vi.fn(),
    onRestart: vi.fn(),
    onResume: vi.fn(),
    onTrust: vi.fn(),
  };
  const onOpenChange = vi.fn();
  render(
    <TooltipProvider>
      <QuickActionsPalette
        open
        onOpenChange={onOpenChange}
        processes={[WEB_RUNNING]}
        projects={[STOREFRONT]}
        activeProjectId={1}
        {...handlers}
        {...props}
      />
    </TooltipProvider>,
  );
  return { onOpenChange, ...handlers };
}

describe("QuickActionsPalette", () => {
  it("offers only the status-valid actions for the active project's processes", () => {
    renderActions();
    expect(screen.getByText("Stop")).toBeTruthy();
    expect(screen.getByText("Restart")).toBeTruthy();
    // Running, so Start is withheld (single-sourced from processActions).
    expect(screen.queryByText("Start")).toBeNull();
  });

  it("runs an action and closes", () => {
    const { onStop, onOpenChange } = renderActions();
    fireEvent.click(screen.getByText("Stop"));
    expect(onStop).toHaveBeenCalledWith(10);
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("prompts to open a project when none is active", () => {
    renderActions({ activeProjectId: null });
    expect(screen.getByText("Open a project to see actions.")).toBeTruthy();
  });
});
