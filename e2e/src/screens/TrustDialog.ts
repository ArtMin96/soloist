import { $ } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";

const DIALOG = '[role="dialog"]';

/**
 * The trust review dialog: raised by a config change or opened from a command's Trust
 * affordance. Both routes show the exact variant that a grant will authorize.
 */
export const trustDialog = {
  /** Waits for the review to open — the observable outcome of a config change that needs trust. */
  async waitUntilOpen(): Promise<void> {
    await $(DIALOG).waitForDisplayed({
      timeout: WAIT.core,
      timeoutMsg: "the trust review dialog never opened",
    });
  },

  /** Waits for the review to close — every pending command was trusted or it was dismissed. */
  async waitUntilClosed(): Promise<void> {
    await $(DIALOG).waitForDisplayed({
      reverse: true,
      timeout: WAIT.render,
      timeoutMsg: "the trust review dialog never closed",
    });
  },

  /** Whether the review currently lists `name` as needing trust. */
  async listsCommand(name: string): Promise<boolean> {
    return $(DIALOG).$(`aria/Trust ${name}`).isDisplayed();
  },

  /** Whether the review visibly contains the exact command line being authorized. */
  async showsCommand(command: string): Promise<boolean> {
    return $(DIALOG).$(`code=${command}`).isDisplayed();
  },

  /** Trusts one listed command from the review. */
  async trust(name: string): Promise<void> {
    const button = await $(DIALOG).$(`aria/Trust ${name}`);
    await button.waitForClickable({ timeout: WAIT.render });
    await button.click();
  },
};
