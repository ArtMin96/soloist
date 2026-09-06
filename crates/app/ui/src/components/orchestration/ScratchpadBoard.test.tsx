// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { CARD_TRIGGER_ATTRIBUTE } from "@/components/common/CardRow";
import { DETAIL_BACK_ATTRIBUTE } from "@/components/common/DetailPane";
import { GROUP_ATTRIBUTE } from "@/components/common/CollapsibleGroup";
import { BOARD_COUNT_ATTRIBUTE } from "@/components/common/BoardToolbar";
import { PANEL_ATTRIBUTE, PANEL_ROUTE_ATTRIBUTE } from "@/components/common/SlidingPanels";
import {
  SCRATCHPAD_ROW_ID_ATTRIBUTE,
  SCRATCHPAD_ROW_NAME_ATTRIBUTE,
  ScratchpadBoard,
} from "@/components/orchestration/ScratchpadBoard";
import { TooltipProvider } from "@/components/ui/tooltip";
import { HotkeysContext } from "@/store/hotkeysContext";
import { loading, ready } from "@/store/loadable";
import type { SaveOutcome } from "@/store/saveOutcome";
import type { ScratchpadActionsStore } from "@/store/useScratchpadActions";
import type { ScratchpadEditorStore } from "@/store/useScratchpadEditor";
import type { HotkeyBindingView, ScratchpadSummary } from "@/domain";

// The board's two hooks are the only IPC on this surface; stubbing them keeps this file on the
// board's arrangement (grouping, filtering, which panel is showing, which document is open) rather
// than on writes, which the hooks' own suites cover. Each stub is typed as the store it stands in
// for, so a member added to the real hook fails the typecheck here instead of leaving the board
// under test wired to a shape the app no longer has.
const archived: [string, boolean][] = [];
const created: [string, string][] = [];
let createOutcome: SaveOutcome = "saved";

vi.mock("@/store/useScratchpadActions", () => ({
  useScratchpadActions: (): ScratchpadActionsStore => ({
    error: null,
    createError: null,
    create: async (name: string, body: string) => {
      created.push([name, body]);
      return createOutcome;
    },
    archive: (name: string, flag: boolean) => void archived.push([name, flag]),
    exportMarkdown: vi.fn(),
    copyMarkdown: vi.fn(),
    clearError: vi.fn(),
  }),
}));

/** The revision the stubbed read answers with — what a bumped summary is measured against. */
const LOADED_REVISION = 1;
const reload = vi.fn();

vi.mock("@/store/useScratchpadEditor", () => ({
  useScratchpadEditor: (): ScratchpadEditorStore => {
    // `open` and `close` genuinely move the stub between having a document and not, as the real
    // hook does, so a test can watch the board open and drop a document rather than watch it call a
    // spy — which would pass just as happily if the board opened the wrong one.
    const [open, setOpen] = useState<string | null>(null);
    return {
      name: open,
      document:
        open == null
          ? loading()
          : ready({
              id: 0,
              name: open,
              tags: [],
              archived: false,
              revision: LOADED_REVISION,
              body: `body of ${open}`,
              rendered: "",
            }),
      baseRevision: open == null ? null : LOADED_REVISION,
      mountKey: 0,
      conflict: null,
      error: null,
      open: setOpen,
      close: () => setOpen(null),
      save: vi.fn(),
      reload,
      rename: vi.fn(),
      copyLink: vi.fn(),
    };
  },
}));

// The create form and the edit surface both mount the lazy rich editor, and so does the read view's
// renderer; standing it in keeps this file on the board's arrangement.
vi.mock("@/components/editor/LazyRichTextEditor", () => ({
  LazyRichTextEditor: (props: { initialMarkdown: string; ariaLabel?: string }) => (
    <div data-testid="rich-text" aria-label={props.ariaLabel}>
      {props.initialMarkdown}
    </div>
  ),
}));

// The board persists its group's collapse state through `localStorage`, which this environment does
// not provide; an in-memory stand-in makes that round trip real rather than silently swallowed.
const stored = new Map<string, string>();
vi.stubGlobal("localStorage", {
  getItem: (key: string) => stored.get(key) ?? null,
  setItem: (key: string, value: string) => void stored.set(key, value),
  removeItem: (key: string) => void stored.delete(key),
  clear: () => stored.clear(),
});

afterEach(() => {
  cleanup();
  localStorage.clear();
  archived.length = 0;
  created.length = 0;
  createOutcome = "saved";
  vi.clearAllMocks();
});

