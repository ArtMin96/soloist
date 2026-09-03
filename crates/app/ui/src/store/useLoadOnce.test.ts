// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useLoadOnce } from "@/store/useLoadOnce";

describe("useLoadOnce", () => {
  it("loads once across rerenders, and hands the result to the latest onLoaded", async () => {
    const load = vi.fn(() => Promise.resolve("value"));
    const onLoadedFirst = vi.fn();
    const onLoadedSecond = vi.fn();

    const { rerender } = renderHook(
      ({ onLoaded }: { onLoaded: (value: string) => void }) => useLoadOnce(load, onLoaded),
      { initialProps: { onLoaded: onLoadedFirst } },
    );
    // A fresh `load`/`onLoaded` closure each render must not re-trigger the load.
    rerender({ onLoaded: onLoadedSecond });
    rerender({ onLoaded: onLoadedSecond });

    await waitFor(() => expect(onLoadedSecond).toHaveBeenCalledWith("value"));
    expect(load).toHaveBeenCalledTimes(1);
    expect(onLoadedFirst).not.toHaveBeenCalled();
  });

  it("drops a result that resolves after unmount", async () => {
    let resolveLoad: (value: string) => void = () => {};
    const load = vi.fn(() => new Promise<string>((resolve) => (resolveLoad = resolve)));
    const onLoaded = vi.fn();

    const { unmount } = renderHook(() => useLoadOnce(load, onLoaded));
    unmount();

    await act(async () => {
      resolveLoad("value");
      await Promise.resolve();
    });

    expect(onLoaded).not.toHaveBeenCalled();
  });

  it("drops a rejection that lands after unmount instead of calling onError", async () => {
    let rejectLoad: (reason: unknown) => void = () => {};
    const load = vi.fn(() => new Promise<string>((_, reject) => (rejectLoad = reject)));
    const onLoaded = vi.fn();
    const onError = vi.fn();

    const { unmount } = renderHook(() => useLoadOnce(load, onLoaded, onError));
    unmount();

    await act(async () => {
      rejectLoad("failed");
      await Promise.resolve();
    });

    expect(onError).not.toHaveBeenCalled();
  });
});
