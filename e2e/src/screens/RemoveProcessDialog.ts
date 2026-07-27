import { $ } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";

const DIALOG = '[role="dialog"]';

/**
 * The confirmation raised before a *live* agent or terminal is removed. A resting one never
 * reaches here — it is removed outright — so a spec that sees this dialog has already proved the
 * process was still running.
 */
export const removeProcessDialog = {
  async waitUntilOpen(): Promise<void> {
    await $(DIALOG).waitForDisplayed({
      timeout: WAIT.render,
      timeoutMsg: "the remove confirmation never opened",
    });
  },

  async waitUntilClosed(): Promise<void> {
    await $(DIALOG).waitForDisplayed({
      reverse: true,
      timeout: WAIT.render,
      timeoutMsg: "the remove confirmation never closed",
    });
  },

  /** Whether the confirmation is currently up at all. */
  async isOpen(): Promise<boolean> {
    return $(DIALOG).isDisplayed();
  },

  /**
   * Whether it names `label` as the process it would remove, matching the title the user reads.
   *
   * Read from the dialog's own text rather than by a text selector: the title is one text node
   * inside nested elements, which `*=` does not reliably resolve, and reading the whole dialog
   * also proves the name is actually rendered rather than merely present in the markup.
   */
  async names(label: string): Promise<boolean> {
    return (await $(DIALOG).getText()).includes(`Remove “${label}”?`);
  },

  /** Confirms the removal. */
  async confirm(): Promise<void> {
    await this.click("Remove");
  },

  /** Backs out, leaving the process running. */
  async cancel(): Promise<void> {
    await this.click("Cancel");
  },

  async click(name: string): Promise<void> {
    // Scoped to the dialog: the sidebar row behind it carries its own "Remove" control, so a
    // global lookup could click the affordance that opened this instead of answering it.
    const button = $(DIALOG).$(`button=${name}`);
    await button.waitForClickable({ timeout: WAIT.render });
    await button.click();
  },
};
