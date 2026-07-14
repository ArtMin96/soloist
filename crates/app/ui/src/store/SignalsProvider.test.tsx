// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, waitFor } from "@testing-library/react";

// The agent-activity snapshot, the event subscription, and the resync signal are the IPC boundary;
// mock them so the test drives the provider's own seeding — from the snapshot on mount and again on
// a resync, the self-heal for a dropped `AgentActivityChanged`.
vi.mock("@/api", () => ({
  agentActivity: vi.fn(() => Promise.resolve([])),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
  onResync: vi.fn(() => Promise.resolve(() => {})),
}));

import { agentActivity, onResync } from "@/api";
import { SignalsProvider } from "@/store/SignalsProvider";
import { useSignal } from "@/store/signalsContext";

const snapshot = vi.mocked(agentActivity);
const resync = vi.mocked(onResync);

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

// A leaf that renders one process's idle badge — the surface a dropped delta would leave stale.
function Badge({ id }: { id: number }) {
  const { activity } = useSignal(id);
  return <span data-testid="activity">{activity ?? "none"}</span>;
}

describe("SignalsProvider", () => {
  it("seeds the idle badge from the agent-activity snapshot on mount", async () => {
    snapshot.mockResolvedValue([{ id: 7, activity: "Idle" }]);
    const { getByTestId } = render(
      <SignalsProvider>
        <Badge id={7} />
      </SignalsProvider>,
    );
    await waitFor(() => expect(getByTestId("activity").textContent).toBe("Idle"));
  });

  it("re-seeds on a backend resync, healing a badge a dropped delta left stale", async () => {
    snapshot.mockResolvedValue([{ id: 7, activity: "Working" }]);
    const { getByTestId } = render(
      <SignalsProvider>
        <Badge id={7} />
      </SignalsProvider>,
    );
    await waitFor(() => expect(getByTestId("activity").textContent).toBe("Working"));

    // The agent is Idle now, but its `AgentActivityChanged` was dropped during bus lag; the resync
    // re-reads the snapshot and reconciles the badge.
    snapshot.mockResolvedValue([{ id: 7, activity: "Idle" }]);
    const handler = resync.mock.calls[0]?.[0];
    if (!handler) throw new Error("no resync subscriber registered");
    act(() => handler());
    await waitFor(() => expect(getByTestId("activity").textContent).toBe("Idle"));
  });
});
