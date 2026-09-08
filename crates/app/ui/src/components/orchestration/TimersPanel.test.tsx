// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { TimersPanel } from "@/components/orchestration/TimersPanel";
import { TooltipProvider } from "@/components/ui/tooltip";
import { formatPausedRemaining } from "@/store/timerPanel";
import type { TimerView } from "@/domain";

// Pause/resume/cancel are not exercised here — this file is only the countdown cell's own
// derivation, which needs no IPC.
vi.mock("@/api", () => ({
  timerCancel: vi.fn(),
  timerPause: vi.fn(),
  timerResume: vi.fn(),
}));

afterEach(cleanup);

function pausedTimer(pausedRemainingMillis: number): TimerView {
  return {
    id: 1,
    owner: 7,
    body: "Ping the release channel",
    fire: { kind: "at" },
    status: "paused",
    deadline_unix_millis: Date.now() + 60_000,
    waiting_on: [],
    already_idle: false,
    paused_remaining_millis: pausedRemainingMillis,
  };
}

function armedTimer(remainingMillis: number): TimerView {
  return {
    ...pausedTimer(0),
    status: "armed",
    deadline_unix_millis: Date.now() + remainingMillis,
    paused_remaining_millis: null,
  };
}

function panel(timers: TimerView[]) {
  return (
    <TooltipProvider>
      <TimersPanel timers={timers} agents={[]} project={1} />
    </TooltipProvider>
  );
}

describe("TimersPanel paused countdown", () => {
  it("shows the frozen remaining time", () => {
    render(panel([pausedTimer(125_000)]));

    expect(screen.getByText(formatPausedRemaining(125_000))).toBeTruthy();
  });

  // The frozen text is derived straight from props during render, not synced in an effect, so a
  // pause (or its remaining time changing) never paints the running countdown's last value first.
  it("updates the frozen text immediately when the paused remaining time changes", () => {
    const { rerender } = render(panel([pausedTimer(125_000)]));
    expect(screen.getByText(formatPausedRemaining(125_000))).toBeTruthy();

    rerender(panel([pausedTimer(30_000)]));

    expect(screen.getByText(formatPausedRemaining(30_000))).toBeTruthy();
    expect(screen.queryByText(formatPausedRemaining(125_000))).toBeNull();
  });

  // A resume schedules the next tick on a frame that has not fired yet by the time this assertion
  // runs, so the frozen text from the moment of resuming is what must still be on screen -- not
  // whatever the countdown showed the last time it was running, before the pause.
  it("carries the frozen text into a resume rather than a stale pre-pause countdown", () => {
    const { rerender } = render(panel([armedTimer(8 * 60_000)]));

    rerender(panel([pausedTimer(125_000)]));
    expect(screen.getByText(formatPausedRemaining(125_000))).toBeTruthy();

    rerender(panel([armedTimer(90_000)]));

    expect(screen.getByText(formatPausedRemaining(125_000))).toBeTruthy();
  });
});
