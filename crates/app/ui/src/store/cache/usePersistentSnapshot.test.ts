// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

// The persisted cache is the disk boundary; mock it so the hook's stale-while-revalidate logic
// is driven from controlled cache reads and the write-through is observable.
vi.mock("@/store/cache/persistentCache", () => ({
  CacheKey: { projects: "projects", appInfo: "app-info", agents: "agents" },
  readSnapshot: vi.fn(),
  writeSnapshot: vi.fn(() => Promise.resolve()),
}));

import { CacheKey, readSnapshot, writeSnapshot } from "@/store/cache/persistentCache";
import { usePersistentSnapshot } from "@/store/cache/usePersistentSnapshot";

const read = vi.mocked(readSnapshot);
const write = vi.mocked(writeSnapshot);

afterEach(() => vi.clearAllMocks());

describe("usePersistentSnapshot", () => {
  it("paints the cached value first, then reconciles to the backend (stale-then-fresh)", async () => {
    read.mockResolvedValue(["cached"]);
    let resolveFetch: (value: string[]) => void = () => {};
    const fetcher = vi.fn(() => new Promise<string[]>((resolve) => (resolveFetch = resolve)));

    const { result } = renderHook(() => usePersistentSnapshot(CacheKey.projects, fetcher));

    // The stale cached value paints before the fetch resolves.
    await waitFor(() => expect(result.current.value).toEqual(["cached"]));

    // The backend value then replaces it and is written through.
    act(() => resolveFetch(["fresh"]));
    await waitFor(() => expect(result.current.value).toEqual(["fresh"]));
    expect(write).toHaveBeenCalledWith(CacheKey.projects, ["fresh"]);
  });

  it("populates from the backend alone on a cache miss", async () => {
    read.mockResolvedValue(null);
    const fetcher = vi.fn(() => Promise.resolve(["fresh"]));
    const { result } = renderHook(() => usePersistentSnapshot(CacheKey.projects, fetcher));
    await waitFor(() => expect(result.current.value).toEqual(["fresh"]));
  });

  it("lets the backend value win over a stale cache (backend authoritative)", async () => {
    read.mockResolvedValue(["stale"]);
    const fetcher = vi.fn(() => Promise.resolve(["authoritative"]));
    const { result } = renderHook(() => usePersistentSnapshot(CacheKey.projects, fetcher));
    await waitFor(() => expect(result.current.value).toEqual(["authoritative"]));
  });

  it("keeps the stale value and reports the error when revalidation fails", async () => {
    read.mockResolvedValue(["stale"]);
    const fetcher = vi.fn(() => Promise.reject("backend down"));
    const onError = vi.fn();
    const { result } = renderHook(() =>
      usePersistentSnapshot(CacheKey.projects, fetcher, { onError }),
    );
    await waitFor(() => expect(onError).toHaveBeenCalledWith("backend down"));
    expect(result.current.value).toEqual(["stale"]);
    expect(write).not.toHaveBeenCalled();
  });

  it("only seeds from cache and defers the fetch when revalidateOnMount is false", async () => {
    read.mockResolvedValue(["cached"]);
    const fetcher = vi.fn(() => Promise.resolve(["fresh"]));
    const { result } = renderHook(() =>
      usePersistentSnapshot(CacheKey.agents, fetcher, { revalidateOnMount: false }),
    );

    await waitFor(() => expect(result.current.value).toEqual(["cached"]));
    expect(fetcher).not.toHaveBeenCalled();

    act(() => result.current.revalidate());
    await waitFor(() => expect(result.current.value).toEqual(["fresh"]));
    expect(fetcher).toHaveBeenCalledTimes(1);
  });

  it("shows a fetcher's partial on a cold cache, then the authoritative value", async () => {
    read.mockResolvedValue(null);
    let resolveDetect: (value: string[]) => void = () => {};
    const fetcher = vi.fn((emit: (partial: string[]) => void) => {
      emit(["partial"]);
      return new Promise<string[]>((resolve) => (resolveDetect = resolve));
    });

    const { result } = renderHook(() => usePersistentSnapshot(CacheKey.agents, fetcher));

    await waitFor(() => expect(result.current.value).toEqual(["partial"]));
    act(() => resolveDetect(["detected"]));
    await waitFor(() => expect(result.current.value).toEqual(["detected"]));
  });

  it("does not downgrade a cached value to a fetcher's partial", async () => {
    read.mockResolvedValue(["cached"]);
    let resolveDetect: (value: string[]) => void = () => {};
    const fetcher = vi.fn((emit: (partial: string[]) => void) => {
      emit(["partial"]);
      return new Promise<string[]>((resolve) => (resolveDetect = resolve));
    });

    const { result } = renderHook(() => usePersistentSnapshot(CacheKey.agents, fetcher));

    // The cached value holds; the partial does not replace it.
    await waitFor(() => expect(result.current.value).toEqual(["cached"]));
    expect(result.current.value).toEqual(["cached"]);

    act(() => resolveDetect(["detected"]));
    await waitFor(() => expect(result.current.value).toEqual(["detected"]));
  });
});
