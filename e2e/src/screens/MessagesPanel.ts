import { $ } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";

// The orchestration pane's Messages view: the project's retained agent-to-agent transcript, one
// chronological stream of rows carrying the routing, the kind, the delivery outcome, and the body.
// It is a live region (`role="log"`) rather than a list of testids, so a spec reads it by the same
// accessible name a screen reader announces.
const TRANSCRIPT = '[role="log"][aria-label="Agent messages"]';

/** The project's readable record of what the agents said to each other. */
export const messagesPanel = {
  /**
   * Waits for the transcript to render and returns its whole text. Unlike a terminal read, this is
   * the app's own surface: a body legible here was carried by the core into the retained record and
   * rendered by Soloist, not echoed by the fixture that received it.
   */
  async waitForTranscript(): Promise<string> {
    const log = await $(TRANSCRIPT);
    await log.waitForDisplayed({ timeout: WAIT.core });
    return (await log.getText()).trim();
  },
};
