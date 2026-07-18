import { $ } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";
import { waitUntilOr } from "../harness/waitUntilOr.js";

// The orchestration pane's Timers view: the armed timers a project's leads hold, each rendered with
// a fire-condition badge, a live countdown to the max-wait deadline, and — for a fire-when-idle timer
// with watches outstanding — the agents it is still waiting to go idle. The panel carries no per-row
// role or testid, so a timer is read through the two accessible names its parts expose: the
// countdown's "Time remaining: …" and the waiting-on group's "Waiting on N agent(s)". One armed
// timer at a time is all this walk arms, so a panel-wide read is unambiguous.
const COUNTDOWN = '[aria-label^="Time remaining:"]';
const WAITING_ON = '[aria-label^="Waiting on"]';

/** The project's live timers surface, read through the panel's accessible names. */
export const timersPanel = {
  /**
   * Waits until an armed timer's live countdown is shown, then returns its remaining-time text (the
   * accessible name after "Time remaining: ", e.g. "9m 58s"). Proves the real core surfaced a real
   * armed timer with a real deadline — the countdown value itself is computed UI-side and is the
   * Vitest suite's to verify, so this asserts only that one is rendered.
   */
  async waitForCountdown(): Promise<string> {
    const countdown = await $(COUNTDOWN);
    await countdown.waitForDisplayed({ timeout: WAIT.core });
    const label = (await countdown.getAttribute("aria-label")) ?? "";
    return label.replace(/^Time remaining:\s*/, "").trim();
  },

  /**
   * Waits until an armed fire-when-idle timer shows the agents it is waiting on, then returns their
   * chip labels as one string. These are the watched processes the real core reports not yet idle —
   * data a panel wired to nothing could not name.
   */
  async waitForWaitingOn(): Promise<string> {
    const group = await $(WAITING_ON);
    await group.waitForDisplayed({ timeout: WAIT.core });
    return (await group.getText()).trim();
  },

  /**
   * Waits until no armed timer remains in the panel — its countdown gone from the DOM — proving the
   * timer fired and left, not merely that a repaint hid it.
   */
  async waitForNoTimers(): Promise<void> {
    await waitUntilOr(
      async () => !(await $(COUNTDOWN).isExisting()),
      () => "an armed timer never left the panel — it did not fire",
    );
  },
};
