// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { TodoDetail, type TodoEditState } from "@/components/orchestration/TodoDetail";
import { TODO_STATUS, TODO_STATUS_ORDER, TODO_STATUS_TONE } from "@/lib/todo";
import type { ScratchpadRef, TodoStatus, TodoView } from "@/domain";

// The rich editor is a lazy TipTap surface that needs real layout; standing in for it keeps this
// file on what the panel does — which renderer it hands each body to, and how — rather than on
// TipTap's own Markdown parsing, which `markdownRoundTrip` already covers. The `?? true` defaults
// make the stub fail loudly if a caller ever stops passing `editable={false}` / `toolbar={false}`,
// instead of silently reading undefined.
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

const plan: ScratchpadRef = { id: 4, name: "release-plan" };

function todo(overrides: Partial<TodoView> = {}): TodoView {
  return {
    id: 1,
    doc: { title: "Ship the release", body: "", status: "open" },
    tags: [],
    blockers: [],
    blocked_by: [],
    blocked: false,
    comments: [],
    locked_by: null,
    scratchpad: null,
    revision: 1,
    ...overrides,
  };
}

function editState(overrides: Partial<TodoEditState> = {}): TodoEditState {
  return {
    initial: { title: "Ship the release", body: "", status: "open" },
    initialScratchpad: null,
    mountKey: 0,
    conflict: null,
    error: null,
    onSave: vi.fn(async () => "saved" as const),
    onReload: vi.fn(),
    onDone: vi.fn(),
    ...overrides,
  };
}

function element(overrides: Partial<Parameters<typeof TodoDetail>[0]> = {}) {
  return (
    <TodoDetail
      todo={todo()}
      onBack={vi.fn()}
      titleOf={() => undefined}
      lockOwnerLabel={undefined}
      busy={false}
      error={undefined}
      onComplete={vi.fn()}
      onCopyLink={vi.fn()}
      onComment={vi.fn()}
      onStartEdit={vi.fn()}
      scratchpads={[]}
      edit={null}
      {...overrides}
    />
  );
}

const panel = (overrides: Partial<Parameters<typeof TodoDetail>[0]> = {}) =>
  render(element(overrides));

const root = () => document.querySelector("[data-todo-detail]") as HTMLElement;
const header = () => root().querySelector(":scope > header") as HTMLElement;
const statusChip = () => document.querySelector("[data-todo-status]") as HTMLElement;
const avatars = () => [...document.querySelectorAll('[data-slot="avatar"]')] as HTMLElement[];
const region = (name: string) => screen.getByRole("region", { name });
const bodyIn = (name: string) => within(region(name)).getByTestId("rich-text");

