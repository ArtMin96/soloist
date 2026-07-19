// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { TodoItem } from "@/components/orchestration/TodoItem";
import type { TodoView } from "@/domain";

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

const todo = (body: string): TodoView => ({
  id: 1,
  doc: { title: "Ship the release", body, status: "open" },
  tags: [],
  blockers: [],
  blocked_by: [],
  blocked: false,
  comments: [],
  locked_by: null,
  scratchpad: null,
  revision: 1,
});

function row(body: string) {
  return render(
    <TodoItem
      todo={todo(body)}
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
    />,
  );
}

describe("TodoItem", () => {
  it("renders an expanded body through the Markdown renderer instead of printing its source", () => {
    row("## Acceptance\n\n- one\n- two");

    const body = screen.getByTestId("rich-text");
    expect(body.textContent).toContain("## Acceptance");
    // The raw text reaches the renderer, not a paragraph of its own: nothing in the row prints the
    // Markdown source itself, which is what left `##` and `-` on screen before.
    expect(document.querySelector(".whitespace-pre-wrap")).toBeNull();
  });

  it("renders the body read-only and without editing chrome", () => {
    row("Some detail");

    const body = screen.getByTestId("rich-text");
    expect(body.dataset.editable).toBe("false");
    expect(body.dataset.toolbar).toBe("false");
  });

  it("renders no body region at all when the todo has none", () => {
    row("");

    expect(screen.queryByTestId("rich-text")).toBeNull();
  });
});
