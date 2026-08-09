// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { AgentsPanel } from "@/components/settings/AgentsPanel";
import type { AgentTool, Assist, DetectedTool } from "@/domain";

const tool = (name: string, command: string): AgentTool => ({
  name,
  command,
  default_args: [],
  kind: "Generic",
  prompt_mode: "Stdin",
});

// Records which detection command each read routed to: the cached sweep (`agent_detect`) or the
// re-probing one (`agent_redetect`). The distinction is the whole point of the Detect button.
// `assist_settings` is answered too, because the panel reads which tool may draft text; `saved`
// collects what a change to that selection persisted. `detectFails` makes the sweep refuse with
// that message, so the panel can be tested for what it says when nothing was ever checked.
function mockAgents(opts: { detected: DetectedTool[]; assist?: Assist; detectFails?: string }) {
  const calls: string[] = [];
  const saved: Assist[] = [];
  let assist: Assist = opts.assist ?? { tool: null };
  mockIPC((cmd, args) => {
    if (cmd === "agent_list") return opts.detected.map((d) => d.tool);
    if (cmd === "agent_detect" || cmd === "agent_redetect") {
      calls.push(cmd);
      if (opts.detectFails !== undefined) throw new Error(opts.detectFails);
      return opts.detected;
    }
    if (cmd === "assist_settings") return assist;
    if (cmd === "set_assist_settings") {
      assist = (args as { assist: Assist }).assist;
      saved.push(assist);
      return assist;
    }
    return undefined;
  });
  return { calls, saved };
}

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("Settings — Agents", () => {
  it("lists detected agent tools with their detection status", async () => {
    mockAgents({
      detected: [
        { tool: tool("Claude", "claude"), detection: "Installed", can_draft: true },
        { tool: tool("Codex", "codex"), detection: "Missing", can_draft: true },
      ],
    });

    render(<AgentsPanel />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeTruthy());
    expect(screen.getByText("installed")).toBeTruthy();
    expect(screen.getByText("not found")).toBeTruthy();
  });

  it("reports a tool the probe could not check as unchecked, not as absent", async () => {
    // A probe that reached no answer must not render as "not found" — that is exactly how a
    // failing probe disguised itself as a machine with no agent CLIs installed.
    mockAgents({
      detected: [{ tool: tool("Claude", "claude"), detection: "Unknown", can_draft: true }],
    });

    render(<AgentsPanel />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeTruthy());
    expect(screen.getByText("not checked")).toBeTruthy();
    expect(screen.queryByText("not found")).toBeNull();
  });

  it("re-probes rather than re-reading the cached sweep when Detect is clicked", async () => {
    // Reading the cached sweep again is what made this button appear dead: the core serves the
    // same answer for the whole TTL, so an explicit re-check has to bypass it to mean anything.
    const { calls } = mockAgents({
      detected: [{ tool: tool("Claude", "claude"), detection: "Installed", can_draft: true }],
    });

    render(<AgentsPanel />);

    await waitFor(() => expect(calls).toEqual(["agent_detect"]));
    fireEvent.click(screen.getByRole("button", { name: "Detect" }));
    await waitFor(() => expect(calls).toEqual(["agent_detect", "agent_redetect"]));
  });

  it("offers only installed tools Soloist can run headless as the drafting tool", async () => {
    // A provider Soloist cannot ask a single question, and one that is not on this machine, would
    // both only fail — so neither is offered rather than offered and then refused.
    mockAgents({
      detected: [
        { tool: tool("Claude", "claude"), detection: "Installed", can_draft: true },
        { tool: tool("Copilot", "copilot"), detection: "Installed", can_draft: false },
        { tool: tool("Codex", "codex"), detection: "Missing", can_draft: true },
      ],
    });

    render(<AgentsPanel />);

    fireEvent.click(await screen.findByRole("combobox", { name: "Draft text with" }));
    const offered = (await screen.findAllByRole("option")).map((option) => option.textContent);
    expect(offered).toEqual(["Off", "Claude"]);
  });

  it("persists the drafting tool that was picked", async () => {
    const { saved } = mockAgents({
      detected: [{ tool: tool("Claude", "claude"), detection: "Installed", can_draft: true }],
    });

    render(<AgentsPanel />);

    fireEvent.click(await screen.findByRole("combobox", { name: "Draft text with" }));
    fireEvent.click(await screen.findByRole("option", { name: "Claude" }));

    await waitFor(() => expect(saved).toEqual([{ tool: "Claude" }]));
  });

  it("starts with no drafting tool, so nothing is ever run unasked", async () => {
    mockAgents({
      detected: [{ tool: tool("Claude", "claude"), detection: "Installed", can_draft: true }],
    });

    render(<AgentsPanel />);

    expect(
      (await screen.findByRole("combobox", { name: "Draft text with" })).textContent,
    ).toContain("Off");
  });

  it("says which CLIs to install, beside the picker rather than instead of it", async () => {
    // Half the complaint this answers: with nothing installed the picker held one entry, "Off",
    // which reads as decoration rather than as the place drafting is configured — and never said
    // what would make it work. The named CLIs come from the registry's own drafting-capable rows,
    // so Copilot (no one-shot form) is absent from them for the same reason it is never offered.
    // The other half is that the guidance must not cost the reader the control: the setting is
    // where a selection is read and changed, and this is a state a selection can be stuck in.
    mockAgents({
      detected: [
        { tool: tool("Claude", "claude"), detection: "Missing", can_draft: true },
        { tool: tool("Kimi", "kimi"), detection: "Missing", can_draft: true },
        { tool: tool("Copilot", "copilot"), detection: "Missing", can_draft: false },
      ],
    });

    render(<AgentsPanel />);

    expect(
      await screen.findByText(
        "No tool is available to draft with. Install one of Claude, Kimi, then run Detect above.",
      ),
    ).toBeTruthy();
    expect(screen.getByRole("combobox", { name: "Draft text with" })).toBeTruthy();
  });

  it("keeps a stored tool visible and clearable when nothing installed can draft", async () => {
    // The state naming an unavailable value was written for, and the one it could never reach: a
    // tool was picked and then went away, so nothing is on offer and the guidance took the picker's
    // place. The stored name was then invisible — the trigger it would have named was not on screen
    // — and there was no control left to clear it with, which made the selection permanent.
    const { saved } = mockAgents({
      detected: [{ tool: tool("Claude", "claude"), detection: "Missing", can_draft: true }],
      assist: { tool: "Claude" },
    });

    render(<AgentsPanel />);

    const picker = await screen.findByRole("combobox", { name: "Draft text with" });
    await waitFor(() => expect(picker.textContent).toBe("Claude (unavailable)"));

    fireEvent.click(picker);
    fireEvent.click(await screen.findByRole("option", { name: "Off" }));

    await waitFor(() => expect(saved).toEqual([{ tool: null }]));
  });

  it("says the detection sweep failed rather than reading as a machine with no agent CLIs", async () => {
    // A refused sweep leaves every badge at "not checked" and the drafting picker with nothing to
    // offer — the same picture as a machine that simply has no agent CLIs on it. Only saying so
    // tells those apart.
    mockAgents({
      detected: [{ tool: tool("Claude", "claude"), detection: "Installed", can_draft: true }],
      detectFails: "the probe timed out",
    });

    render(<AgentsPanel />);

    const notice = await screen.findByRole("alert");
    expect(notice.textContent).toContain("Could not check which agent CLIs are installed");
    expect(notice.textContent).toContain("the probe timed out");
    expect(screen.getByText("not checked")).toBeTruthy();
  });

  it("names a stored drafting tool the picker can no longer offer, rather than showing nothing", async () => {
    // A stored name that is no longer offered (the tool was uninstalled, or renamed in the
    // registry) used to paint an empty trigger, which is exactly what "nothing is selected" looks
    // like — so the one state that needs fixing was the one that looked like the default.
    mockAgents({
      detected: [{ tool: tool("Claude", "claude"), detection: "Installed", can_draft: true }],
      assist: { tool: "Kimi" },
    });

    render(<AgentsPanel />);

    const picker = await screen.findByRole("combobox", { name: "Draft text with" });
    await waitFor(() => expect(picker.textContent).toBe("Kimi (unavailable)"));
  });

  it("offers no auto-summarization opt-in (the feature is not built)", async () => {
    mockAgents({
      detected: [{ tool: tool("Claude", "claude"), detection: "Installed", can_draft: true }],
    });

    render(<AgentsPanel />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeTruthy());
    expect(screen.queryByText("Auto-summarization")).toBeNull();
    expect(screen.queryByLabelText("Summarizer tool")).toBeNull();
    expect(screen.queryByLabelText("Summarizer model")).toBeNull();
  });
});
