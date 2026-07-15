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

  /** Clicks "Open project", which raises the OS folder picker — unusable from a spec. */
  async openProject(): Promise<void> {
    const button = await $("aria/Open project");
    await button.waitForClickable({ timeout: WAIT.render });
    await button.click();
  },
};
