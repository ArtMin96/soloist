import { $, $$ } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";

// The picker is lazy-loaded behind a deferred overlay, so every read waits rather than assumes.
const ROOT = "[cmdk-root]";
const ITEM = "[cmdk-item]";

// The picker identifies each entry by a kind-prefixed value, so an agent tool named "Terminal"
// cannot be mistaken for the terminal entry. Mirrors the component's own encoding — the value is
// what `data-value` carries, and it is not what the row reads.
const AGENT_ENTRY_PREFIX = "agent:";

/** The terminal entry's value, as the picker identifies it. */
export const TERMINAL_ENTRY = "terminal";

/** An agent tool's entry value, as the picker identifies it. */
export const agentEntry = (tool: string) => `${AGENT_ENTRY_PREFIX}${tool}`;

/** The launch picker: choose an agent tool or a terminal, and which project to open it in. */
export const launchPicker = {
  async waitUntilOpen(): Promise<void> {
    await $(ROOT).waitForDisplayed({ timeout: WAIT.render });
  },

  /** Whether the picker is up right now — read without waiting, so a caller can drive it open. */
  async isOpen(): Promise<boolean> {
    return $(ROOT).isDisplayed();
  },

  async waitUntilClosed(): Promise<void> {
    await $(ROOT).waitForDisplayed({ timeout: WAIT.render, reverse: true });
  },

  /** The entries offered (agent tools and the terminal), in the order shown. */
  async entries(): Promise<string[]> {
    await this.waitUntilOpen();
    const names = await $$(ITEM).map((item) => item.getAttribute("data-value"));
    return names.filter((name): name is string => name !== null);
  },

  /** The project the picker will launch into, as the footer reports it. */
  async targetProject(): Promise<string> {
    const target = await $(ROOT).$('[data-testid="palette-target"]');
    await target.waitForDisplayed({ timeout: WAIT.render });
    return (await target.getText()).trim();
  },

  /** The command line shown beside an agent tool — what the app would actually spawn. */
  async commandFor(tool: string): Promise<string> {
    const code = await $(`${ITEM}[data-value="${agentEntry(tool)}"] code`);
    await code.waitForDisplayed({ timeout: WAIT.render });
    return (await code.getText()).trim();
  },

  /** Picks an entry by its value ([`agentEntry`] or [`TERMINAL_ENTRY`]), starting it. */
  async choose(entry: string): Promise<void> {
    const item = await $(`${ITEM}[data-value="${entry}"]`);
    await item.waitForClickable({ timeout: WAIT.render });
    await item.click();
  },
};
