// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CommitBox } from "@/components/git/CommitBox";
import { ASSIST_SETTINGS_TAB } from "@/components/settings/tabs";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ASSIST_SETUP_HINT } from "@/lib/agents";
import { OpenSettingsContext, type OpenSettings } from "@/store/settingsContext";
import type { FileChange } from "@/domain";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

function change(path: string, staged: boolean): FileChange {
  return {
    path,
    status: staged
      ? { staged: "modified", unstaged: null }
      : { staged: null, unstaged: "modified" },
    original_path: null,
  };
}

/** Renders the box with the shell's open-Settings action in place, since one of the controls' whole
 *  job is to reach it. */
function renderBox({
  changes = [change("src/a.rs", true)],
  draft = null,
  busy = false,
  template = null,
  onCommit = vi.fn(() => Promise.resolve(true)),
  openSettings = vi.fn(),
  reveal = true,
}: {
  changes?: FileChange[];
  draft?: { drafting: boolean; request: () => Promise<string | null> } | null;
  busy?: boolean;
  template?: string | null;
  onCommit?: (message: string, amend: boolean) => Promise<boolean>;
  openSettings?: OpenSettings;
  reveal?: boolean;
} = {}) {
  render(
    <OpenSettingsContext value={openSettings}>
      <TooltipProvider>
        <CommitBox
          changes={changes}
          busy={busy}
          template={template}
          draft={draft}
          onCommit={onCommit}
        />
      </TooltipProvider>
    </OpenSettingsContext>,
  );
  if (reveal) fireEvent.click(screen.getByRole("button", { name: /commit changes/i }));
  return { onCommit, openSettings };
}

function message(): HTMLTextAreaElement {
  return screen.getByLabelText("Commit message") as HTMLTextAreaElement;
}

describe("CommitBox — disclosure", () => {
  it("starts as one compact action and reveals the composer only when asked", () => {
    renderBox({ changes: [change("src/a.rs", true), change("src/b.rs", true)], reveal: false });

    const trigger = screen.getByRole("button", { name: /commit changes/i });
    expect(trigger.textContent).toContain("2 staged");
    expect(screen.queryByLabelText("Commit message")).toBeNull();

    fireEvent.click(trigger);

    expect(screen.getByLabelText("Commit message")).toBeTruthy();
  });

  it("keeps an unfinished message and amend choice while the composer is hidden", () => {
    renderBox();
    fireEvent.change(message(), { target: { value: "A message that is not finished yet" } });
    fireEvent.click(screen.getByRole("checkbox"));

    fireEvent.click(screen.getByRole("button", { name: /hide commit composer/i }));
    expect(screen.queryByLabelText("Commit message")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /show commit composer/i }));

    expect(message().value).toBe("A message that is not finished yet");
    expect((screen.getByRole("checkbox") as HTMLButtonElement).dataset.state).toBe("checked");
  });
});

describe("CommitBox — drafting a message", () => {
  it("offers drafting before a tool is picked, and takes the reader to the setting that picks one", async () => {
    // The opt-in defaults to off, so hiding the control until it is on is how the feature stays
    // undiscovered: nobody switches on something they never saw.
    const { openSettings, onCommit } = renderBox({ draft: null });

    fireEvent.click(screen.getByRole("button", { name: "Draft…" }));

    await waitFor(() => expect(openSettings).toHaveBeenCalledWith(ASSIST_SETTINGS_TAB));
    expect(message().value, "reaching a setting drafts nothing").toBe("");
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("explains the door in the words the pull request's form uses for it", async () => {
    // Both surfaces send the reader to the same setting, so both say the same thing about it — and
    // neither promises a tool to pick, because whether there is one to pick is what that setting
    // answers.
    renderBox({ draft: null });

    fireEvent.focus(screen.getByRole("button", { name: "Draft…" }));

    const tip = await screen.findByRole("tooltip");
    expect(tip.textContent).toBe(ASSIST_SETUP_HINT);
  });

  it("puts what a picked tool drafted in the box, editable, and commits nothing by itself", async () => {
    const request = vi.fn(() => Promise.resolve("Record the staged index"));
    const { onCommit } = renderBox({ draft: { drafting: false, request } });

    fireEvent.click(screen.getByRole("button", { name: "Draft" }));

    await waitFor(() => expect(message().value).toBe("Record the staged index"));
    fireEvent.change(message(), { target: { value: "Record the staged index, corrected" } });
    expect(message().value, "what came back is editable like anything typed").toBe(
      "Record the staged index, corrected",
    );
    expect(
      onCommit,
      "a draft is a draft; committing it stays the user's press",
    ).not.toHaveBeenCalled();
  });

  it("will not ask a picked tool about a change nobody staged", () => {
    const request = vi.fn(() => Promise.resolve("Anything"));
    renderBox({ changes: [change("src/a.rs", false)], draft: { drafting: false, request } });

    fireEvent.click(screen.getByRole("button", { name: "Draft" }));

    expect(request).not.toHaveBeenCalled();
  });
});

describe("CommitBox — what a press would record", () => {
  it("names what is staged, so the resting state says something rather than nothing", () => {
    renderBox({ changes: [change("src/a.rs", true), change("src/b.rs", true)] });

    expect(screen.getByText("2 files staged")).toBeTruthy();
  });

  it("counts one staged file as one", () => {
    renderBox({ changes: [change("src/a.rs", true)] });

    expect(screen.getByText("1 file staged")).toBeTruthy();
  });

  it("says nothing is staged instead of leaving the line out", () => {
    renderBox({ changes: [change("src/a.rs", false)] });

    expect(screen.getByText("Nothing is staged to commit")).toBeTruthy();
  });

  it("says the last commit is being amended, which is a different thing from adding one", () => {
    renderBox({ changes: [change("src/a.rs", false)] });

    fireEvent.click(screen.getByRole("checkbox"));

    expect(screen.getByText("Amending the last commit")).toBeTruthy();
  });

  it("says a draft is being written while one is", () => {
    renderBox({ draft: { drafting: true, request: () => Promise.resolve(null) } });

    expect(screen.getByText("Drafting a message…")).toBeTruthy();
  });
});

describe("CommitBox — amending", () => {
  it("explains amending through the tooltip every other control here uses", async () => {
    renderBox();

    fireEvent.focus(screen.getByRole("checkbox"));

    const tip = await screen.findByRole("tooltip");
    expect(tip.textContent).toContain("Replace the last commit instead of adding one");
    expect(
      screen.getByText("Amend").getAttribute("title"),
      "a native title is the one hint the rest of the feature does not use",
    ).toBeNull();
  });

  it("commits an amendment with no staged change, because that is how a message is corrected", async () => {
    const { onCommit } = renderBox({ changes: [] });

    fireEvent.change(message(), { target: { value: "Say it properly" } });
    fireEvent.click(screen.getByRole("checkbox"));
    fireEvent.click(screen.getByRole("button", { name: "Commit" }));

    await waitFor(() => expect(onCommit).toHaveBeenCalledWith("Say it properly", true));
  });
});
