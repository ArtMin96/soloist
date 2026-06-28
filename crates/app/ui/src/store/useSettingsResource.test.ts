// @vitest-environment jsdom
import { describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { useSettingsResource } from "@/store/useSettingsResource";

describe("useSettingsResource", () => {
  it("loads the stored document once, superseding the fallback", async () => {
    const load = vi.fn().mockResolvedValue({ n: 1 });
    const save = vi.fn();
    const { result } = renderHook(() => useSettingsResource(load, save, { n: 0 }));

    // Before the load resolves the fallback shows; after, the stored value supersedes it.
    expect(result.current.value).toEqual({ n: 0 });
    await waitFor(() => expect(result.current.value).toEqual({ n: 1 }));
    expect(load).toHaveBeenCalledTimes(1);
  });

  it("applies an update optimistically, then reconciles with the saved echo", async () => {
    const load = vi.fn().mockResolvedValue({ n: 1 });
    let resolveSave: (value: { n: number }) => void = () => {};
    const save = vi.fn().mockImplementation(
      () =>
        new Promise<{ n: number }>((resolve) => {
          resolveSave = resolve;
        }),
    );
    const { result } = renderHook(() => useSettingsResource(load, save, { n: 0 }));
    await waitFor(() => expect(result.current.value).toEqual({ n: 1 }));

    // The local value changes immediately, before the save resolves.
    act(() => result.current.update({ n: 2 }));
    expect(result.current.value).toEqual({ n: 2 });
    expect(save).toHaveBeenCalledWith({ n: 2 });

    // The facade can normalize on save; the hook reconciles to whatever it echoes back.
    await act(async () => {
      resolveSave({ n: 99 });
    });
    await waitFor(() => expect(result.current.value).toEqual({ n: 99 }));
  });
});
