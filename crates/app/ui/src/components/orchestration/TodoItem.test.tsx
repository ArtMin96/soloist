// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TodoItem } from "@/components/orchestration/TodoItem";
import type { ScratchpadRef, TodoView } from "@/domain";

// The rich editor is a lazy TipTap surface that needs real layout; standing in for it keeps this
// test on what the row does — which renderer it hands the body to, and how — rather than on
// TipTap's own Markdown parsing, which `markdownRoundTrip` already covers.
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

function row(overrides: Partial<Parameters<typeof TodoItem>[0]> = {}) {
  return render(
    <TodoItem
      todo={todo()}
      open
      onToggle={vi.fn()}
      titleOf={() => undefined}
      lockOwnerLabel={undefined}
      busy={false}
      error={undefined}
      onComplete={vi.fn()}
      onCopyLink={vi.fn()}
      onComment={vi.fn()}
      onStartEdit={vi.fn()}
      showScratchpad={false}
      scratchpads={[]}
      edit={null}
      {...overrides}
    />,
  );
}

describe("TodoItem", () => {
  it("renders an expanded body through the Markdown renderer instead of printing its source", () => {
    row({
      todo: todo({
        doc: { title: "Ship the release", body: "## Acceptance\n\n- one\n- two", status: "open" },
      }),
    });

    const body = screen.getByTestId("rich-text");
    expect(body.textContent).toContain("## Acceptance");
    // The raw text reaches the renderer, not a paragraph of its own: nothing in the row prints the
    // Markdown source itself, which is what left `##` and `-` on screen before.
    expect(document.querySelector(".whitespace-pre-wrap")).toBeNull();
  });

  it("renders the body read-only and without editing chrome", () => {
    row({
      todo: todo({ doc: { title: "Ship the release", body: "Some detail", status: "open" } }),
    });

    const body = screen.getByTestId("rich-text");
    expect(body.dataset.editable).toBe("false");
    expect(body.dataset.toolbar).toBe("false");
  });

  it("renders no body region at all when the todo has none", () => {
    row();

    expect(screen.queryByTestId("rich-text")).toBeNull();
  });

  it("renders the row's tags", () => {
    row({ todo: todo({ tags: ["a", "b"] }) });

    expect(screen.getByText("a")).toBeTruthy();
    expect(screen.getByText("b")).toBeTruthy();
  });

  it("clips the trigger's meta so it cannot spill under the sibling agent control", () => {
    row({ todo: todo({ blocked_by: [2], blockers: [2], locked_by: 9 }), onOpenAgent: vi.fn() });

    const trigger = document.querySelector("[data-todo-trigger]") as HTMLElement;
    expect(trigger.className).toContain("overflow-hidden");

    const status = document.querySelector("[data-todo-status]") as HTMLElement;
    expect(status.querySelector("svg")?.getAttribute("class")).toContain("shrink-0");
    expect(status.querySelector("span")?.className).toContain("truncate");

    const blockers = document.querySelector("[data-todo-blockers]") as HTMLElement;
    expect(blockers.className).toContain("min-w-0");
    expect(blockers.className).toContain("truncate");
  });

  it("renders its id, its status label, and its scratchpad", () => {
    row({ todo: todo({ scratchpad: plan }), showScratchpad: true });

    expect(screen.getByText("#1")).toBeTruthy();
    expect(screen.getByText("Open")).toBeTruthy();
    expect(screen.getByText("Release plan")).toBeTruthy();
  });

  it("names a blocked row's unmet-blocker count with the right plurality", () => {
    row({ todo: todo({ blockers: [2, 3], blocked_by: [2, 3], blocked: true }) });

    expect(screen.getByText("2 unmet blockers")).toBeTruthy();
  });

  it("renders no blocker text once nothing is unmet", () => {
    row({ todo: todo({ blockers: [2], blocked_by: [], blocked: false }) });

    expect(screen.queryByText(/unmet blocker/)).toBeNull();
  });

  it("keeps the disclosure trigger free of any interactive descendant", () => {
    row({ todo: todo({ locked_by: 9 }), onOpenAgent: vi.fn() });

    const trigger = document.querySelector("[data-todo-trigger]") as HTMLElement;
    expect(trigger.querySelector('button, [role="button"], a[href]')).toBeNull();
  });

  it("activates the agent control without toggling the row", () => {
    const onOpenAgent = vi.fn();
    const onToggle = vi.fn();
    row({ todo: todo({ locked_by: 9 }), onOpenAgent, onToggle });

    fireEvent.click(document.querySelector("[data-todo-agent]") as HTMLElement);

    expect(onOpenAgent).toHaveBeenCalledWith(9);
    expect(onToggle).not.toHaveBeenCalled();
  });

  it("is keyboard-reachable as its own tab stop, separate from the trigger", () => {
    row({ todo: todo({ locked_by: 9 }), onOpenAgent: vi.fn() });

    const trigger = document.querySelector("[data-todo-trigger]") as HTMLElement;
    const agent = document.querySelector("[data-todo-agent]") as HTMLElement;
    expect(agent.tagName).toBe("BUTTON");
    expect(agent.tabIndex).not.toBe(-1);
    expect(trigger.contains(agent)).toBe(false);
  });

  it("renders the agent control disabled when no onOpenAgent is given", () => {
    row({ todo: todo({ locked_by: 9 }) });

    const agent = document.querySelector("[data-todo-agent]") as HTMLButtonElement;
    expect(agent.disabled).toBe(true);
  });

  it("renders no agent control on an unlocked row", () => {
    row({ todo: todo({ locked_by: null }), onOpenAgent: vi.fn() });

    expect(document.querySelector("[data-todo-agent]")).toBeNull();
  });
});