describe("TodoDetail", () => {
  // The handle carries which todo is open, so a caller proves the panel landed on the right one by
  // name rather than by reading the heading — and `[data-todo-detail]` still matches, which is what
  // the board relies on to mean "a todo is loaded".
  it("names the todo the panel is showing", () => {
    panel({ todo: todo({ id: 42 }) });

    expect(root().dataset.todoDetail).toBe("42");
    expect(document.querySelectorAll("[data-todo-detail]").length).toBe(1);
  });

  it("renders the description through the Markdown renderer instead of printing its source", () => {
    panel({
      todo: todo({
        doc: { title: "Ship the release", body: "## Acceptance\n\n- one\n- two", status: "open" },
      }),
    });

    const description = region("Description");
    expect(bodyIn("Description").textContent).toContain("## Acceptance");
    // The raw text reaches the renderer, not a paragraph of its own: nothing in the region prints
    // the Markdown source itself, which is what would leave `##` and `-` on screen.
    expect(description.querySelector(".whitespace-pre-wrap")).toBeNull();
  });

  it("renders the description read-only and without editing chrome", () => {
    panel({
      todo: todo({ doc: { title: "Ship the release", body: "Some detail", status: "open" } }),
    });

    const body = bodyIn("Description");
    expect(body.dataset.editable).toBe("false");
    expect(body.dataset.toolbar).toBe("false");
  });

  it("says a todo has no description rather than dropping the region", () => {
    panel();

    expect(screen.queryByTestId("rich-text")).toBeNull();
    expect(region("Description").textContent).toContain("No description.");
  });

  // The user's report: a comment showed its Markdown source. A comment is a document like any other
  // and goes through the one renderer, so `##` and `-` are formatting here too, not literal text.
  it("renders a comment body through the Markdown renderer instead of printing its source", () => {
    panel({ todo: todo({ comments: [{ id: 1, body: "## Findings\n\n- one", author: null }] }) });

    const thread = region("Comments");
    const body = within(thread).getByTestId("rich-text");
    expect(body.textContent).toContain("## Findings");
    expect(body.dataset.editable).toBe("false");
    expect(thread.querySelector(".whitespace-pre-wrap")).toBeNull();
  });

  it("offers the comment composer under its own titled section even with no comments", () => {
    panel();

    expect(screen.getByRole("heading", { name: "Comments" })).toBeTruthy();
    expect(screen.getByLabelText("Add a comment")).toBeTruthy();
  });

  // The description is keyed by the todo's revision, and only a document write bumps that — so a
  // posted comment must leave the rendered body alone rather than tearing down a TipTap instance.
  it("leaves the description mounted when a comment arrives", () => {
    const subject = todo({
      doc: { title: "Ship the release", body: "Some detail", status: "open" },
    });
    const { rerender } = panel({ todo: subject });
    const before = bodyIn("Description");

    rerender(element({ todo: { ...subject, comments: [{ id: 1, body: "noted", author: null }] } }));

    expect(bodyIn("Description")).toBe(before);
  });

  // The tone is a colour and jsdom computes no styles, so what is observable is the wiring: the chip
  // takes a distinct treatment per status, and the label declares a colour of its own instead of
  // inheriting the chip's. The `--status-*` hues measure as low as 2.48:1 against the light theme's
  // card, so a label left to inherit one is unreadable — this reddens both if the tone stops varying
  // and if the label loses its own ink.
  it("tones the chip per status while the label keeps a colour of its own", () => {
    const chipTones = new Set<string>();

    for (const status of TODO_STATUS_ORDER) {
      panel({ todo: todo({ doc: { title: "Ship the release", body: "", status } }) });
      const chip = statusChip();
      chipTones.add(chip.className);

      const label = within(chip).getByText(TODO_STATUS[status]);
      const labelColours = label.className.split(/\s+/).filter((name) => name.startsWith("text-"));
      expect(labelColours.length).toBeGreaterThan(0);
      expect(labelColours).not.toContain(TODO_STATUS_TONE[status]);

      cleanup();
    }

    expect(chipTones.size).toBe(TODO_STATUS_ORDER.length);
  });

  it("marks the chip with the status it is showing", () => {
    const status: TodoStatus = "blocked";
    panel({ todo: todo({ doc: { title: "Ship the release", body: "", status } }) });

    expect(statusChip().dataset.status).toBe(status);
    expect(within(statusChip()).getByText(TODO_STATUS[status])).toBeTruthy();
  });

  // The header is the pane's only pinned row, so the actions stay reachable however far the
  // discussion below has been scrolled.
  it("puts the actions in the pinned header, not in the scrolling content", () => {
    panel();

    expect(within(header()).getByRole("button", { name: "Edit" })).toBeTruthy();
    expect(within(header()).getByRole("button", { name: "Complete" })).toBeTruthy();
    expect(within(header()).getByRole("button", { name: "Copy link to todo" })).toBeTruthy();
  });

  // Band 1 is two clusters at one control height; band 3 is chips centred against each other.
  // Nothing in the header is baseline-aligned, because a chip is a flex box and contributes its
  // content's baseline from the middle of its own box — which is what put the status chip 3-4px
  // below the title and is the misalignment this header was rebuilt to design out.
  it("aligns the header's bands by centring, never by baseline", () => {
    panel({ todo: todo({ tags: ["infra"] }) });

    const rows = [...header().children] as HTMLElement[];
    expect(rows.length).toBeGreaterThanOrEqual(3);
    for (const row of rows) {
      expect(row.className).not.toContain("items-baseline");
    }
    // The title is alone on its line, so nothing shares a row with it to disagree about.
    const title = screen.getByRole("heading", { level: 2 });
    expect(title.parentElement).toBe(header());
  });

  it("keeps the id, status, blocker gate and tags together in one meta rail", () => {
    panel({
      todo: todo({ blockers: [2], blocked_by: [2], blocked: true, tags: ["infra"] }),
    });

    const rail = statusChip().parentElement as HTMLElement;
    expect(rail.textContent).toContain("#1");
    expect(rail.textContent).toContain("1 unmet blocker");
    expect(within(rail).getByText("infra")).toBeTruthy();
  });

  // Below a 15rem container the two secondary actions become menu items. Both forms exist in the
  // DOM under mutually exclusive container queries, so exactly one of them is ever reachable —
  // jsdom applies no CSS, which is why this asserts the queries rather than the visibility.
  it("offers the secondary actions inline or in a menu, never as two live copies", () => {
    panel();

    const inline = within(header()).getByRole("button", { name: "Edit" })
      .parentElement as HTMLElement;
    const menu = within(header()).getByRole("button", { name: "More actions" });

    expect(inline.className).toContain("@max-[15rem]/detail-header:hidden");
    expect(menu.className).toContain("@min-[15rem]/detail-header:hidden");
  });

  it("offers no Complete on a todo that is already done", () => {
    panel({ todo: todo({ doc: { title: "Ship the release", body: "", status: "done" } }) });

    expect(screen.queryByRole("button", { name: "Complete" })).toBeNull();
    expect(screen.getByRole("button", { name: "Edit" })).toBeTruthy();
  });

  it("names the in-flight completion and refuses a second press", () => {
    panel({ busy: true });

    const complete = screen.getByRole("button", { name: "Completing…" }) as HTMLButtonElement;
    expect(complete.disabled).toBe(true);
  });

  it("completes through the core rather than pre-empting the blocked gate", () => {
    const onComplete = vi.fn();
    panel({ todo: todo({ blockers: [2], blocked_by: [2], blocked: true }), onComplete });

    const complete = screen.getByRole("button", { name: "Complete" }) as HTMLButtonElement;
    expect(complete.disabled).toBe(false);

    fireEvent.click(complete);
    expect(onComplete).toHaveBeenCalledTimes(1);
  });

  // The refusal answers a control in the pinned header, so it may not live somewhere the reader has
  // already scrolled past by the time it arrives.
  it("announces a refusal in a strip pinned outside the scrolling content", () => {
    panel({ error: "Todo 1 is blocked by 1 unmet blocker" });

    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("Todo 1 is blocked by 1 unmet blocker");
    expect(alert.parentElement).toBe(root());
  });

  it("leads with the blockers, above the description", () => {
    panel({ todo: todo({ blockers: [2], blocked_by: [2], blocked: true }) });

    const following = region("Blockers").compareDocumentPosition(region("Description"));
    expect(following & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("sets the id in mono, matching how the list row prints the same value", () => {
    panel({ todo: todo({ id: 42 }) });

    const reference = screen.getByText("#42");
    expect(reference.className).toContain("font-mono");
    expect(reference.className).toContain("tabular-nums");
  });

  it("names the scratchpad a todo derives from, humanized", () => {
    panel({ todo: todo({ scratchpad: plan }) });

    expect(screen.getByText("Release plan")).toBeTruthy();
  });

  it("says plainly that a todo derives from no scratchpad rather than hiding the field", () => {
    panel({ todo: todo({ scratchpad: null }) });

    expect(screen.getByText("Not derived from a scratchpad")).toBeTruthy();
  });

  it("replaces the read regions and the actions with the editor rather than showing both", () => {
    const subject = todo({ scratchpad: plan, comments: [{ id: 1, body: "noted", author: null }] });
    panel({ todo: subject });
    expect(screen.getByText("Release plan")).toBeTruthy();
    cleanup();

    // A stale scratchpad shown beside the editor's own picker would name two different documents as
    // the todo's origin at once, so the read view has to go, not merely be covered. The actions go
    // with it — Complete on a document mid-edit acts on the text the editor has not saved yet.
    panel({ todo: subject, edit: editState() });

    expect(screen.queryByText("Release plan")).toBeNull();
    expect(screen.queryByRole("heading", { name: "Comments" })).toBeNull();
    expect(screen.queryByLabelText("Add a comment")).toBeNull();
    expect(screen.queryByRole("button", { name: "Complete" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Edit" })).toBeNull();
  });

  it("keeps the back control available while the todo is being edited", () => {
    const onBack = vi.fn();
    panel({ edit: editState(), onBack });

    fireEvent.click(document.querySelector("[data-todo-back]") as HTMLElement);

    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it("returns to the board from the back control", () => {
    const onBack = vi.fn();
    panel({ onBack });

    const back = document.querySelector("[data-todo-back]") as HTMLButtonElement;
    expect(back.tagName).toBe("BUTTON");
    expect(back.getAttribute("aria-label")).toBe("Back to todos");

    fireEvent.click(back);
    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it("marks an unattributed comment with a glyph, not a letter cut from the word", () => {
    panel({ todo: todo({ comments: [{ id: 1, body: "noted", author: null }] }) });

    expect(screen.getByText("unattributed")).toBeTruthy();
    const [avatar] = avatars();
    expect(avatar.querySelector("svg")).not.toBeNull();
    expect(avatar.textContent).toBe("");
  });

  it("monograms an attributed comment's author", () => {
    panel({
      todo: todo({
        comments: [{ id: 1, body: "noted", author: { kind: "process", id: 3, label: "worker-a" } }],
      }),
    });

    expect(screen.getByText("worker-a")).toBeTruthy();
    const [avatar] = avatars();
    expect(avatar.textContent).toBe("W");
    expect(avatar.querySelector("svg")).toBeNull();
  });

  it("distinguishes the blockers still holding the todo from the ones already met", () => {
    panel({
      todo: todo({ blockers: [2, 3], blocked_by: [3], blocked: true }),
      titleOf: (id) => `Todo number ${id}`,
    });

    const rows = screen.getAllByRole("listitem");
    const met = rows.find((row) => row.textContent?.includes("Todo number 2")) as HTMLElement;
    const unmet = rows.find((row) => row.textContent?.includes("Todo number 3")) as HTMLElement;

    expect(within(met).getByText("done")).toBeTruthy();
    expect(within(unmet).getByText("open")).toBeTruthy();
    expect(screen.getByText("1 unmet blocker")).toBeTruthy();
  });
});
