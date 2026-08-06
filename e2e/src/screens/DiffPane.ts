import { $, browser } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";
import { waitUntilOr } from "../harness/waitUntilOr.js";

const PANE = 'section[aria-label="Diff"]';

/** The split at the foot of the main area that shows one path's change. */
export const diffPane = {
  /**
   * Everything the split currently renders, read in one pass.
   *
   * The viewer colours a line by splitting it across as many elements as it found tokens, and
   * re-splits it a moment later when the grammar it asked for arrives — so a line is read as the
   * text the split shows, never as a hierarchy walked element by element.
   */
  async text(): Promise<string> {
    return browser.execute((pane: string) => {
      return (document.querySelector(pane) as HTMLElement | null)?.innerText ?? "";
    }, PANE);
  },

  /**
   * Waits until the split has rendered `substring`, then returns everything it shows — so a
   * caller reads the rest of the change from the same settled frame it matched on.
   */
  async waitForText(substring: string): Promise<string> {
    await $(PANE).waitForDisplayed({ timeout: WAIT.render });
    let last = "";
    await waitUntilOr(
      async () => {
        last = await this.text();
        return last.includes(substring);
      },
      () => `the diff never showed ${JSON.stringify(substring)}; last read:\n${last}`,
    );
    return last;
  },
};
