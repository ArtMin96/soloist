// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MERMAID_RENDER_DEBOUNCE_MS } from "./const";
import { useDebouncedPreview } from "./useDebouncedPreview";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

const THEMED = "---\nconfig:\n  theme: dark\n---\nflowchart TD";

/**
 * The hook driven the way the panel drives it: the source is owned above, so an edit travels up
 * through the change handler and comes back down as a new `source`.
 */
function panel(initial: string) {
  let source = initial;
  const { result, rerender } = renderHook(() =>
    useDebouncedPreview(source, (next) => {
      source = next;
    }),
  );
  return {
    preview: () => result.current[0],
    type: (next: string) =>
      act(() => {
        result.current[1](next);
        rerender();
      }),
    rewrite: (next: string) =>
      act(() => {
        source = next;
        rerender();
      }),
    settle: () => act(() => void vi.advanceTimersByTime(MERMAID_RENDER_DEBOUNCE_MS)),
  };
}

describe("useDebouncedPreview", () => {
  it("holds a keystroke back so a burst coalesces into one render", () => {
    const diagram = panel("flowchart TD");

    diagram.type("flowchart TD\n  A");

    expect(diagram.preview()).toBe("flowchart TD");

    diagram.settle();

    expect(diagram.preview()).toBe("flowchart TD\n  A");
  });

  it("previews a change the editor did not make at once", () => {
    const diagram = panel("flowchart TD");

    diagram.rewrite(THEMED);

    expect(diagram.preview()).toBe(THEMED);
  });

  it("previews a theme rewrite at once even while a keystroke is still waiting", () => {
    // The case a recency rule gets wrong: picking a theme moments after typing is still not typing,
    // and treating it as such reimposes the whole wait on the discrete change the wait is not for.
    const diagram = panel("flowchart TD");
    diagram.type("flowchart TD\n  A");

    diagram.rewrite(THEMED);

    expect(diagram.preview()).toBe(THEMED);
  });
});
