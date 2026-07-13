// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

vi.mock("@/api", () => ({
  onResync: vi.fn(() => Promise.resolve(() => {})),
}));

import { onResync } from "@/api";
import { useReconcile } from "@/store/useReconcile";

const subscribe = vi.mocked(onResync);

afterEach(() => vi.clearAllMocks());

describe("useReconcile", () => {
  it("refreshes when the backend signals a resync", async () => {
    const refresh = vi.fn();
    renderHook(() => useReconcile(refresh));
    await waitFor(() => expect(subscribe).toHaveBeenCalled());
    const handler = subscribe.mock.calls[0][0];
    act(() => handler());
    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it("refreshes when the window regains focus", async () => {
    const refresh = vi.fn();
    renderHook(() => useReconcile(refresh));
    await waitFor(() => expect(subscribe).toHaveBeenCalled());
    act(() => window.dispatchEvent(new Event("focus")));
    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it("stops refreshing on focus after unmount", async () => {
    const refresh = vi.fn();
    const { unmount } = renderHook(() => useReconcile(refresh));
    await waitFor(() => expect(subscribe).toHaveBeenCalled());
    unmount();
    act(() => window.dispatchEvent(new Event("focus")));
    expect(refresh).not.toHaveBeenCalled();
  });
});
