// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAutosave } from "./useAutosave";

describe("useAutosave", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("saves once after the debounce, carrying the latest value", () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useAutosave({ onSave, delayMs: 800 }));

    act(() => result.current.push("a"));
    act(() => result.current.push("ab"));
    expect(onSave).not.toHaveBeenCalled();
    expect(result.current.dirty).toBe(true);

    act(() => vi.advanceTimersByTime(800));
    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onSave).toHaveBeenCalledWith("ab");
  });

  it("flushes immediately and cancels the pending debounce", () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useAutosave({ onSave }));

    act(() => result.current.push("x"));
    act(() => result.current.flush());
    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onSave).toHaveBeenCalledWith("x");

    // The cancelled timer must not fire a second, redundant save.
    act(() => vi.advanceTimersByTime(2000));
    expect(onSave).toHaveBeenCalledTimes(1);
  });

  it("does not save while paused, and flush is a no-op", () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useAutosave({ onSave, paused: true }));

    act(() => result.current.push("x"));
    expect(result.current.dirty).toBe(true); // still honestly dirty
    act(() => vi.advanceTimersByTime(2000));
    act(() => result.current.flush());
    expect(onSave).not.toHaveBeenCalled();
  });

  it("goes clean after a save, so a later flush does nothing", () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useAutosave({ onSave }));

    act(() => result.current.push("x"));
    act(() => result.current.flush());
    act(() => result.current.flush());
    expect(onSave).toHaveBeenCalledTimes(1);
  });

  it("persists a pending edit when the editor unmounts", () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const { result, unmount } = renderHook(() => useAutosave({ onSave }));

    act(() => result.current.push("z"));
    act(() => unmount());
    expect(onSave).toHaveBeenCalledWith("z");
  });
});
