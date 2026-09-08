// @vitest-environment jsdom
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SlidingPanels, type SlidingPanel } from "@/components/common/SlidingPanels";
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

  it("mounts its prose straight away when no panel is carrying it", () => {
    // A body in a comment thread or a template preview has no movement to wait for. It reads the
    // published default rather than a signal, so nothing can leave it waiting for one forever.
    render(<MarkdownView markdown="Looks good." ariaLabel="body" />);

    expect(screen.getByTestId("rich-text")).not.toBeNull();
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

// The body as a detail panel actually carries it: absent while the list is showing, then arriving
// with the route, which is the moment the panel starts moving.
function slideIn(markdown: string) {
  const panels = (showing: SlidingPanel, detail: ReactNode) => (
    <SlidingPanels showing={showing} list={null} detail={detail} onSettled={() => {}} />
  );
  const { container, rerender } = render(panels("list", null));
  rerender(panels("detail", <MarkdownView markdown={markdown} ariaLabel="body" />));
  return { track: container.querySelector("[data-panel-route] > div")! };
}

describe("MarkdownView in a panel that is moving", () => {
  it("holds the stand-in for the movement and builds the prose once the panel arrives", () => {
    const { track } = slideIn("Ship the release.");

    // Starting a document up costs hundreds of milliseconds of main thread; spending them here
    // would eat the movement's frames and the stand-in would never be painted.
    expect(screen.getByRole("status").getAttribute("aria-busy")).toBe("true");
    expect(screen.queryByTestId("rich-text")).toBeNull();

    fireEvent.transitionEnd(track, { propertyName: "translate" });

    expect(screen.getByTestId("rich-text")).not.toBeNull();
  });

  it("builds the prose anyway when the movement never reports finishing", async () => {
    // Reduced motion, an interrupted transition, a detail that arrives without a slide: the panel
    // is on screen and the reader is waiting, so the wait is bounded rather than open-ended.
    slideIn("Ship the release.");
    expect(screen.queryByTestId("rich-text")).toBeNull();

    expect(await screen.findByTestId("rich-text")).not.toBeNull();
  });
});
