// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { attentionSnapshot, clearAllAttention, onDomainEvent } from "@/api";
import { useAttention } from "@/store/useAttention";
import type { AttentionSnapshot, DomainEvent } from "@/domain";

vi.mock("@/api", () => ({
  attentionSnapshot: vi.fn(),
  clearAllAttention: vi.fn(),
  onDomainEvent: vi.fn(),
}));

const query = vi.mocked(attentionSnapshot);
const clear = vi.mocked(clearAllAttention);
const listen = vi.mocked(onDomainEvent);

/// The domain-event handler the hook registered, so a test can publish a real event through it.
let publish: ((event: DomainEvent) => void) | undefined;

function snapshot(...processes: AttentionSnapshot["processes"]): AttentionSnapshot {
  return {
    processes,
    total: processes.reduce((sum, entry) => sum + entry.kinds.length, 0),
  };
}

const NOTHING = snapshot();
const ONE_CRASH = snapshot({ process: 1, kinds: ["crashed"] });
const TWO_UNREAD = snapshot(
  { process: 1, kinds: ["crashed"] },
  { process: 2, kinds: ["agent_error"] },
);

beforeEach(() => {
  vi.clearAllMocks();
  publish = undefined;
  query.mockResolvedValue(NOTHING);
  clear.mockResolvedValue(undefined);
  listen.mockImplementation((handler) => {
    publish = handler;
    return Promise.resolve(() => {});
  });
});

describe("useAttention", () => {
  it("shows what is already unread when the window opens", async () => {
    // The core keeps unread across a window that was closed, so a shell that only listened for
    // changes would open blank with alerts already waiting.
    query.mockResolvedValue(ONE_CRASH);

    const { result } = renderHook(() => useAttention());

    await waitFor(() => expect(result.current.snapshot).toEqual(ONE_CRASH));
  });

  it("re-reads the core's snapshot when attention changes", async () => {
    const { result } = renderHook(() => useAttention());
    await waitFor(() => expect(result.current.snapshot).toEqual(NOTHING));

    query.mockResolvedValue(TWO_UNREAD);
    act(() => publish?.({ type: "AttentionChanged" }));

    await waitFor(() => expect(result.current.snapshot).toEqual(TWO_UNREAD));
  });

  it("ignores events that are not about attention", async () => {
    const { result } = renderHook(() => useAttention());
    await waitFor(() => expect(result.current.snapshot).toEqual(NOTHING));

    query.mockResolvedValue(TWO_UNREAD);
    act(() => publish?.({ type: "TerminalBell", id: 1 }));

    await waitFor(() => expect(query).toHaveBeenCalledTimes(1));
    expect(result.current.snapshot).toEqual(NOTHING);
  });

  it("keeps what it last showed when a read fails", async () => {
    query.mockResolvedValue(ONE_CRASH);
    const { result } = renderHook(() => useAttention());
    await waitFor(() => expect(result.current.snapshot).toEqual(ONE_CRASH));

    // Flashing to zero would say "nothing needs you" on the strength of a failed read.
    query.mockRejectedValue(new Error("backend gone"));
    act(() => publish?.({ type: "AttentionChanged" }));

    await waitFor(() => expect(query).toHaveBeenCalledTimes(2));
    expect(result.current.snapshot).toEqual(ONE_CRASH);
  });

  it("ignores an answer that is not a snapshot", async () => {
    query.mockResolvedValue(ONE_CRASH);
    const { result } = renderHook(() => useAttention());
    await waitFor(() => expect(result.current.snapshot).toEqual(ONE_CRASH));

    // A command the backend does not know resolves to `undefined` rather than rejecting. Adopting
    // it would put a non-snapshot into every indicator's derivation and throw into the render tree.
    query.mockResolvedValue(undefined as unknown as AttentionSnapshot);
    act(() => publish?.({ type: "AttentionChanged" }));

    await waitFor(() => expect(query).toHaveBeenCalledTimes(2));
    expect(result.current.snapshot).toEqual(ONE_CRASH);
  });

  it("settles on the newest read when a burst of changes overlaps", async () => {
    const { result } = renderHook(() => useAttention());
    await waitFor(() => expect(result.current.snapshot).toEqual(NOTHING));

    // Two reads in flight, the first answering last: the stale answer must not win.
    let answerFirst: (value: AttentionSnapshot) => void = () => {};
    query.mockReturnValueOnce(new Promise((resolve) => (answerFirst = resolve)));
    query.mockResolvedValueOnce(TWO_UNREAD);

    act(() => publish?.({ type: "AttentionChanged" }));
    act(() => publish?.({ type: "AttentionChanged" }));
    await waitFor(() => expect(result.current.snapshot).toEqual(TWO_UNREAD));

    await act(async () => {
      answerFirst(ONE_CRASH);
    });

    expect(result.current.snapshot).toEqual(TWO_UNREAD);
  });

  it("asks the core to clear everything rather than emptying itself", async () => {
    query.mockResolvedValue(TWO_UNREAD);
    const { result } = renderHook(() => useAttention());
    await waitFor(() => expect(result.current.snapshot).toEqual(TWO_UNREAD));

    query.mockResolvedValue(NOTHING);
    await act(async () => {
      result.current.clearAll();
    });

    // The core is the one source. Clearing must not empty the surface locally — until the core
    // says the registry is empty, it is not, and a surface that emptied itself would disagree
    // with the dock badge reading the same snapshot.
    expect(clear).toHaveBeenCalled();
    expect(result.current.snapshot).toEqual(TWO_UNREAD);

    // The announcement is what empties it.
    act(() => publish?.({ type: "AttentionChanged" }));
    await waitFor(() => expect(result.current.snapshot).toEqual(NOTHING));
  });

  it("stops listening when the window goes away", async () => {
    const stop = vi.fn();
    listen.mockImplementation((handler) => {
      publish = handler;
      return Promise.resolve(stop);
    });
    const { unmount } = renderHook(() => useAttention());
    await waitFor(() => expect(publish).toBeDefined());

    unmount();

    await waitFor(() => expect(stop).toHaveBeenCalled());
  });
});
