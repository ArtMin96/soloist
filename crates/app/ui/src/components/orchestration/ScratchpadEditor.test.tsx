// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { scratchpadRead, scratchpadWrite } from "@/api";
import { ScratchpadDetail } from "@/components/orchestration/ScratchpadDetail";
import { SCRATCHPAD_BODY_LABEL } from "@/components/orchestration/ScratchpadEditor";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useScratchpadEditor } from "@/store/useScratchpadEditor";
import type { ScratchpadSummary, ScratchpadView } from "@/domain";

// The real store and the real pane, wired together exactly as the board wires them — only the
// network boundary (`@/api`) is mocked. This is the seam a refused autosave actually crosses:
// `useAutosave` inside `ScratchpadEditor`, the real `useScratchpadEditor.save`, and the conflict
// banner the pane renders from the store's own state.
vi.mock("@/api", () => ({
  scratchpadRead: vi.fn(),
  scratchpadWrite: vi.fn(),
  scratchpadRename: vi.fn(),
  scratchpadLink: vi.fn(),
}));

// The rich editor needs real layout jsdom does not provide; a textarea standing in for it keeps this
// file on the autosave/conflict wiring rather than on TipTap. It seeds from `initialMarkdown` so the
// read view's rendered body is readable back as the field's value.
vi.mock("@/components/editor/LazyRichTextEditor", () => ({
  LazyRichTextEditor: (props: {
    ariaLabel?: string;
    initialMarkdown: string;
    onChange: (value: string) => void;
    onBlur?: () => void;
  }) => (
    <textarea
      aria-label={props.ariaLabel}
      defaultValue={props.initialMarkdown}
      onChange={(event) => props.onChange(event.target.value)}
      onBlur={() => props.onBlur?.()}
    />
  ),
}));

afterEach(() => {
  cleanup();
  // Reset rather than clear: a queued one-shot the paused autosave never consumed would otherwise
  // be the answer the next test's write receives.
  vi.resetAllMocks();
});

const PROJECT = 1;
const NOW = new Date(2026, 0, 20, 12).getTime();

const pad: ScratchpadSummary = {
  id: 4,
  name: "release-plan",
  tags: [],
  archived: false,
  revision: 3,
  gist: "the plan",
  updated_at: NOW,
};

const view = (revision: number, body = "the plan"): ScratchpadView => ({
  id: 4,
  name: "release-plan",
  tags: [],
  archived: false,
  revision,
  body,
  rendered: `# release-plan\n\n${body}`,
});

/** The real store bound to the real pane, open on `release-plan` and editing from the first read. */
function OpenScratchpad() {
  const editor = useScratchpadEditor(PROJECT);
  const [editing, setEditing] = useState(true);

  if (editor.name == null) {
    editor.open(pad.name);
    return null;
  }

  return (
    <TooltipProvider delayDuration={0}>
      <ScratchpadDetail
        pad={pad}
        document={editor.document}
        onRetry={editor.reload}
        now={NOW}
        onBack={() => {}}
        onStartEdit={() => setEditing(true)}
        onArchive={() => {}}
        onCopyLink={() => {}}
        onExport={() => {}}
        onCopyMarkdown={() => {}}
        error={null}
        edit={
          editing
            ? {
                mountKey: editor.mountKey,
                conflict: editor.conflict,
                error: editor.error,
                onSave: editor.save,
                onReload: editor.reload,
                onRename: editor.rename,
                onDone: () => setEditing(false),
                onBodyChange: () => {},
              }
            : null
        }
      />
    </TooltipProvider>
  );
}

const editorField = () => screen.findByLabelText(SCRATCHPAD_BODY_LABEL);

describe("ScratchpadEditor", () => {
  // The core refused the write because the document moved on elsewhere. Nothing was overwritten, so
  // the surface must say so and then stop trying: retrying behind the user's back is what would turn
  // a refusal into the lost edit the guard exists to prevent.
  it("names the conflicting revision and never reports a stale edit as saved", async () => {
    vi.mocked(scratchpadRead).mockResolvedValueOnce(view(3));
    render(<OpenScratchpad />);
    const body = await editorField();

    vi.mocked(scratchpadWrite).mockRejectedValueOnce("scratchpad revision conflict");
    vi.mocked(scratchpadRead).mockResolvedValueOnce(view(9));

    fireEvent.change(body, { target: { value: "edited here" } });
    fireEvent.blur(body);

    await waitFor(() => expect(screen.getByText(/changed elsewhere/)).toBeTruthy());
    expect(screen.getByText(/now at revision 9/)).toBeTruthy();
    expect(screen.getByText("Unsaved changes")).toBeTruthy();

    // Typing on past the refusal must not resume writing: the conflict is still unresolved, and the
    // write that lands would be the very one the core just turned down.
    vi.mocked(scratchpadWrite).mockResolvedValueOnce(view(10, "edited again"));
    fireEvent.change(body, { target: { value: "edited again" } });
    fireEvent.blur(body);

    await waitFor(() => expect(screen.getByText("Unsaved changes")).toBeTruthy());
    expect(screen.queryByText("Saved")).toBeNull();
    expect(scratchpadWrite).toHaveBeenCalledTimes(1);
  });

  // There is no Save control: leaving edit mode is what persists the last keystrokes, through the
  // autosave's unmount flush. If that flush stopped happening, the edit would be silently discarded.
  it("persists the last keystrokes when edit mode is left, and reads them back", async () => {
    vi.mocked(scratchpadRead).mockResolvedValueOnce(view(3));
    render(<OpenScratchpad />);
    const body = await editorField();

    vi.mocked(scratchpadWrite).mockResolvedValueOnce(view(4, "the revised plan"));
    fireEvent.change(body, { target: { value: "the revised plan" } });
    fireEvent.click(screen.getByRole("button", { name: "Done" }));

    await waitFor(() => expect(scratchpadWrite).toHaveBeenCalledTimes(1));
    // What crossed the network boundary is what was persisted; the guard it carried is the revision
    // the document was opened at.
    expect(vi.mocked(scratchpadWrite).mock.calls[0]).toEqual([
      PROJECT,
      pad.name,
      "the revised plan",
      3,
    ]);

    // The read view renders the body the write answered with, under its own name.
    const read = (await screen.findByLabelText("Release plan body")) as HTMLTextAreaElement;
    expect(read.value).toBe("the revised plan");
  });
});
