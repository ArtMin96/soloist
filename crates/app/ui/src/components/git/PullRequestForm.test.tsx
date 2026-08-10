// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { PullRequestForm } from "@/components/git/PullRequestForm";
import { ASSIST_SETTINGS_TAB } from "@/components/settings/tabs";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ASSIST_SETUP_HINT } from "@/lib/agents";
import { OpenSettingsContext, type OpenSettings } from "@/store/settingsContext";
import type { PullRequestTemplate } from "@/domain";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const HEAD = "feature";
const BASE = "main";

function renderForm({
  title = "Live changes rail",
  base = BASE,
  body = "Because.",
  draft = false,
  templates = [],
  template = null,
  busy = false,
  assist = null,
  onSubmit = vi.fn(),
  openSettings = vi.fn(),
}: {
  title?: string;
  base?: string;
  body?: string;
  draft?: boolean;
  templates?: PullRequestTemplate[];
  template?: string | null;
  busy?: boolean;
  assist?: { drafting: boolean; request: () => void } | null;
  onSubmit?: () => void;
  openSettings?: OpenSettings;
} = {}) {
  render(
    <OpenSettingsContext value={openSettings}>
      <TooltipProvider>
        <PullRequestForm
          head={HEAD}
          title={title}
          base={base}
          body={body}
          draft={draft}
          templates={templates}
          template={template}
          busy={busy}
          assist={assist}
          onTitleChange={() => {}}
          onBaseChange={() => {}}
          onBodyChange={() => {}}
          onDraftChange={() => {}}
          onTemplateChange={() => {}}
          onSubmit={onSubmit}
        />
      </TooltipProvider>
    </OpenSettingsContext>,
  );
  return { onSubmit, openSettings };
}

function submitButton(): HTMLButtonElement {
  return screen.getByRole("button", { name: "Open pull request" }) as HTMLButtonElement;
}

describe("PullRequestForm", () => {
  it("names the branch the commits would come from, beside the one they would go into", () => {
    renderForm();

    expect((screen.getByLabelText("Merge into") as HTMLInputElement).value).toBe(BASE);
    expect(screen.getByText(`← ${HEAD}`)).toBeTruthy();
  });

  it("will not send a proposal with no title", () => {
    const { onSubmit } = renderForm({ title: "   " });

    expect(submitButton().disabled).toBe(true);
    fireEvent.click(submitButton());
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("will not send a proposal with nowhere to merge into", () => {
    const { onSubmit } = renderForm({ base: "" });

    expect(submitButton().disabled).toBe(true);
    fireEvent.click(submitButton());
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("says it is opening while it is, so the press is not repeated", () => {
    renderForm({ busy: true });

    expect(screen.getByRole("button", { name: "Opening…" })).toBeTruthy();
    expect((screen.getByRole("button", { name: "Opening…" }) as HTMLButtonElement).disabled).toBe(
      true,
    );
  });

  it("explains opening as a draft through the tooltip the rest of the feature uses", async () => {
    renderForm();

    fireEvent.focus(screen.getByRole("checkbox", { name: /draft/i }));

    const tip = await screen.findByRole("tooltip");
    expect(tip.textContent).toContain("Propose it without asking for review yet");
    expect(
      screen.getByText("Open as a draft").getAttribute("title"),
      "a native title is the one hint the rest of the feature does not use",
    ).toBeNull();
  });

  it("will not ask for a draft of a description with nowhere to compare against", () => {
    const request = vi.fn();
    renderForm({ base: "", assist: { drafting: false, request } });

    fireEvent.click(screen.getByRole("button", { name: "Draft a description" }));

    expect(request).not.toHaveBeenCalled();
  });

  it("says a description is being drafted while one is, and asks for no second", () => {
    const request = vi.fn();
    renderForm({ assist: { drafting: true, request } });

    expect(screen.getByText("Drafting a description…")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Draft a description" }));
    expect(request).not.toHaveBeenCalled();
  });
});

describe("PullRequestForm — before a tool is picked to draft with", () => {
  // The commit box and this form ask the same question of the same setting, so they answer it the
  // same way: the control is there, it is pressable, and pressing it goes to where a tool is picked.
  // This form used to leave it out instead — the same feature, offered on one surface and hidden on
  // the other, with the reader left to discover the setting some other way.
  it("offers the control anyway and takes the reader to the setting that picks one", async () => {
    const { openSettings, onSubmit } = renderForm({ assist: null });

    const control = screen.getByRole("button", { name: "Draft a description…" });
    expect(control.hasAttribute("disabled"), "a door that leads somewhere is not disabled").toBe(
      false,
    );
    fireEvent.click(control);

    await waitFor(() => expect(openSettings).toHaveBeenCalledWith(ASSIST_SETTINGS_TAB));
    expect(onSubmit, "reaching a setting proposes nothing").not.toHaveBeenCalled();
  });

  it("explains the door in the words the commit box uses for it", async () => {
    renderForm({ assist: null });

    fireEvent.focus(screen.getByRole("button", { name: "Draft a description…" }));

    const tip = await screen.findByRole("tooltip");
    expect(tip.textContent).toBe(ASSIST_SETUP_HINT);
  });
});