/** The one section header the board draws, as the board words it. */
const ARCHIVED_GROUP_LABEL = "Archived";

const ARCHIVE_HOTKEY: HotkeyBindingView = {
  action: "archive_scratchpad",
  scope: "scratchpad",
  binding: { ctrl: true, alt: false, shift: true, super: false, key: "W" },
  is_default: true,
  conflict: false,
};

function pad(overrides: Partial<ScratchpadSummary>): ScratchpadSummary {
  return {
    id: 0,
    name: "",
    tags: [],
    archived: false,
    revision: LOADED_REVISION,
    gist: "",
    updated_at: 0,
    ...overrides,
  };
}

const design = pad({
  id: 1,
  name: "rich-editor-design",
  gist: "three revisions",
  tags: ["design"],
  updated_at: 300,
});
const notes = pad({ id: 2, name: "release-notes", gist: "what shipped", updated_at: 200 });
const retired = pad({ id: 3, name: "old-plan", archived: true, updated_at: 100 });

const pads = [design, notes, retired];

// The detail panel's Copy link and overflow controls are Tooltip triggers, and its archive hotkey
// reads the live keymap — both supplied here as the app supplies them once at its root.
function mount(props: Partial<React.ComponentProps<typeof ScratchpadBoard>>) {
  return (
    <TooltipProvider>
      <HotkeysContext
        value={{
          bindings: [ARCHIVE_HOTKEY],
          remap: vi.fn(),
          disable: vi.fn(),
          reset: vi.fn(),
          resetAll: vi.fn(),
        }}
      >
        <ScratchpadBoard project={1} scratchpads={pads} {...props} />
      </HotkeysContext>
    </TooltipProvider>
  );
}

function board(
  scratchpads: ScratchpadSummary[] = pads,
  overrides: Partial<React.ComponentProps<typeof ScratchpadBoard>> = {},
) {
  return render(mount({ scratchpads, ...overrides }));
}

/** Re-renders the board under test with `props` merged over the defaults. */
function rerenderBoard(
  rerender: (ui: React.ReactElement) => void,
  props: Partial<React.ComponentProps<typeof ScratchpadBoard>>,
) {
  rerender(mount(props));
}

/** The board's group headers, in render order, read off the handle carrying the label itself. */
function groupLabels(): string[] {
  return [...document.querySelectorAll(`[${GROUP_ATTRIBUTE}]`)].map(
    (header) => header.getAttribute(GROUP_ATTRIBUTE) ?? "",
  );
}

/** The scratchpads on screen, in the order the board lays them out. */
function rowNames(): string[] {
  return [...document.querySelectorAll(`[${SCRATCHPAD_ROW_NAME_ATTRIBUTE}]`)].map(
    (row) => row.getAttribute(SCRATCHPAD_ROW_NAME_ATTRIBUTE) ?? "",
  );
}

/** The Archived section's header — the control that collapses it. An archived card wears the same
 *  word on its own status chip, so the group is addressed by its handle rather than by that word. */
function archivedGroup(): HTMLElement {
  return document.querySelector<HTMLElement>(
    `[${GROUP_ATTRIBUTE}="${ARCHIVED_GROUP_LABEL}"]`,
  ) as HTMLElement;
}

/** Which panel the board is showing — the route, not merely which panel is mounted. */
function route(): string | null {
  return (
    document.querySelector(`[${PANEL_ROUTE_ATTRIBUTE}]`)?.getAttribute(PANEL_ROUTE_ATTRIBUTE) ??
    null
  );
}

function panel(name: "list" | "detail"): HTMLElement {
  return document.querySelector<HTMLElement>(`[${PANEL_ATTRIBUTE}="${name}"]`) as HTMLElement;
}

/** The card that opens scratchpad `id` — the same handle the end-to-end walks aim at. */
function card(id: number): HTMLElement {
  return document.querySelector<HTMLElement>(
    `[${SCRATCHPAD_ROW_ID_ATTRIBUTE}="${id}"] [${CARD_TRIGGER_ATTRIBUTE}]`,
  ) as HTMLElement;
}

function backButton(): HTMLElement {
  return panel("detail").querySelector<HTMLElement>(`[${DETAIL_BACK_ATTRIBUTE}]`) as HTMLElement;
}

function searchBox(): HTMLInputElement {
  return screen.getByRole("searchbox", { name: "Search scratchpads" }) as HTMLInputElement;
}

function count(): string {
  return document.querySelector(`[${BOARD_COUNT_ATTRIBUTE}]`)?.textContent ?? "";
}

