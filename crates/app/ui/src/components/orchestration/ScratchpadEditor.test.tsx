// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { scratchpadRead, scratchpadWrite } from "@/api";
import { ScratchpadEditor } from "@/components/orchestration/ScratchpadEditor";
import { useScratchpadEditor } from "@/store/useScratchpadEditor";
import type { ScratchpadView } from "@/domain";

// The real store and the real editor shell, wired together exactly as `ScratchpadPanel` wires them —
// only the network boundary (`@/api`) is mocked. This is the seam a refused autosave actually crosses:
// `useAutosave` inside `ScratchpadBody`, the real `useScratchpadEditor.save`, and the conflict banner
// `ScratchpadEditor` renders from the store's own `conflict` state.
vi.mock("@/api", () => ({
  scratchpadRead: vi.fn(),
  scratchpadWrite: vi.fn(),
  scratchpadRename: vi.fn(),
  scratchpadLink: vi.fn(),
}));

// The rich editor needs real layout jsdom does not provide; a plain textarea standing in for it keeps
// this file on the autosave/conflict wiring rather than on TipTap.
vi.mock("@/components/editor/LazyRichTextEditor", () => ({
  LazyRichTextEditor: (props: { ariaLabel?: string; onChange: (value: string) => void }) => (
    <textarea
      aria-label={props.ariaLabel}
      onChange={(event) => props.onChange(event.target.value)}
    />
  ),
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const view = (revision: number): ScratchpadView => ({
  id: 4,
  name: "release-plan",
  body: "the plan",
  rendered: "# release-plan\n\nthe plan",
  tags: [],
  archived: false,
  revision,
});

/** Mounts the real store bound to the real editor shell, open on `release-plan` from the first read. */
function OpenScratchpad() {
  const editor = useScratchpadEditor(1);
  if (editor.name == null) {
    editor.open("release-plan");
    return null;
  }
  if (editor.initialBody == null) return null;
  return (
    <ScratchpadEditor
      name={editor.name}
      initialBody={editor.initialBody}
      revision={editor.baseRevision}
      mountKey={editor.mountKey}
      conflict={editor.conflict}
      error={editor.error}
      archived={false}
      onSave={editor.save}
      onReload={editor.reload}
      onCopyLink={() => {}}
      onArchive={() => {}}
      onRename={editor.rename}
    />
  );
}

describe("ScratchpadEditor refused save", () => {
  it("renders the conflict banner and stays honestly unsaved — never 'Saved'", async () => {
    vi.mocked(scratchpadRead).mockResolvedValueOnce(view(3));
    render(<OpenScratchpad />);
    const body = await screen.findByLabelText("Scratchpad body");

    // The core refuses the write; the re-read it triggers reveals a revision moved on elsewhere.
    vi.mocked(scratchpadWrite).mockRejectedValueOnce("scratchpad revision conflict");
    vi.mocked(scratchpadRead).mockResolvedValueOnce(view(9));

    fireEvent.change(body, { target: { value: "edited elsewhere too" } });
    fireEvent.click(screen.getByRole("button", { name: /Save/ }));

    await waitFor(() => expect(screen.getByText(/changed elsewhere/)).toBeTruthy());
    expect(screen.getByText(/now at revision 9/)).toBeTruthy();
    expect(screen.getByText("Unsaved changes")).toBeTruthy();
    expect(screen.queryByText("Saved")).toBeNull();
  });
});
