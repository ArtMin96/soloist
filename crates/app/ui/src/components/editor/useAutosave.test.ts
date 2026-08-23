// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAutosave } from "./useAutosave";
import type { SaveOutcome } from "@/store/saveOutcome";

/** A promise plus its resolver, so a test can hold a save open and settle it on its own schedule. */
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

/** Flushes pending microtasks (a resolved promise's `.then` chain) without advancing fake timers. */
async function flushMicrotasks() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("useAutosave", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("saves once after the debounce, carrying the latest value", () => {
    const onSave = vi.fn<(value: string) => Promise<SaveOutcome>>().mockResolvedValue("saved");
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
    const onSave = vi.fn<(value: string) => Promise<SaveOutcome>>().mockResolvedValue("saved");
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
    const onSave = vi.fn<(value: string) => Promise<SaveOutcome>>().mockResolvedValue("saved");
    const { result } = renderHook(() => useAutosave({ onSave, paused: true }));

    act(() => result.current.push("x"));
    expect(result.current.dirty).toBe(true); // still honestly dirty
    act(() => vi.advanceTimersByTime(2000));
    act(() => result.current.flush());
    expect(onSave).not.toHaveBeenCalled();
  });

  it("goes clean after a save settles, so a later flush does nothing", async () => {
    const onSave = vi.fn<(value: string) => Promise<SaveOutcome>>().mockResolvedValue("saved");
    const { result } = renderHook(() => useAutosave({ onSave }));

    act(() => result.current.push("x"));
    act(() => result.current.flush());
    await flushMicrotasks();
    expect(result.current.dirty).toBe(false);

    act(() => result.current.flush());
    expect(onSave).toHaveBeenCalledTimes(1);
  });

  it("persists a pending edit when the editor unmounts", () => {
    const onSave = vi.fn<(value: string) => Promise<SaveOutcome>>().mockResolvedValue("saved");
    const { result, unmount } = renderHook(() => useAutosave({ onSave }));

    act(() => result.current.push("z"));
    act(() => unmount());
    expect(onSave).toHaveBeenCalledWith("z");
  });

  it("restores a refused save as dirty, and a later flush re-sends it", async () => {
    const first = deferred<SaveOutcome>();
    const onSave = vi
      .fn<(value: string) => Promise<SaveOutcome>>()
      .mockReturnValueOnce(first.promise);
    const { result } = renderHook(() => useAutosave({ onSave }));

    act(() => result.current.push("a"));
    act(() => result.current.flush());
    expect(result.current.saving).toBe(true);

    onSave.mockResolvedValueOnce("saved");
    first.resolve("refused");
    await flushMicrotasks();

    expect(result.current.saving).toBe(false);
    expect(result.current.dirty).toBe(true); // honestly unsaved, never falsely "Saved"

    act(() => result.current.flush());
    expect(onSave).toHaveBeenCalledTimes(2);
    expect(onSave).toHaveBeenNthCalledWith(2, "a");
  });

  it("never fires a second save while one is in flight, only after it settles", async () => {
    const first = deferred<SaveOutcome>();
    const onSave = vi
      .fn<(value: string) => Promise<SaveOutcome>>()
      .mockReturnValueOnce(first.promise);
    const { result } = renderHook(() => useAutosave({ onSave }));

    act(() => result.current.push("a"));
    act(() => vi.advanceTimersByTime(800));
    expect(onSave).toHaveBeenCalledTimes(1);

    onSave.mockResolvedValueOnce("saved");
    act(() => result.current.push("b"));
    // A push mid-flight must not start a concurrent save.
    expect(onSave).toHaveBeenCalledTimes(1);
    act(() => vi.advanceTimersByTime(5000));
    expect(onSave).toHaveBeenCalledTimes(1);

    first.resolve("saved");
    await flushMicrotasks();

    expect(onSave).toHaveBeenCalledTimes(2);
    expect(onSave).toHaveBeenNthCalledWith(2, "b");
  });

  it("drains only the latest queued edit, not an intermediate one", async () => {
    const first = deferred<SaveOutcome>();
    const onSave = vi
      .fn<(value: string) => Promise<SaveOutcome>>()
      .mockReturnValueOnce(first.promise);
    const { result } = renderHook(() => useAutosave({ onSave }));

    act(() => result.current.push("a"));
    act(() => vi.advanceTimersByTime(800));

    onSave.mockResolvedValueOnce("saved");
    act(() => result.current.push("b"));
    act(() => result.current.push("c"));

    first.resolve("saved");
    await flushMicrotasks();

    expect(onSave).toHaveBeenNthCalledWith(2, "c");
    expect(onSave).toHaveBeenCalledTimes(2);
  });

  it("keeps saving true until the last in-flight write settles", async () => {
    const first = deferred<SaveOutcome>();
    const second = deferred<SaveOutcome>();
    const onSave = vi
      .fn<(value: string) => Promise<SaveOutcome>>()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const { result } = renderHook(() => useAutosave({ onSave }));

    act(() => result.current.push("a"));
    act(() => vi.advanceTimersByTime(800));
    expect(result.current.saving).toBe(true);

    act(() => result.current.push("b"));
    first.resolve("saved");
    await flushMicrotasks();
    // The second write started; the hook has not gone idle.
    expect(result.current.saving).toBe(true);
    expect(onSave).toHaveBeenCalledTimes(2);

    second.resolve("saved");
    await flushMicrotasks();
    expect(result.current.saving).toBe(false);
  });

  it("suppresses the queued follow-up when paused turns on mid-flight", async () => {
    const first = deferred<SaveOutcome>();
    const onSave = vi
      .fn<(value: string) => Promise<SaveOutcome>>()
      .mockReturnValueOnce(first.promise);
    const { result, rerender } = renderHook(({ paused }) => useAutosave({ onSave, paused }), {
      initialProps: { paused: false },
    });

    act(() => result.current.push("a"));
    act(() => vi.advanceTimersByTime(800));
    act(() => result.current.push("b"));

    rerender({ paused: true });
    first.resolve("saved");
    await flushMicrotasks();

    expect(onSave).toHaveBeenCalledTimes(1); // "b" was never sent
    expect(result.current.saving).toBe(false);
    expect(result.current.dirty).toBe(true);
  });

  it("persists a queued edit that arrives during an in-flight save, even after unmount", async () => {
    const first = deferred<SaveOutcome>();
    const onSave = vi
      .fn<(value: string) => Promise<SaveOutcome>>()
      .mockReturnValueOnce(first.promise);
    const { result, unmount } = renderHook(() => useAutosave({ onSave }));

    act(() => result.current.push("a"));
    act(() => vi.advanceTimersByTime(800));

    onSave.mockResolvedValueOnce("saved");
    act(() => result.current.push("b"));
    act(() => unmount());
    // The queued edit must not go out until the in-flight write settles — not synchronously at
    // unmount, which would race it against the write already underway.
    expect(onSave).toHaveBeenCalledTimes(1);

    first.resolve("saved");
    await flushMicrotasks();

    expect(onSave).toHaveBeenCalledTimes(2);
    expect(onSave).toHaveBeenNthCalledWith(2, "b");
  });

  it("never retries a refusal on its own — only a further user action tries again", async () => {
    const onSave = vi.fn<(value: string) => Promise<SaveOutcome>>().mockResolvedValue("refused");
    const { result } = renderHook(() => useAutosave({ onSave }));

    act(() => result.current.push("a"));
    act(() => vi.advanceTimersByTime(800));
    await flushMicrotasks();
    expect(onSave).toHaveBeenCalledTimes(1);
    // Honestly unsaved — never quietly declared clean while the refused value sits unpersisted.
    expect(result.current.dirty).toBe(true);

    act(() => vi.advanceTimersByTime(60_000));
    await flushMicrotasks();
    expect(onSave).toHaveBeenCalledTimes(1);
  });
});
