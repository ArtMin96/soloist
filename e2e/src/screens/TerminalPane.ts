import { $ } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";

// The pane keeps every opened process mounted and hides all but the selected one, so a query for
// "the terminal" must mean the visible one rather than any that exists.
const VISIBLE_PANE = "section:not(.hidden)";
const HOST = '[data-testid="terminal-host"]';

/** The main pane when a process is selected: its header and the terminal surface. */
export const terminalPane = {
  /**
   * The header's title. This is the process's label until the process sets its own via an OSC
   * escape, which a live agent does — so it identifies the process rather than restating its label.
   */
  async title(): Promise<string> {
    const heading = await $(`${VISIBLE_PANE} header span`);
    await heading.waitForDisplayed({ timeout: WAIT.render });
    return (await heading.getText()).trim();
  },

  /** Whether the terminal surface itself is mounted and laid out. */
  async isMounted(): Promise<boolean> {
    const host = await $(`${VISIBLE_PANE} ${HOST}`);
    await host.waitForExist({ timeout: WAIT.core });
    return host.isDisplayed();
  },

  /** The rendered size of the terminal surface — proves it was given real layout, not zero. */
  async size(): Promise<{ width: number; height: number }> {
    const host = await $(`${VISIBLE_PANE} ${HOST}`);
    await host.waitForDisplayed({ timeout: WAIT.core });
    return host.getSize();
  },
};
