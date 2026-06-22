// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { AgentPicker } from "@/components/AgentPicker";
import type { DetectedTool, ProjectView } from "@/domain";

const TOOLS: DetectedTool[] = [
  {
    tool: {
      name: "Claude",
      command: "claude",
      default_args: [],
      kind: "Claude",
      prompt_mode: "AppendedArg",
    },
    installed: true,
  },
  {
    tool: {
      name: "Gemini",
      command: "gemini",
      default_args: [],
      kind: "Gemini",
      prompt_mode: "AppendedArg",
    },
    installed: false,
  },
];

const STOREFRONT: ProjectView = { id: 1, name: "Storefront", root: "/p/storefront", icon: null };
const API: ProjectView = { id: 2, name: "Api", root: "/p/api", icon: null };

afterEach(cleanup);

function renderPicker(props: Partial<Parameters<typeof AgentPicker>[0]> = {}) {
  const onLaunch = vi.fn();
  render(
    <AgentPicker
      open
      onOpenChange={() => {}}
      tools={TOOLS}
      projects={[STOREFRONT]}
      activeProjectId={1}
      onLaunch={onLaunch}
      {...props}
    />,
  );
  return { onLaunch };
}

describe("AgentPicker", () => {
  it("lists the tools with their command and detection status", () => {
    renderPicker();
    expect(screen.getByText("Claude")).toBeTruthy();
    expect(screen.getByText("claude")).toBeTruthy();
    expect(screen.getByText("installed")).toBeTruthy();
    expect(screen.getByText("not found")).toBeTruthy();
    // The footer names the launch target so it is never ambiguous.
    expect(screen.getByText("▸ Storefront")).toBeTruthy();
  });

  it("launches the chosen tool with no flags into the active project", () => {
    const { onLaunch } = renderPicker();
    fireEvent.click(screen.getByText("Claude"));
    expect(onLaunch).toHaveBeenCalledWith(1, "Claude", []);
  });

  it("opens a one-shot flags field on Alt+Enter and launches with tokenized flags", () => {
    const { onLaunch } = renderPicker();
    fireEvent.keyDown(screen.getByPlaceholderText("Launch agent…"), {
      key: "Enter",
      altKey: true,
    });

    const flags = screen.getByPlaceholderText("--model sonnet --permission-mode plan");
    fireEvent.change(flags, { target: { value: "--model sonnet" } });
    fireEvent.keyDown(flags, { key: "Enter" });
    expect(onLaunch).toHaveBeenCalledWith(1, "Claude", ["--model", "sonnet"]);
  });

  it("asks which project first when several are open and none is active", () => {
    const { onLaunch } = renderPicker({
      projects: [STOREFRONT, API],
      activeProjectId: null,
    });
    // The tool list is gated behind picking a project.
    expect(screen.queryByText("Claude")).toBeNull();
    fireEvent.click(screen.getByText("Api"));

    // Now the tools appear and launch into the chosen project.
    fireEvent.click(screen.getByText("Claude"));
    expect(onLaunch).toHaveBeenCalledWith(2, "Claude", []);
  });
});
