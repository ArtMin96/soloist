// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { MarkdownView } from "@/components/editor/MarkdownView";

// The renderer is a lazy TipTap surface that needs real layout, so it is stood in for here. The stub
// hands the test the readiness callback the view actually waits on, which is the whole point of
// these cases: what is on screen before the prose has been seeded, and what replaces it after.
let reportReady: (() => void) | null = null;

vi.mock("@/components/editor/LazyRichTextEditor", () => ({
  LazyRichTextEditor: (props: { initialMarkdown: string; onReady?: () => void }) => {
    reportReady = props.onReady ?? null;
    return <div data-testid="rich-text">{props.initialMarkdown}</div>;
  },
}));

afterEach(() => {
  reportReady = null;
  cleanup();
});

/** How many prose lines the stand-in draws for a body, read off the rendered DOM. */
function standInLines(markdown: string): number {
  const { container } = render(<MarkdownView markdown={markdown} ariaLabel="body" />);
  const lines = container.querySelectorAll('[data-slot="skeleton"]').length;
  cleanup();
  return lines;
}

describe("MarkdownView", () => {
  it("holds a stand-in and reports the region busy until the prose is seeded", () => {
    render(<MarkdownView markdown="Ship the release." ariaLabel="Ship the release body" />);

    const standIn = screen.getByRole("status");
    expect(standIn.getAttribute("aria-busy")).toBe("true");
    expect(screen.getByText("Loading Ship the release body")).not.toBeNull();
    expect(screen.getByTestId("rich-text").closest(".invisible")).not.toBeNull();

    act(() => reportReady?.());

    expect(screen.queryByRole("status")).toBeNull();
    expect(document.querySelector('[aria-busy="true"]')).toBeNull();
    expect(screen.getByTestId("rich-text").closest(".invisible")).toBeNull();
  });

  it("stays silent while a body whose surrounding structure already reads waits", () => {
    render(<MarkdownView markdown="Looks good." ariaLabel="Comment from Ada" announce={false} />);

    expect(document.querySelector('[aria-busy="true"]')).not.toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByText(/^Loading/)).toBeNull();
  });

  it("draws more stand-in lines for a longer body, and stops at a cap", () => {
    const paragraph = "steady words ".repeat(20);
    const chapter = "steady words ".repeat(200);
    const tome = "steady words ".repeat(2000);

    expect(standInLines("Done.")).toBeGreaterThan(0);
    expect(standInLines(paragraph)).toBeGreaterThan(standInLines("Done."));
    expect(standInLines(chapter)).toBeGreaterThan(standInLines(paragraph));
    expect(standInLines(tome)).toBe(standInLines(chapter));
  });
});
