// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { scratchpadRead } from "@/api";
import { ScratchpadPanel } from "@/components/orchestration/ScratchpadPanel";
import type { ScratchpadSummary, ScratchpadView } from "@/domain";

// The panel reaches IPC only when a scratchpad is opened; the network boundary (`@/api`) is mocked to
// keep the Tauri bridge out of the headless test, exactly as `ScratchpadEditor.test.tsx` mocks it.
vi.mock("@/api", () => ({
  scratchpadRead: vi.fn(),
  scratchpadWrite: vi.fn(),
  scratchpadRename: vi.fn(),
  scratchpadLink: vi.fn(),
  scratchpadArchive: vi.fn(),
  exportMarkdown: vi.fn(),
}));

// The rich editor needs real layout jsdom does not provide; a plain textarea stands in, exactly as in
// `ScratchpadEditor.test.tsx`.
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

const pad = (id: number, name: string): ScratchpadSummary => ({
  id,
  name,
  tags: [],
  archived: false,
  revision: 1,
  gist: "",
  updated_at: 0,
});

describe("ScratchpadPanel", () => {
  it("shows the first-run guidance and the pick-one placeholder when there are no scratchpads", () => {
    render(<ScratchpadPanel project={1} scratchpads={[]} />);
    expect(screen.getByText(/No scratchpads yet/)).toBeTruthy();
    expect(screen.getByText(/Select a scratchpad to read or edit it/)).toBeTruthy();
  });

  it("walks the three-way branch: nothing open, then loading, then the open editor", async () => {
    let resolveRead: (view: ScratchpadView) => void = () => {};
    vi.mocked(scratchpadRead).mockReturnValueOnce(
      new Promise((resolve) => {
        resolveRead = resolve;
      }),
    );
    render(<ScratchpadPanel project={1} scratchpads={[pad(1, "release-plan")]} />);

    fireEvent.click(screen.getByRole("option", { name: /release plan/i }));
    expect(screen.getByText("Loading…")).toBeTruthy();

    resolveRead({
      id: 1,
      name: "release-plan",
      body: "the plan",
      rendered: "# release-plan\n\nthe plan",
      tags: [],
      archived: false,
      revision: 3,
    });

    await waitFor(() => expect(screen.getByLabelText("Scratchpad body")).toBeTruthy());
    expect(screen.queryByText("Loading…")).toBeNull();
  });

  it("focuses the target row once it arrives, even when the focus props land before the first snapshot", async () => {
    vi.mocked(scratchpadRead).mockResolvedValue({
      id: 1,
      name: "release-plan",
      body: "the plan",
      rendered: "# release-plan\n\nthe plan",
      tags: [],
      archived: false,
      revision: 1,
    });

    // Coming straight from a terminal, the pane mounts before its first snapshot arrives — the
    // focus target is not in `scratchpads` yet on this first render.
    const { rerender } = render(
      <ScratchpadPanel project={1} scratchpads={[]} focusName="release-plan" focusNonce={1} />,
    );
    expect(document.querySelector('[data-scratchpad-name="release-plan"]')).toBeNull();

    rerender(
      <ScratchpadPanel
        project={1}
        scratchpads={[pad(1, "release-plan")]}
        focusName="release-plan"
        focusNonce={1}
      />,
    );

    await waitFor(() => {
      const row = document.querySelector('[data-scratchpad-name="release-plan"]');
      expect(row).not.toBeNull();
      expect(document.activeElement).toBe(row);
    });
  });
});