/** The collapse keys the board actually persisted, whichever bucket they were stored in. */
function collapsedKeys(): string[] {
  return [...stored.values()].flatMap((value) =>
    Object.keys(JSON.parse(value) as Record<string, boolean>),
  );
}

function archiveChord(target: HTMLElement) {
  fireEvent.keyDown(target, { key: "w", ctrlKey: true, shiftKey: true });
}

describe("ScratchpadBoard", () => {
  it("lists the active scratchpads first and gathers the archived ones under their own header", () => {
    board();

    expect(rowNames()).toEqual(["rich-editor-design", "release-notes", "old-plan"]);
    expect(groupLabels()).toEqual([ARCHIVED_GROUP_LABEL]);
    // The header states how many are behind it, so collapsing it never hides an unknown quantity.
    expect(archivedGroup().textContent).toContain("1");
  });

  it("draws no group at all when nothing is archived", () => {
    board([design, notes]);

    expect(groupLabels()).toEqual([]);
    expect(rowNames()).toEqual(["rich-editor-design", "release-notes"]);
  });

  it("remembers a collapsed Archived group across a remount, under this board's own key", () => {
    const { unmount } = board();

    fireEvent.click(archivedGroup());
    expect(rowNames()).toEqual(["rich-editor-design", "release-notes"]);
    // Namespaced, so the choice cannot collide with another list's group of the same name.
    expect(collapsedKeys()).toEqual(["scratchpads.archived"]);

    unmount();
    board();

    expect(rowNames()).toEqual(["rich-editor-design", "release-notes"]);
  });

  it("flattens the board while a search is active, and restores the group when it clears", () => {
    board();

    fireEvent.change(searchBox(), { target: { value: "release" } });

    // A search is already a triage question, so the matches are not buried under a header.
    expect(groupLabels()).toEqual([]);
    expect(rowNames()).toEqual(["release-notes"]);
    expect(count()).toBe("1 of 3");

    fireEvent.change(searchBox(), { target: { value: "" } });

    expect(groupLabels()).toEqual([ARCHIVED_GROUP_LABEL]);
    expect(count()).toBe("3 of 3");
  });

  it("keeps the search box live and settles the rows to the match, even over a large board", () => {
    // Filtering runs off a deferred copy of the toolbar's filter, so the search box itself must
    // never wait on it — this is the surface that would visibly lag if the input were bound to
    // anything but the live keystroke.
    const many = Array.from({ length: 3000 }, (_, i) => pad({ id: i, name: `note-number-${i}` }));
    board(many);

    fireEvent.change(searchBox(), { target: { value: "number 1234" } });

    expect(searchBox().value).toBe("number 1234");
    // The deferred derivation settles to exactly the matching row — never the unfiltered 3000, and
    // never a mix of the two.
    expect(rowNames()).toEqual(["note-number-1234"]);
  });

  it("says the board is empty only when it is, and says so differently from no matches", () => {
    board([]);
    expect(screen.getByText("No scratchpads yet")).toBeTruthy();

    cleanup();
    board();
    fireEvent.change(searchBox(), { target: { value: "nothing here" } });

    expect(screen.queryByText("No scratchpads yet")).toBeNull();
    expect(screen.getByText("No scratchpads match your search.")).toBeTruthy();
  });

  it("reorders the rows when the sort changes from recency to name", () => {
    board([design, notes]);
    expect(rowNames()).toEqual(["rich-editor-design", "release-notes"]);

    fireEvent.click(screen.getByRole("combobox", { name: "Sort scratchpads" }));
    fireEvent.click(screen.getByRole("option", { name: "Name" }));

    expect(rowNames()).toEqual(["release-notes", "rich-editor-design"]);
  });

  it("offers one create action at a time, and closes the form only once a create lands", async () => {
    board();
    fireEvent.click(screen.getByRole("button", { name: /New scratchpad/ }));

    // The form's own Create is now the default action; two filled buttons would each claim to be it.
    expect(screen.queryByRole("button", { name: /New scratchpad/ })).toBeNull();
    fireEvent.change(screen.getByRole("textbox", { name: "New scratchpad name" }), {
      target: { value: "fresh-notes" },
    });

    createOutcome = "refused";
    fireEvent.click(screen.getByRole("button", { name: /Create scratchpad/ }));
    await waitFor(() => expect(created).toHaveLength(1));

    // A refusal keeps the draft on screen: the name the user typed is not thrown away.
    expect(screen.getByRole("textbox", { name: "New scratchpad name" })).toBeTruthy();

    createOutcome = "saved";
    fireEvent.click(screen.getByRole("button", { name: /Create scratchpad/ }));

    expect(await screen.findByRole("button", { name: /New scratchpad/ })).toBeTruthy();
    expect(screen.queryByRole("textbox", { name: "New scratchpad name" })).toBeNull();
    // Creating leaves the reader on the list — nothing was opened.
    expect(route()).toBe("list");
  });

  it("hands the pane to a scratchpad's detail when its card is opened", () => {
    board();
    expect(route()).toBe("list");

    fireEvent.click(card(design.id));

    expect(route()).toBe("detail");
    expect(
      within(panel("detail")).getByRole("heading", { name: "Rich editor design" }),
    ).toBeTruthy();
    // The list goes inert the moment the detail shows; focus left on the card behind it would be
    // dropped to the document body and restart keyboard traversal at the top of the app.
    expect(panel("list").hasAttribute("inert")).toBe(true);
    expect(document.activeElement).toBe(backButton());
  });

  it("returns to the list on Back, with focus back on the card it came from", () => {
    board();
    fireEvent.click(card(design.id));

    fireEvent.click(backButton());

    expect(route()).toBe("list");
    expect(document.activeElement).toBe(card(design.id));
  });

  it("reaches a target the list is hiding, without clearing the filter or expanding the group", () => {
    const { rerender } = board();
    fireEvent.click(archivedGroup());
    fireEvent.change(searchBox(), { target: { value: "release" } });
    expect(rowNames()).toEqual(["release-notes"]);

    rerenderBoard(rerender, { focusName: "old-plan", focusNonce: 10 });

    // The detail shows the scratchpad whatever the list is doing, so nothing has to be revealed to
    // reach it — and the list the reader returns to is still arranged the way they left it.
    expect(route()).toBe("detail");
    expect(within(panel("detail")).getByRole("heading", { name: "Old plan" })).toBeTruthy();
    expect(searchBox().value).toBe("release");
  });

  it("opens the target once it arrives, when the nonce was set before the scratchpads were", () => {
    // Mirrors a pane that mounts fresh and asks for a target before its first snapshot lands.
    const { rerender } = board([], { focusName: "release-notes", focusNonce: 20 });
    expect(route()).toBe("list");

    rerenderBoard(rerender, { scratchpads: pads, focusName: "release-notes", focusNonce: 20 });

    expect(route()).toBe("detail");
    expect(document.activeElement).toBe(backButton());
  });

  it("opens on the list when an activation it already acted on is still standing at mount", () => {
    // The pane leaves `focus` set after acting on it and unmounts the board whenever the user
    // switches view, so switching away and back re-delivers the same activation to a fresh board. A
    // nonce is one navigation, not a standing instruction to keep reopening the detail.
    const first = board(pads, { focusName: "release-notes", focusNonce: 30 });
    expect(route()).toBe("detail");
    first.unmount();

    board(pads, { focusName: "release-notes", focusNonce: 30 });

    expect(route()).toBe("list");
  });

  it("re-reads an open scratchpad a concurrent write moved past, while it is being read", () => {
    const { rerender } = board();
    fireEvent.click(card(design.id));
    expect(reload).not.toHaveBeenCalled();

    rerenderBoard(rerender, {
      scratchpads: [{ ...design, revision: LOADED_REVISION + 1 }, notes, retired],
    });

    expect(reload).toHaveBeenCalledTimes(1);
  });

  it("leaves an open editor alone when a concurrent write lands, rather than reloading under it", () => {
    const { rerender } = board();
    fireEvent.click(card(design.id));
    fireEvent.click(within(panel("detail")).getByRole("button", { name: /Edit/ }));

    rerenderBoard(rerender, {
      scratchpads: [{ ...design, revision: LOADED_REVISION + 1 }, notes, retired],
    });

    // Re-reading here would replace the text under the caret with someone else's.
    expect(reload).not.toHaveBeenCalled();
  });

  it("archives the open scratchpad on the chord, and does nothing with no scratchpad open", () => {
    board();
    archiveChord(searchBox());
    expect(archived).toEqual([]);

    fireEvent.click(card(design.id));
    archiveChord(backButton());

    expect(archived).toEqual([["rich-editor-design", true]]);
  });

  it("restores an archived scratchpad on the same chord, rather than archiving it twice", () => {
    board();
    fireEvent.click(card(retired.id));

    archiveChord(backButton());

    expect(archived).toEqual([["old-plan", false]]);
  });
});
