// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { DETAIL_BACK_ATTRIBUTE, DETAIL_DONE_ATTRIBUTE } from "@/components/common/DetailPane";
import { AUTOSAVE_STATUS_ATTRIBUTE } from "@/components/editor/AutosaveStatus";
import {
  SCRATCHPAD_DETAIL_ATTRIBUTE,
  ScratchpadDetail,
  type ScratchpadEditState,
} from "@/components/orchestration/ScratchpadDetail";
import {
  SCRATCHPAD_HANDLE_ATTRIBUTE,
  SCRATCHPAD_REVISION_ATTRIBUTE,
} from "@/components/orchestration/ScratchpadMeta";
import { TooltipProvider } from "@/components/ui/tooltip";
import { failed, loading, ready } from "@/store/loadable";
import type { ScratchpadSummary, ScratchpadView } from "@/domain";

// The rich editor is a lazy TipTap surface that needs real layout; standing in for it keeps this
// file on what the pane does — which renderer it hands the body to, and how — rather than on TipTap's
// own Markdown parsing. The `?? true` defaults make the stub fail loudly if the read view ever stops
// passing `editable={false}` / `toolbar={false}`, instead of silently reading undefined.
vi.mock("@/components/editor/LazyRichTextEditor", () => ({
  LazyRichTextEditor: (props: {
    initialMarkdown: string;
    editable?: boolean;
    toolbar?: boolean;
    ariaLabel?: string;
  }) => (
    <div
      data-testid="rich-text"
      data-editable={String(props.editable ?? true)}
      data-toolbar={String(props.toolbar ?? true)}
      aria-label={props.ariaLabel}
    >
      {props.initialMarkdown}
    </div>
  ),
}));

afterEach(cleanup);

const NOW = new Date(2026, 0, 20, 12).getTime();
const MINUTE = 60_000;

function pad(overrides: Partial<ScratchpadSummary> = {}): ScratchpadSummary {
  return {
    id: 4,
    name: "release-plan",
    tags: [],
    archived: false,
    revision: 3,
    gist: "the plan",
    updated_at: NOW - 5 * MINUTE,
    ...overrides,
  };
}

function view(overrides: Partial<ScratchpadView> = {}): ScratchpadView {
  return {
    id: 4,
    name: "release-plan",
    tags: [],
    archived: false,
    revision: 3,
    body: "the plan",
    rendered: "# release-plan\n\nthe plan",
    ...overrides,
  };
}

function editState(overrides: Partial<ScratchpadEditState> = {}): ScratchpadEditState {
  return {
    mountKey: 0,
    conflict: null,
    error: null,
    onSave: vi.fn(async () => "saved" as const),
    onReload: vi.fn(),
    onRename: vi.fn(async () => {}),
    onDone: vi.fn(),
    onBodyChange: vi.fn(),
    ...overrides,
  };
}

function pane(overrides: Partial<Parameters<typeof ScratchpadDetail>[0]> = {}) {
  return render(
    <TooltipProvider delayDuration={0}>
      <ScratchpadDetail
        pad={pad()}
        document={ready(view())}
        onRetry={vi.fn()}
        now={NOW}
        onBack={vi.fn()}
        onStartEdit={vi.fn()}
        onArchive={vi.fn()}
        onCopyLink={vi.fn()}
        onExport={vi.fn()}
        onCopyMarkdown={vi.fn()}
        error={null}
        edit={null}
        {...overrides}
      />
    </TooltipProvider>,
  );
}

const root = () => document.querySelector(`[${SCRATCHPAD_DETAIL_ATTRIBUTE}]`) as HTMLElement;
const header = () => root().querySelector(":scope > header") as HTMLElement;
const body = () => screen.getByTestId("rich-text");

