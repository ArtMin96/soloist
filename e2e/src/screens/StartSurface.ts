import { $ } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";

/** The main pane's stable starting point and its available session action. */
export const startSurface = {
  async open(): Promise<void> {
    const button = await $("aria/Start page");
    await button.waitForClickable({ timeout: WAIT.render });
    await button.click();
    await $("aria/Start in Soloist").waitForDisplayed({ timeout: WAIT.render });
  },

  async launchAgent(): Promise<void> {
    await this.open();
    const button = await $("aria/Launch agent");
    await button.waitForClickable({ timeout: WAIT.render });
    await button.click();
  },

  async offersOpenProject(): Promise<boolean> {
    await this.open();
    const button = await $(
      "aria/Open project. Choose a folder already on this computer.",
    );
    await button.waitForExist({ timeout: WAIT.render });
    return button.isDisplayed();
  },
};
