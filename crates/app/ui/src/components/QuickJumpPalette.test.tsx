// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { QuickJumpPalette } from "@/components/QuickJumpPalette";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { ProcessView, ProjectView } from "@/domain";

const STOREFRONT: ProjectView = { id: 1, name: "Storefront", root: "/p/storefront", icon: null };
const WEB: ProcessView = {
  id: 10,
  project: 1,
  kind: "Agent",
  label: "Web",
  status: "Running",
  exit_code: null,
  requires_trust: false,
  resumable: false,
  ports: [],
  ready: "Ungated",
};

afterEach(cleanup);

function renderJump(props: Partial<Parameters<typeof QuickJumpPalette>[0]> = {}) {
  const onSelectProcess = vi.fn();
  const onSelectProject = vi.fn();
  const onOpenChange = vi.fn();
  render(
    <TooltipProvider>
      <QuickJumpPalette
        open
        onOpenChange={onOpenChange}
        processes={[WEB]}
        projects={[STOREFRONT]}
        onSelectProcess={onSelectProcess}
        onSelectProject={onSelectProject}
        {...props}
      />
    </TooltipProvider>,
  );
  return { onSelectProcess, onSelectProject, onOpenChange };
}

describe("QuickJumpPalette", () => {
  it("lists open projects and their processes", () => {
    renderJump();
    // The project is both a group heading and a selectable row, so it appears more than once.
    expect(screen.getAllByText("Storefront").length).toBeGreaterThan(0);
    expect(screen.getByText("Web")).toBeTruthy();
  });

  it("jumps to a process and closes", () => {
    const { onSelectProcess, onOpenChange } = renderJump();
    fireEvent.click(screen.getByText("Web"));
    expect(onSelectProcess).toHaveBeenCalledWith(10);
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("opens project settings from the project row and closes", () => {
    const { onSelectProject, onOpenChange } = renderJump();
    fireEvent.click(screen.getByText("Project settings"));
    expect(onSelectProject).toHaveBeenCalledWith(1);
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("shows an empty state when nothing is open", () => {
    renderJump({ processes: [], projects: [] });
    expect(screen.getByText("No destinations found.")).toBeTruthy();
  });
});
