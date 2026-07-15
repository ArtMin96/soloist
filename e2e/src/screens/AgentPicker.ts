import { $, $$ } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";

// The picker is lazy-loaded behind a deferred overlay, so every read waits rather than assumes.
const ROOT = "[cmdk-root]";
const ITEM = "[cmdk-item]";

/** The agent picker: choose which tool to launch, and into which project. */
export const agentPicker = {
  async waitUntilOpen(): Promise<void> {
    await $(ROOT).waitForDisplayed({ timeout: WAIT.render });
  },

  async waitUntilClosed(): Promise<void> {
    await $(ROOT).waitForDisplayed({ timeout: WAIT.render, reverse: true });
  },

  /** The tool names offered, in the order shown. */
  async tools(): Promise<string[]> {
    await this.waitUntilOpen();
    const names = await $$(ITEM).map((item) => item.getAttribute("data-value"));
    return names.filter((name): name is string => name !== null);
  },

  /**
   * The project the picker will launch into, as the footer reports it. The footer renders a "▸ "
   * marker before the name; this returns just the name.
   */
  async targetProject(): Promise<string> {
    const footer = await $(ROOT).$("span*=▸");
    await footer.waitForDisplayed({ timeout: WAIT.render });
    return (await footer.getText()).replace("▸", "").trim();
  },

  /** The command line shown beside a tool — what the app would actually spawn. */
  async commandFor(tool: string): Promise<string> {
    const code = await $(`${ITEM}[data-value="${tool}"] code`);
    await code.waitForDisplayed({ timeout: WAIT.render });
    return (await code.getText()).trim();
  },

  /** Picks a tool by name, launching it into the target project. */
  async choose(tool: string): Promise<void> {
    const item = await $(`${ITEM}[data-value="${tool}"]`);
    await item.waitForClickable({ timeout: WAIT.render });
    await item.click();
  },
};
