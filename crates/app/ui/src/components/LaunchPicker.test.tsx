// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { LaunchPicker } from "@/components/LaunchPicker";
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
    detection: "Installed",
  },
  {
    tool: {
      name: "Gemini",
      command: "gemini",
      default_args: [],
      kind: "Gemini",
      prompt_mode: "AppendedArg",
    },
    detection: "Missing",
  },
];

const STOREFRONT: ProjectView = { id: 1, name: "Storefront", root: "/p/storefront", icon: null };
const API: ProjectView = { id: 2, name: "Api", root: "/p/api", icon: null };

const FLAGS_PLACEHOLDER = "--model sonnet --permission-mode plan";

afterEach(cleanup);

function renderPicker(props: Partial<Parameters<typeof LaunchPicker>[0]> = {}) {
  const onLaunch = vi.fn();
  const onCreateTerminal = vi.fn();
  render(
    <LaunchPicker
      open
      onOpenChange={() => {}}
      tools={TOOLS}
      projects={[STOREFRONT]}
      onLaunch={onLaunch}
      onCreateTerminal={onCreateTerminal}
      {...props}
    />,
  );
  return { onLaunch, onCreateTerminal };
}

describe("LaunchPicker", () => {
  it("lists the tools with their command and detection status", () => {
    renderPicker();
    expect(screen.getByText("Claude")).toBeTruthy();
    expect(screen.getByText("claude")).toBeTruthy();
    expect(screen.getByText("installed")).toBeTruthy();
    expect(screen.getByText("not found")).toBeTruthy();
    // The footer names the launch target so it is never ambiguous.
    expect(screen.getByTestId("palette-target").textContent).toBe("Storefront");
  });

  it("marks a tool the probe could not check as unchecked, not as absent", () => {
    // "The probe reached no answer" and "this CLI is not on the machine" are different facts;
    // showing both as "not found" is what made a failing probe indistinguishable from an empty
    // toolchain.
    renderPicker({
      tools: [
        {
          tool: {
            name: "Claude",
            command: "claude",
            default_args: [],
            kind: "Claude",
            prompt_mode: "AppendedArg",
          },
          detection: "Unknown",
        },
      ],
    });

    expect(screen.getByText("not checked")).toBeTruthy();
    expect(screen.queryByText("not found")).toBeNull();
  });

  it("launches the chosen tool with no flags into the active project", () => {
    const { onLaunch } = renderPicker();
    fireEvent.click(screen.getByText("Claude"));
    expect(onLaunch).toHaveBeenCalledWith(1, "Claude", []);
  });

  it("opens a one-shot flags field on Alt+Enter and launches with tokenized flags", () => {
    const { onLaunch } = renderPicker();
    fireEvent.keyDown(screen.getByPlaceholderText("Launch an agent or open a terminal…"), {
      key: "Enter",
      altKey: true,
    });

    const flags = screen.getByPlaceholderText(FLAGS_PLACEHOLDER);
    fireEvent.change(flags, { target: { value: "--model sonnet" } });
    fireEvent.keyDown(flags, { key: "Enter" });
    expect(onLaunch).toHaveBeenCalledWith(1, "Claude", ["--model", "sonnet"]);
  });

  it("asks which project first when several are open and none is active", () => {
    const { onLaunch } = renderPicker({
      projects: [STOREFRONT, API],
    });
    // The tool list is gated behind picking a project.
    expect(screen.queryByText("Claude")).toBeNull();
    fireEvent.click(screen.getByText("Api"));

    // Now the tools appear and launch into the chosen project.
    fireEvent.click(screen.getByText("Claude"));
    expect(onLaunch).toHaveBeenCalledWith(2, "Claude", []);
  });

  it("offers a terminal alongside the agent tools", () => {
    // Ctrl+T has always been labelled "New agent or terminal"; the terminal is the half that
    // used to be missing, so it must be reachable from the same list without configuring one.
    renderPicker();
    expect(screen.getByText("Terminal")).toBeTruthy();
    expect(screen.getByText("your default shell")).toBeTruthy();
  });

  it("opens a terminal in the active project when the terminal entry is chosen", () => {
    const { onCreateTerminal, onLaunch } = renderPicker();
    fireEvent.click(screen.getByText("Terminal"));

    expect(onCreateTerminal).toHaveBeenCalledWith(1);
    expect(onLaunch).not.toHaveBeenCalled();
  });

  it("opens a terminal in the project the user chose when several are open", () => {
    const { onCreateTerminal } = renderPicker({ projects: [STOREFRONT, API] });
    fireEvent.click(screen.getByText("Api"));
    fireEvent.click(screen.getByText("Terminal"));

    expect(onCreateTerminal).toHaveBeenCalledWith(2);
  });

  it("offers no flags field for a terminal, which takes none", () => {
    // The flags step edits an agent's command line. A terminal has no command to append to, so
    // Alt+Enter on it must do nothing rather than open a field whose value is silently dropped.
    const { onCreateTerminal } = renderPicker({ tools: [] });
    fireEvent.keyDown(screen.getByPlaceholderText("Launch an agent or open a terminal…"), {
      key: "Enter",
      altKey: true,
    });

    expect(screen.queryByPlaceholderText(FLAGS_PLACEHOLDER)).toBeNull();
    // The terminal is still the one thing on offer, and still opens.
    fireEvent.click(screen.getByText("Terminal"));
    expect(onCreateTerminal).toHaveBeenCalledWith(1);
  });
});
