import { useCallback, useEffect, useRef, useState } from "react";

/** Reported when no target is active — a document with nothing to track. */
export const NO_ACTIVE_TARGET = -1;

/** Where a scroll container sits and where its targets are, in the container's content coordinates. */
export interface ScrollSpyGeometry {
  /** Each target's distance from the top of the scrolled content, in document order. */
  offsets: number[];
  /** How far the container is scrolled from its top. */
  scrollTop: number;
  /** The container's visible height. */
  clientHeight: number;
  /** The height of the container's full scrollable content. */
  scrollHeight: number;
}

/** The reading band — the slice of the container a target must fall in to count as being read. */
export interface ScrollSpyBand {
  /** Distance from the container's top edge down to the top of the band. */
  topOffset: number;
  /** Fraction of the container's height cut off the band's bottom, between 0 and 1. */
  bottomFraction: number;
}

export type ScrollSpyOptions = Partial<ScrollSpyBand>;

export interface ScrollSpy {
  /** The target being read, or `NO_ACTIVE_TARGET` when there is none. */
  activeIndex: number;
  /** Jumps to a target: marks it active at once, then scrolls it up into the band. */
  scrollToTarget: (index: number) => void;
  /** Re-reads the cached target positions, for a caller that knows the content just reflowed. */
  remeasure: () => void;
}

// The band's default shape: the reading line sits at the container's top edge, and the lower 70% of
// the view is excluded so "active" means the section you have read into — not any section on screen.
const DEFAULT_TOP_OFFSET = 0;
const DEFAULT_BOTTOM_FRACTION = 0.7;

// Slack for the scrolled-to-the-bottom test: fractional layout heights rarely add up exactly.
const SCROLL_END_EPSILON = 1;

// How long a jump owns the highlight when no `scrollend` arrives to hand it back sooner.
const JUMP_SETTLE_MS = 700;

const REDUCED_MOTION_QUERY = "(prefers-reduced-motion: reduce)";

/**
 * Which target is being read, given where the container is scrolled and where its targets sit.
 *
 * Through the body of a document the band decides: the top-most target inside it wins, and when the
 * band falls in the middle of a long section, the last target above it does. Both ends are pinned
 * instead, because there the band cannot answer — the first target sits above it at scroll-top, and
 * the last may never reach it at scroll-bottom.
 */
export function activeTargetIndex(geometry: ScrollSpyGeometry, band: ScrollSpyBand): number {
  const { offsets, scrollTop, clientHeight, scrollHeight } = geometry;
  if (offsets.length === 0) return NO_ACTIVE_TARGET;
  if (scrollTop <= 0) return 0;
  if (scrollTop + clientHeight >= scrollHeight - SCROLL_END_EPSILON) return offsets.length - 1;

  const bandTop = scrollTop + band.topOffset;
  const bandBottom = scrollTop + clientHeight * (1 - band.bottomFraction);
  const inBand = offsets.findIndex((offset) => offset >= bandTop && offset < bandBottom);
  return inBand >= 0 ? inBand : lastTargetAtOrAbove(offsets, bandTop);
}

// The last target at or above `limit`, by binary search over the ascending offsets; the first target
// when every one of them is below it.
function lastTargetAtOrAbove(offsets: number[], limit: number): number {
  let low = 0;
  let high = offsets.length - 1;
  let found = 0;
  while (low <= high) {
    const mid = (low + high) >> 1;
    if (offsets[mid] <= limit) {
      found = mid;
      low = mid + 1;
    } else {
      high = mid - 1;
    }
  }
  return found;
}

/**
 * True when the user has asked the system to reduce motion. The CSS side of that preference is
 * handled globally in `index.css`, but a `scrollIntoView` behavior passed from script overrides the
 * computed `scroll-behavior`, so a scripted scroll has to read the preference itself.
 */
export function prefersReducedMotion(): boolean {
  return typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia(REDUCED_MOTION_QUERY).matches
    : false;
}

/**
 * Tracks which of `targets` the reader is on inside `container`, and jumps to one on demand.
 *
 * Target positions are measured once and cached, so a scroll costs a binary search rather than a
 * layout read per target. Pass a stable `targets` array — a new identity re-measures and re-observes.
 */
export function useScrollSpy(
  container: HTMLElement | null,
  targets: HTMLElement[],
  options: ScrollSpyOptions = {},
): ScrollSpy {
  const { topOffset = DEFAULT_TOP_OFFSET, bottomFraction = DEFAULT_BOTTOM_FRACTION } = options;
  const [activeIndex, setActiveIndex] = useState(NO_ACTIVE_TARGET);
  const offsetsRef = useRef<number[]>([]);
  const frameRef = useRef(0);
  // A jump owns the highlight until its scroll settles; tracking mid-glide would walk the highlight
  // through every target the scroll passes over.
  const settleAtRef = useRef(0);

  const recompute = useCallback(() => {
    if (!container || Date.now() < settleAtRef.current) return;
    setActiveIndex(
      activeTargetIndex(
        {
          offsets: offsetsRef.current,
          scrollTop: container.scrollTop,
          clientHeight: container.clientHeight,
          scrollHeight: container.scrollHeight,
        },
        { topOffset, bottomFraction },
      ),
    );
  }, [container, topOffset, bottomFraction]);

  // Coalesce every wakeup to one recompute per frame: a scroll fires far faster than the highlight
  // can meaningfully change.
  const schedule = useCallback(() => {
    if (frameRef.current) return;
    frameRef.current = requestAnimationFrame(() => {
      frameRef.current = 0;
      recompute();
    });
  }, [recompute]);

  const remeasure = useCallback(() => {
    if (container) {
      const contentTop = container.getBoundingClientRect().top - container.scrollTop;
      offsetsRef.current = targets.map((el) => el.getBoundingClientRect().top - contentTop);
    }
    schedule();
  }, [container, targets, schedule]);

  useEffect(() => {
    if (!container) return;
    remeasure();

    // The observer wakes the rule exactly when a target crosses the band, so a document nobody is
    // scrolling costs nothing. The scroll listener covers what a band crossing cannot see: reaching
    // either end of the document, where the rule pins the first or last target instead.
    const crossings = new IntersectionObserver(schedule, {
      root: container,
      rootMargin: `${-topOffset}px 0px -${bottomFraction * 100}% 0px`,
    });
    for (const target of targets) crossings.observe(target);

    const resize = new ResizeObserver(remeasure);
    resize.observe(container);

    const settle = () => {
      settleAtRef.current = 0;
      schedule();
    };
    container.addEventListener("scroll", schedule, { passive: true });
    container.addEventListener("scrollend", settle);

    return () => {
      if (frameRef.current) cancelAnimationFrame(frameRef.current);
      frameRef.current = 0;
      crossings.disconnect();
      resize.disconnect();
      container.removeEventListener("scroll", schedule);
      container.removeEventListener("scrollend", settle);
    };
  }, [container, targets, remeasure, schedule, topOffset, bottomFraction]);

  const scrollToTarget = useCallback(
    (index: number) => {
      const target = targets[index];
      if (!target) return;
      setActiveIndex(index);
      settleAtRef.current = Date.now() + JUMP_SETTLE_MS;
      target.scrollIntoView({
        behavior: prefersReducedMotion() ? "auto" : "smooth",
        block: "start",
      });
    },
    [targets],
  );

  return { activeIndex, scrollToTarget, remeasure };
}
