// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { StartSurface } from "@/components/StartSurface";

afterEach(cleanup);

describe("StartSurface", () => {
  it("offers the existing project action and clearly marks future paths unavailable", () => {
    const onOpenProject = vi.fn();
    render(
      <StartSurface hasProjects={false} onOpenProject={onOpenProject} onLaunchAgent={vi.fn()} />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Open project/ }));
    expect(onOpenProject).toHaveBeenCalledOnce();
    expect(screen.queryByRole("button", { name: /Clone from URL/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /Quick start/ })).toBeNull();
    expect(screen.getAllByText("Coming soon")).toHaveLength(2);
  });

  it("enables launching only after a project is open", () => {
    const onLaunchAgent = vi.fn();
    const { rerender } = render(
      <StartSurface hasProjects={false} onOpenProject={vi.fn()} onLaunchAgent={onLaunchAgent} />,
    );

    expect(
      (screen.getByRole("button", { name: "Launch agent" }) as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(screen.getByText("Open a project first.")).toBeTruthy();

    rerender(<StartSurface hasProjects onOpenProject={vi.fn()} onLaunchAgent={onLaunchAgent} />);
    fireEvent.click(screen.getByRole("button", { name: "Launch agent" }));
    expect(onLaunchAgent).toHaveBeenCalledOnce();
  });

  it("keeps project feedback inside the oriented start surface", () => {
    render(
      <StartSurface
        hasProjects
        notice="Created a starter solo.yml."
        onOpenProject={vi.fn()}
        onLaunchAgent={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "Start in Soloist" })).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("Created a starter solo.yml.");
  });
});
