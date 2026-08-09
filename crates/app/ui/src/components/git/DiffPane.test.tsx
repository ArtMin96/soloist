// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

// The two reads and the event subscription are the IPC boundary. The viewer and the preview are
// stubbed to a marker each: what this exercises is which of them the split shows, and what it
// says when it shows neither.
vi.mock("@/api", () => ({
  gitDiff: vi.fn(),
  gitFile: vi.fn(),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
  onResync: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@/components/git/DiffViewer", () => ({
  SIDE_BY_SIDE: "side-by-side",
  UNIFIED: "unified",
  DiffViewer: ({ layout }: { layout: string }) => <div data-testid="diff">{layout}</div>,
}));
vi.mock("@/components/git/FilePreview", () => ({
  FilePreview: ({ path }: { path: string }) => <div data-testid="preview">{path}</div>,
}));

import { gitDiff, gitFile } from "@/api";
import { DiffPane } from "@/components/git/DiffPane";
import { TooltipProvider } from "@/components/ui/tooltip";
import { CHANGE, FILE, type DiffSelection } from "@/store/git/useDiffSelection";
import type { DiffTarget, FileDiff } from "@/domain";

const readDiff = vi.mocked(gitDiff);
const readFile = vi.mocked(gitFile);

const PROJECT = 7;
const PATH = "src/main.rs";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  localStorage.clear();
});

function diffOf(overrides: Partial<FileDiff> = {}): FileDiff {
  return {
    path: PATH,
    original_path: null,
    target: "unstaged",
    binary: false,
    patch: `diff --git a/${PATH} b/${PATH}\n@@ -1 +1 @@\n-a\n+b\n`,
    hunks: [{ old_start: 1, old_lines: 1, new_start: 1, new_lines: 1 }],
    truncated: false,
    ...overrides,
  };
}

function renderPane(selection: DiffSelection = { kind: CHANGE, path: PATH }, onClose = vi.fn()) {
  render(
    <TooltipProvider>
      <DiffPane project={PROJECT} selection={selection} onClose={onClose} />
    </TooltipProvider>,
  );
  return onClose;
}

/** The comparison the most recent read asked for. */
function targetAsked(): DiffTarget | undefined {
  const calls = readDiff.mock.calls;
  return calls[calls.length - 1]?.[2];
}

describe("DiffPane", () => {
  it("shows a path's diff", async () => {
    readDiff.mockResolvedValue(diffOf());

    renderPane();

    expect(await screen.findByTestId("diff")).toBeTruthy();
    expect(screen.getByTitle(PATH)).toBeTruthy();
  });

  it("offers to load the whole of a diff it was only given the start of", async () => {
    readDiff.mockResolvedValue(diffOf({ truncated: true }));
    renderPane();
    await screen.findByTestId("diff");

    expect(screen.getByText(/showing the first part/i)).toBeTruthy();
    readDiff.mockResolvedValue(diffOf({ truncated: false }));
    fireEvent.click(screen.getByRole("button", { name: /load the whole diff/i }));

    await waitFor(() => expect(screen.queryByText(/showing the first part/i)).toBeNull());
  });

  it("says a binary file holds nothing to show rather than rendering its bytes", async () => {
    readDiff.mockResolvedValue(diffOf({ binary: true, patch: "" }));

    renderPane();

    expect(await screen.findByText(/holds bytes rather than text/i)).toBeTruthy();
    expect(screen.queryByTestId("diff")).toBeNull();
  });

  it("says so when the chosen comparison holds no change", async () => {
    readDiff.mockResolvedValue(diffOf({ patch: "" }));

    renderPane();

    expect(await screen.findByText(/no changes at this comparison/i)).toBeTruthy();
  });

  it("reads the comparison the reader chose", async () => {
    readDiff.mockResolvedValue(diffOf());
    renderPane();
    await screen.findByTestId("diff");

    fireEvent.click(screen.getByRole("radio", { name: "Staged" }));

    await waitFor(() => expect(targetAsked()).toBe("staged"));
  });

  it("offers no comparison for an untracked path, which has only one", async () => {
    readDiff.mockResolvedValue(diffOf({ target: "untracked" }));

    renderPane();
    await screen.findByTestId("diff");

    expect(
      screen.queryByRole("radio", { name: "Staged" }),
      "there is nothing earlier to compare an untracked path against",
    ).toBeNull();
  });

  it("lays the two sides out side by side or in one column", async () => {
    readDiff.mockResolvedValue(diffOf());
    renderPane();

    expect((await screen.findByTestId("diff")).textContent).toBe("side-by-side");
    fireEvent.click(screen.getByRole("button", { name: /in one column/i }));
    expect(screen.getByTestId("diff").textContent).toBe("unified");
  });

  it("closes on Escape", async () => {
    readDiff.mockResolvedValue(diffOf());
    const onClose = renderPane();
    await screen.findByTestId("diff");

    fireEvent.keyDown(window, { key: "Escape" });

    expect(onClose).toHaveBeenCalled();
  });

  it("leaves Escape alone while it is being typed into something", async () => {
    readDiff.mockResolvedValue(diffOf());
    const onClose = renderPane();
    await screen.findByTestId("diff");
    const field = document.createElement("textarea");
    document.body.append(field);

    fireEvent.keyDown(field, { key: "Escape" });

    expect(onClose, "a terminal owns its own Escape").not.toHaveBeenCalled();
    field.remove();
  });

  it("remembers how the reader last sized the split", async () => {
    readDiff.mockResolvedValue(diffOf());
    renderPane();
    await screen.findByTestId("diff");

    const divider = screen.getByRole("separator", { name: /resize the split/i });
    const before = Number(divider.getAttribute("aria-valuenow"));
    fireEvent.keyDown(divider, { key: "ArrowUp" });
    const after = Number(divider.getAttribute("aria-valuenow"));
    expect(after).toBeGreaterThan(before);

    cleanup();
    renderPane();
    await screen.findByTestId("diff");

    expect(
      Number(
        screen.getByRole("separator", { name: /resize the split/i }).getAttribute("aria-valuenow"),
      ),
    ).toBe(after);
  });

  it("fills the area and gives it back, remembering which it was", async () => {
    readDiff.mockResolvedValue(diffOf());
    renderPane();
    await screen.findByTestId("diff");

    fireEvent.click(screen.getByRole("button", { name: /fill the area/i }));
    expect(
      screen.queryByRole("separator", { name: /resize the split/i }),
      "there is nothing left to resize the split against",
    ).toBeNull();

    cleanup();
    renderPane();
    await screen.findByTestId("diff");
    expect(screen.getByRole("button", { name: /share the area/i })).toBeTruthy();
  });

  it("previews a file rather than diffing it when the file itself was opened", async () => {
    readFile.mockResolvedValue({ text: "fn main() {}\n", truncated: false });

    renderPane({ kind: FILE, path: PATH });

    expect((await screen.findByTestId("preview")).textContent).toBe(PATH);
    expect(readDiff, "a file is read, not compared").not.toHaveBeenCalled();
  });

  it("says a previewed file was only carried as far as its start", async () => {
    readFile.mockResolvedValue({ text: "a\n", truncated: true });

    renderPane({ kind: FILE, path: PATH });

    expect(await screen.findByText(/showing the beginning of this file/i)).toBeTruthy();
  });
});
