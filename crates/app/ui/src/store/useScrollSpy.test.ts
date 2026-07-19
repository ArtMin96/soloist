// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  NO_ACTIVE_TARGET,
  activeTargetIndex,
  prefersReducedMotion,
  useScrollSpy,
  type ScrollSpyBand,
  type ScrollSpyGeometry,
} from "@/store/useScrollSpy";

// The band the outline rail uses: the reading line is the container's top edge and the lower 70% of
// the view is excluded.
const BAND: ScrollSpyBand = { topOffset: 0, bottomFraction: 0.7 };

// A 500px-tall viewport onto a 3000px document whose four headings sit 400px apart. Only `scrollTop`
// varies per case, so each test states exactly the one thing it is about.
function geometry(
  scrollTop: number,
  overrides: Partial<ScrollSpyGeometry> = {},
): ScrollSpyGeometry {
  return {
    offsets: [0, 400, 800, 1200],
    scrollTop,
    clientHeight: 500,
    scrollHeight: 3000,
    ...overrides,
  };
}

describe("activeTargetIndex", () => {
  it("reports no target when there is nothing to track", () => {
    expect(activeTargetIndex(geometry(600, { offsets: [] }), BAND)).toBe(NO_ACTIVE_TARGET);
  });

  it("picks the heading sitting inside the reading band", () => {
    // Band spans 350–500. The third heading (800) is far below it; the second (400) is inside.
    expect(activeTargetIndex(geometry(350), BAND)).toBe(1);
  });

  it("picks the top-most heading when two share the band", () => {
    // Band spans 350–500, and both of these headings fall in it — document order breaks the tie.
    expect(activeTargetIndex(geometry(350, { offsets: [0, 400, 450, 1200] }), BAND)).toBe(1);
  });

  it("falls back to the last heading above the band when reading between sections", () => {
    // Band spans 600–750; no heading is in it. The reader is inside the section opened at 400.
    expect(activeTargetIndex(geometry(600), BAND)).toBe(1);
  });

  it("holds the first heading when the band sits above every heading", () => {
    // Scrolled a little past the top, but the first heading starts far below the band — the fallback
    // has nothing above it to land on.
    expect(activeTargetIndex(geometry(50, { offsets: [900, 1400] }), BAND)).toBe(0);
  });

  it("forces the first heading at the top of the document", () => {
    // A header pushes the band to 120–150, past two headings that are already on screen. Left to the
    // band the rule would answer with the second; at rest at the top the reader is on the first.
    const withHeader: ScrollSpyBand = { topOffset: 120, bottomFraction: 0.7 };
    const nearTop = { offsets: [20, 60, 900], clientHeight: 500, scrollHeight: 3000 };
    expect(activeTargetIndex({ ...nearTop, scrollTop: 1 }, withHeader)).toBe(1);
    expect(activeTargetIndex({ ...nearTop, scrollTop: 0 }, withHeader)).toBe(0);
  });

  it("forces the last heading at the bottom of the document", () => {
    // The final heading (2900) never reaches a band that stops 350px into a 500px view, so without
    // the pin the rule would stay on the heading at 400 — as it does one viewport higher.
    expect(activeTargetIndex(geometry(2000, { offsets: [0, 400, 2900] }), BAND)).toBe(1);
    expect(activeTargetIndex(geometry(2500, { offsets: [0, 400, 2900] }), BAND)).toBe(2);
  });

  it("honours a band pushed down by a sticky header", () => {
    // Band 350–500 puts the heading at 400 on top; pushing the reading line down to 470 hands it to
    // the heading at 480 instead.
    const withHeader: ScrollSpyBand = { topOffset: 120, bottomFraction: 0.7 };
    const sections = {
      offsets: [0, 400, 480],
      scrollTop: 350,
      clientHeight: 500,
      scrollHeight: 3000,
    };
    expect(activeTargetIndex(sections, BAND)).toBe(1);
    expect(activeTargetIndex(sections, withHeader)).toBe(2);
  });
});

describe("prefersReducedMotion", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("reads the reduce-motion media query", () => {
    const matchMedia = vi.fn(() => ({ matches: true }));
    vi.stubGlobal("matchMedia", matchMedia);
    expect(prefersReducedMotion()).toBe(true);
    expect(matchMedia).toHaveBeenCalledWith("(prefers-reduced-motion: reduce)");
  });

  it("reports no preference when the query does not match", () => {
    vi.stubGlobal("matchMedia", () => ({ matches: false }));
    expect(prefersReducedMotion()).toBe(false);
  });
});

describe("useScrollSpy jumps", () => {
  afterEach(() => vi.unstubAllGlobals());

  function jumpTo(reducedMotion: boolean) {
    vi.stubGlobal("matchMedia", () => ({ matches: reducedMotion }));
    const target = document.createElement("h2");
    target.scrollIntoView = vi.fn();
    // No container: the hook's observers need layout jsdom does not have, and a jump does not
    // consult the container anyway.
    const { result } = renderHook(() => useScrollSpy(null, [target]));

    act(() => result.current.scrollToTarget(0));
    return { target, result };
  }

  it("scrolls smoothly and marks the target active", () => {
    const { target, result } = jumpTo(false);
    expect(target.scrollIntoView).toHaveBeenCalledWith({ behavior: "smooth", block: "start" });
    expect(result.current.activeIndex).toBe(0);
  });

  it("scrolls instantly when the user asked for reduced motion", () => {
    const { target } = jumpTo(true);
    expect(target.scrollIntoView).toHaveBeenCalledWith({ behavior: "auto", block: "start" });
  });

  it("ignores a jump to a target that is not there", () => {
    vi.stubGlobal("matchMedia", () => ({ matches: false }));
    const { result } = renderHook(() => useScrollSpy(null, []));
    act(() => result.current.scrollToTarget(3));
    expect(result.current.activeIndex).toBe(NO_ACTIVE_TARGET);
  });
});
