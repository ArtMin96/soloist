import { $ } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";

// The global Settings surface: the sidebar footer opens it through its accessible name, and a left
// rail switches sections through the exact text the user reads.
const FOOTER_ENTRY = 'button[aria-label="Settings"]';
const RAIL = '[role="tablist"][aria-label="Settings sections"]';
const TEMPLATES_TAB = "button=Templates";

/** The Settings overlay: opening it, and choosing which section is shown. */
export const settingsOverlay = {
  /** Opens Settings from the sidebar's footer entry and waits for the section rail. */
  async open(): Promise<void> {
    const entry = $(FOOTER_ENTRY);
    await entry.waitForClickable({ timeout: WAIT.render });
    await entry.click();
    await $(RAIL).waitForDisplayed({ timeout: WAIT.render });
  },

  /**
   * Shows the Templates section, waiting until the rail reports it selected — the tab's own state,
   * so the wait cannot pass on a click the rail dropped. The tab is looked up inside the rail: the
   * panel it opens carries a "Templates" back control of its own, which an unscoped lookup would
   * race.
   */
  async openTemplates(): Promise<void> {
    const tab = $(RAIL).$(TEMPLATES_TAB);
    await tab.waitForClickable({ timeout: WAIT.render });
    await tab.click();
    await tab.waitUntil(
      async () => (await tab.getAttribute("aria-selected")) === "true",
      {
        timeout: WAIT.render,
        timeoutMsg: "the Templates section never became the selected one",
      },
    );
  },
};
