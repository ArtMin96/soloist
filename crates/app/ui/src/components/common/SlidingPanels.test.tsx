// @vitest-environment jsdom
import { useEffect } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DETAIL_BACK_ATTRIBUTE } from "@/components/common/DetailPane";
import { SlidingPanels, type SlidingPanel } from "@/components/common/SlidingPanels";
import { MarkdownView } from "@/components/editor/MarkdownView";
import { createNavigationLedger, useMasterDetail } from "@/store/useMasterDetail";

// The document renderer needs real layout, so the surface a body would build is stood in for. It
// reports itself seeded the way the real one does, since when that happens is what decides whether
// the reader is looking at prose or at a stand-in. What matters here is when it is built, not what
// it draws.
vi.mock("@/components/editor/LazyRichTextEditor", () => ({
  LazyRichTextEditor: ({
    initialMarkdown,
    onReady,
  }: {
    initialMarkdown: string;
    onReady?: () => void;
  }) => {
    useEffect(() => onReady?.(), [onReady]);
    return <div data-testid="rich-text">{initialMarkdown}</div>;
  },
}));

afterEach(cleanup);

function renderPanels(showing: SlidingPanel, onSettled = () => {}) {
  return render(
    <SlidingPanels
      showing={showing}
      list={<button type="button">In list</button>}
      detail={<button type="button">In detail</button>}
      onSettled={onSettled}
    />,
  );
}

const track = (container: HTMLElement) => container.querySelector("[data-panel-route] > div")!;
const panel = (container: HTMLElement, name: SlidingPanel) =>
  container.querySelector<HTMLElement>(`[data-panel="${name}"]`)!;

describe("SlidingPanels", () => {
  it("holds the off-screen panel inert, and flips it with the route", () => {
    // jsdom implements the `inert` attribute but none of its behaviour, so this checks the
    // attribute reaches the DOM. That it actually removes the panel from the tab order and the
    // accessibility tree is only observable in a real window.
    const { container, rerender } = renderPanels("list");
    expect(panel(container, "list").hasAttribute("inert")).toBe(false);
    expect(panel(container, "detail").hasAttribute("inert")).toBe(true);

    rerender(<SlidingPanels showing="detail" list={null} detail={null} onSettled={() => {}} />);
    expect(panel(container, "list").hasAttribute("inert")).toBe(true);
    expect(panel(container, "detail").hasAttribute("inert")).toBe(false);
  });

  // The property names below are the ones a real window emits. Measured on the track in WebKitGTK:
  // `["start translate self=true", "start box-shadow self=false", …]`, with computed
  // `transitionProperty: "transform, translate, scale, rotate"` and computed `transform: "none"`.
  // Tailwind compiles `-translate-x-full` to `translate`, so `translate` is what actually arrives —
  // a test that fires `transform` describes a code path the browser can never take.
  it("settles on the translate the browser actually emits for the track", () => {
    const onSettled = vi.fn();
    const { container } = renderPanels("detail", onSettled);
    fireEvent.transitionEnd(track(container), { propertyName: "translate" });
    expect(onSettled).toHaveBeenCalledTimes(1);
  });

  it("settles on transform too, for a build that compiles the class that way", () => {
    const onSettled = vi.fn();
    const { container } = renderPanels("detail", onSettled);
    fireEvent.transitionEnd(track(container), { propertyName: "transform" });
    expect(onSettled).toHaveBeenCalledTimes(1);
  });

  it("settles once through the fallback when no transition event arrives", () => {
    const frames: FrameRequestCallback[] = [];
    let timeout: (() => void) | undefined;
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      frames.push(callback);
      return frames.length;
    });
    vi.stubGlobal("cancelAnimationFrame", vi.fn());
    const timeoutSpy = vi.spyOn(window, "setTimeout").mockImplementation((callback) => {
      timeout = callback;
      return 1 as unknown as ReturnType<typeof setTimeout>;
    });

    try {
      const onSettled = vi.fn();
      const { container, rerender } = renderPanels("list", onSettled);
      rerender(<SlidingPanels showing="detail" list={null} detail={null} onSettled={onSettled} />);

      act(() => frames.shift()?.(0));
      act(() => frames.shift()?.(0));
      act(() => timeout?.());

      expect(onSettled).toHaveBeenCalledTimes(1);

      fireEvent.transitionEnd(track(container), { propertyName: "translate" });
      expect(onSettled).toHaveBeenCalledTimes(1);
    } finally {
      timeoutSpy.mockRestore();
      vi.unstubAllGlobals();
    }
  });

  it("settles once per route change when the browser emits multiple movement properties", () => {
    const onSettled = vi.fn();
    const { container, rerender } = renderPanels("list", onSettled);

    rerender(<SlidingPanels showing="detail" list={null} detail={null} onSettled={onSettled} />);
    fireEvent.transitionEnd(track(container), { propertyName: "translate" });
    fireEvent.transitionEnd(track(container), { propertyName: "transform" });
    expect(onSettled).toHaveBeenCalledTimes(1);

    rerender(<SlidingPanels showing="list" list={null} detail={null} onSettled={onSettled} />);
    fireEvent.transitionEnd(track(container), { propertyName: "translate" });
    expect(onSettled).toHaveBeenCalledTimes(2);
  });

  it("ignores a transition that bubbled up from inside a panel", () => {
    const onSettled = vi.fn();
    const { container } = renderPanels("detail", onSettled);
    // Content inside a panel animates too; its movement must not be read as the track arriving.
    fireEvent.transitionEnd(panel(container, "detail"), { propertyName: "translate" });
    expect(onSettled).not.toHaveBeenCalled();
  });

  it("ignores a property that is not the movement, on the track itself", () => {
    const onSettled = vi.fn();
    const { container } = renderPanels("detail", onSettled);
    // `box-shadow` is in the measured stream rather than an invented name.
    fireEvent.transitionEnd(track(container), { propertyName: "box-shadow" });
    expect(onSettled).not.toHaveBeenCalled();
  });

  it("names the route it is showing", () => {
    const { container } = renderPanels("list");
    expect(container.querySelector("[data-panel-route]")!.getAttribute("data-panel-route")).toBe(
      "list",
    );
  });

  it("keeps programmatic panel focus visible for keyboard users", () => {
    const { container } = renderPanels("list");
    const listPanel = panel(container, "list");

    listPanel.focus();

    expect(document.activeElement).toBe(listPanel);
    expect(listPanel.className).toContain("focus-visible:ring-2");
    expect(listPanel.className).toContain("focus-visible:ring-inset");
    expect(listPanel.className).toContain("focus-visible:ring-ring");
  });
});

