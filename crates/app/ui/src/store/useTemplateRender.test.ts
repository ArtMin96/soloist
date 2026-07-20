// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { templateRender } from "@/api";
import type { RenderedPrompt } from "@/domain";
import { useTemplateRender } from "@/store/useTemplateRender";

vi.mock("@/api", () => ({ templateRender: vi.fn() }));

const render = vi.mocked(templateRender);

const OPEN_PROJECT = 1;

// The template the preview is pointed at, unless a test says otherwise.
const target = {
  kind: "prompt" as const,
  scope: "global" as const,
  name: "review",
  project: OPEN_PROJECT,
  revision: 1,
};

function rendered(text: string, unfilled: string[] = []): RenderedPrompt {
  return { text, unfilled, unknown: [] };
}

describe("useTemplateRender", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    render.mockResolvedValue(rendered("review "));
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it("sends one render for a burst of typing, carrying the last value", async () => {
    const { result } = renderHook(() => useTemplateRender(target));
    await act(async () => void vi.advanceTimersByTime(200));
    render.mockClear();

    // Typing "abc" one character at a time, faster than the quiet window.
    act(() => result.current.setValue("file", "a"));
    act(() => result.current.setValue("file", "ab"));
    act(() => result.current.setValue("file", "abc"));
    expect(render).not.toHaveBeenCalled();

    await act(async () => void vi.advanceTimersByTime(200));

    expect(render).toHaveBeenCalledTimes(1);
    // The one render that was sent carries the settled value, not an intermediate keystroke.
    expect(result.current.values).toEqual({ file: "abc" });
  });

  it("renders again once the typing pauses a second time", async () => {
    const { result } = renderHook(() => useTemplateRender(target));
    await act(async () => void vi.advanceTimersByTime(200));
    render.mockClear();

    act(() => result.current.setValue("file", "a"));
    await act(async () => void vi.advanceTimersByTime(200));
    act(() => result.current.setValue("file", "ab"));
    await act(async () => void vi.advanceTimersByTime(200));

    expect(render).toHaveBeenCalledTimes(2);
  });

  it("clearing a field drops the value rather than answering with an empty one", async () => {
    const { result } = renderHook(() => useTemplateRender(target));
    await act(async () => void vi.advanceTimersByTime(200));

    act(() => result.current.setValue("file", "src/a.ts"));
    await act(async () => void vi.advanceTimersByTime(200));
    expect(result.current.values).toEqual({ file: "src/a.ts" });

    act(() => result.current.setValue("file", ""));
    await act(async () => void vi.advanceTimersByTime(200));

    // An absent key is what the core reads as unanswered, so the marker comes back. An empty
    // string would answer the placeholder with nothing and substitute it away.
    expect(result.current.values).toEqual({});
  });

  it("never renders a kind that has no preview", async () => {
    const { result } = renderHook(() =>
      useTemplateRender({ ...target, kind: "scratchpad" as const }),
    );
    await act(async () => void vi.advanceTimersByTime(200));

    expect(result.current.renderable).toBe(false);
    expect(render).not.toHaveBeenCalled();
  });

  it("surfaces a refused render and clears it on the next success", async () => {
    render.mockRejectedValueOnce("the rendered prompt would be 300000 bytes, over the cap");
    const { result } = renderHook(() => useTemplateRender(target));
    await act(async () => void vi.advanceTimersByTime(200));

    expect(result.current.error).toContain("over the cap");
    expect(result.current.rendered).toBeNull();

    render.mockResolvedValue(rendered("review src/a.ts"));
    act(() => result.current.setValue("file", "src/a.ts"));
    await act(async () => void vi.advanceTimersByTime(200));

    expect(result.current.error).toBeNull();
    expect(result.current.rendered?.text).toBe("review src/a.ts");
  });
});
