import { $ } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";

/** The window's top bar: the app-level actions that are always reachable. */
export const titlebar = {
  /** Clicks "Launch agent", which opens the agent picker. */
  async launchAgent(): Promise<void> {
    const button = await $("aria/Launch agent");
    await button.waitForClickable({ timeout: WAIT.render });
    await button.click();
  },

  /**
   * Whether "Open project" is offered. Clicking it raises the OS folder picker, which a WebDriver
   * session cannot drive — so the affordance is only ever asserted on, never driven; flows open
   * projects through `openProject` instead.
   */
  async offersOpenProject(): Promise<boolean> {
    const button = await $("aria/Open project");
    await button.waitForExist({ timeout: WAIT.render });
    return button.isDisplayed();
  },
};