describe("ScratchpadDetail", () => {
  // The pane must never claim a document is empty before its read has landed: an empty state shown
  // during the wait tells the reader the scratchpad has nothing in it, which is a lie the moment the
  // body arrives.
  it("waits for the body rather than declaring it empty", () => {
    pane({ document: loading() });

    expect(screen.queryByText("No content yet.")).toBeNull();
    expect(screen.queryByTestId("rich-text")).toBeNull();

    const region = screen.getByRole("status");
    expect(region.getAttribute("aria-busy")).toBe("true");
    expect(region.textContent).toContain("Loading scratchpad");
  });

  it("renders the body read-only and without editing chrome", () => {
    pane({ document: ready(view({ body: "## Acceptance\n\n- one" })) });

    expect(body().textContent).toContain("## Acceptance");
    expect(body().dataset.editable).toBe("false");
    expect(body().dataset.toolbar).toBe("false");
  });

  it("says a scratchpad has nothing in it rather than showing a blank pane", () => {
    pane({ document: ready(view({ body: "" })) });

    expect(screen.queryByTestId("rich-text")).toBeNull();
    expect(screen.getByText("No content yet.")).toBeTruthy();
  });

  it("offers a retry when the body could not be read", () => {
    const onRetry = vi.fn();
    pane({ document: failed("no such scratchpad"), onRetry });

    expect(screen.getByText("Could not load scratchpad.")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it("names the scratchpad in the header, humanized, over its meta rail", () => {
    pane({ pad: pad({ name: "release-plan", revision: 7, tags: ["infra"] }) });

    expect(within(header()).getByRole("heading", { level: 2 }).textContent).toBe("Release plan");
    expect(header().querySelector(`[${SCRATCHPAD_HANDLE_ATTRIBUTE}]`)?.textContent).toBe(
      "release-plan",
    );
    expect(header().querySelector(`[${SCRATCHPAD_REVISION_ATTRIBUTE}]`)?.textContent).toBe("r7");
    expect(within(header()).getByText("5 min ago")).toBeTruthy();
    expect(within(header()).getByText("infra")).toBeTruthy();
  });

  // Every action works on the body, so none is offered until the body is here — editing a document
  // nobody has read yet would open an editor over text that is still in flight.
  it("withholds the actions until the body has landed", () => {
    pane({ document: loading() });
    expect((screen.getByRole("button", { name: "Edit" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Export .md" }) as HTMLButtonElement).disabled).toBe(
      true,
    );
    cleanup();

    pane();
    expect((screen.getByRole("button", { name: "Edit" }) as HTMLButtonElement).disabled).toBe(
      false,
    );
    expect((screen.getByRole("button", { name: "Export .md" }) as HTMLButtonElement).disabled).toBe(
      false,
    );
  });

  it("hands the pane to the editor, with the rename field and the way out in the header", () => {
    const onDone = vi.fn();
    pane({ edit: editState({ onDone }) });

    expect(
      within(header()).getByRole("button", { name: "Rename scratchpad Release plan" }),
    ).toBeTruthy();
    expect(document.querySelector(`[${AUTOSAVE_STATUS_ATTRIBUTE}]`)).toBeTruthy();

    const done = within(header()).getByRole("button", { name: "Done" });
    expect(done.getAttribute(DETAIL_DONE_ATTRIBUTE)).not.toBeNull();
    expect(screen.queryByRole("button", { name: "Edit" })).toBeNull();

    fireEvent.click(done);
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it("offers to restore an archived scratchpad rather than archiving it again", () => {
    pane({ pad: pad({ archived: true }) });

    expect(screen.getByRole("button", { name: "Restore" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Archive" })).toBeNull();
  });

  it("names the revision a concurrent write moved the scratchpad to, and reloads to it", () => {
    const onReload = vi.fn();
    pane({ edit: editState({ conflict: { actual: 9 }, onReload }) });

    expect(screen.getByText(/now at revision 9/)).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Reload" }));
    expect(onReload).toHaveBeenCalledTimes(1);
  });

  // The refusal answers a control in the pinned header, so it may not live somewhere the reader has
  // already scrolled past by the time it arrives.
  it("announces a refusal in a strip pinned outside the scrolling content", () => {
    pane({ error: "scratchpad release-plan is archived" });

    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("scratchpad release-plan is archived");
    expect(alert.parentElement).toBe(root());
  });

  it("returns to the board from the back control", () => {
    const onBack = vi.fn();
    pane({ onBack });

    const back = document.querySelector(`[${DETAIL_BACK_ATTRIBUTE}]`) as HTMLButtonElement;
    expect(back.getAttribute("aria-label")).toBe("Back to scratchpads");

    fireEvent.click(back);
    expect(onBack).toHaveBeenCalledTimes(1);
  });
});