const LEDGER = createNavigationLedger();
const BODIES: Record<number, string> = { 1: "The first note.", 2: "The second note." };

// A board of the shape every consumer builds: the route hook driving the panels, and a document
// body inside the detail panel. Wired from the real pieces, because the thing under test is what
// they do together on one open and one Back.
function Board({ onOpen }: { onOpen: (key: number) => void }) {
  const route = useMasterDetail<number>({
    project: 1,
    ledger: LEDGER,
    present: () => true,
    rowTrigger: (key) => `[data-row="${key}"]`,
    onOpen,
  });

  return (
    <SlidingPanels
      showing={route.showing}
      onSettled={route.onSettled}
      list={Object.keys(BODIES).map((key) => (
        <button key={key} data-row={key} type="button" onClick={() => route.open(Number(key))}>
          Note {key}
        </button>
      ))}
      detail={
        route.detailKey == null ? null : (
          <div>
            <button {...{ [DETAIL_BACK_ATTRIBUTE]: "" }} type="button" onClick={route.back}>
              Back
            </button>
            <MarkdownView
              key={route.detailKey}
              markdown={BODIES[route.detailKey]}
              ariaLabel="body"
            />
          </div>
        )
      }
    />
  );
}

describe("SlidingPanels driving a board", () => {
  it("stands in for the document while moving, and settles once in each direction", () => {
    const opened = vi.fn();
    const { container } = render(<Board onOpen={opened} />);

    fireEvent.click(screen.getByText("Note 1"));

    // The panel is on its way in: the reader has the stand-in, and the frames belong to the slide.
    expect(screen.getByRole("status").getAttribute("aria-busy")).toBe("true");
    expect(screen.queryByTestId("rich-text")).toBeNull();

    fireEvent.transitionEnd(track(container), { propertyName: "translate" });
    expect(screen.getByTestId("rich-text").textContent).toBe(BODIES[1]);

    fireEvent.click(screen.getByText("Back"));

    // Still carrying its subject: a panel that blanks on the way out reads as a bug, not a slide.
    expect(screen.getByTestId("rich-text").textContent).toBe(BODIES[1]);
    expect(opened).toHaveBeenCalledTimes(1);

    fireEvent.transitionEnd(track(container), { propertyName: "translate" });
    expect(screen.queryByTestId("rich-text")).toBeNull();

    fireEvent.click(screen.getByText("Note 2"));

    // Every open, not just the first: the stand-in is what the reader sees while the next document
    // is built, and nothing was re-read to put it there.
    expect(screen.getByRole("status").getAttribute("aria-busy")).toBe("true");
    expect(screen.queryByTestId("rich-text")).toBeNull();
    expect(opened.mock.calls).toEqual([[1], [2]]);
  });
});
