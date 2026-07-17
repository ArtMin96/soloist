import { $ } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";

const TITLE = "aria/Trust changed commands";

/**
 * The trust review dialog: raised when an open project's `solo.yml` changes with commands
 * whose variant is untrusted. Opening it is never a click — the app raises it from a config
 * change — so the screen only ever waits for it and acts inside it.
 */
export const trustDialog = {
  /** Waits for the review to open — the observable outcome of a config change that needs trust. */
  async waitUntilOpen(): Promise<void> {
    await $(TITLE).waitForDisplayed({
      timeout: WAIT.core,
      timeoutMsg: "the trust review dialog never opened",
    });
  },

  /** Waits for the review to close — every pending command was trusted or it was dismissed. */
  async waitUntilClosed(): Promise<void> {
    await $(TITLE).waitForDisplayed({
      reverse: true,
      timeout: WAIT.render,
      timeoutMsg: "the trust review dialog never closed",
    });
  },

  /** Whether the review currently lists `name` as needing trust. */
  async listsCommand(name: string): Promise<boolean> {
    return $(`aria/Trust ${name}`).isDisplayed();
  },

  /** Trusts one listed command from the review. */
  async trust(name: string): Promise<void> {
    const button = await $(`aria/Trust ${name}`);
    await button.waitForClickable({ timeout: WAIT.render });
    await button.click();
  },
};
