import { $, browser } from "@wdio/globals";
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

  /**
   * The text xterm.js has rendered into the visible terminal's viewport — its rows joined by
   * newlines. This is what the window actually shows. The e2e build runs the terminal with
   * screen-reader mode on, which mirrors the live viewport into the accessibility DOM
   * (`.xterm-accessibility-tree`) regardless of renderer — the only DOM-readable source when the GPU
   * (WebGL) renderer is active, since it draws to a canvas. Falls back to the DOM renderer's rows,
   * and to a sentinel when only a canvas is present, so a failed match reports *why*.
   */
  async text(): Promise<string> {
    const host = await $(`${VISIBLE_PANE} ${HOST}`);
    await host.waitForDisplayed({ timeout: WAIT.core });
    return browser.execute(
      (paneSel: string, hostSel: string) => {
        const host = document.querySelector(`${paneSel} ${hostSel}`);
        if (!host) return "";
        const rowsText = (container: Element | null) =>
          container
            ? [...container.children].map((row) => (row as HTMLElement).textContent ?? "").join("\n")
            : "";
        const a11y = rowsText(host.querySelector(".xterm-accessibility-tree"));
        if (a11y.trim() !== "") return a11y;
        const dom = rowsText(host.querySelector(".xterm-rows"));
        if (dom.trim() !== "") return dom;
        if (host.querySelector("canvas")) {
          return "[[terminal rendered to a WebGL canvas — no DOM text to read]]";
        }
        return "";
      },
      VISIBLE_PANE,
      HOST,
    );
  },

  /**
   * Waits until the visible terminal's rendered text contains `substring`, then returns the full
   * text. Used to observe output the app delivers over a real PTY — the wake body a fired timer
   * writes to the lead's stdin arrives this way — which no repaint can fake.
   */
  async waitForText(substring: string): Promise<string> {
    let last = "";
    try {
      await browser.waitUntil(
        async () => {
          last = await this.text();
          return last.includes(substring);
        },
        { timeout: WAIT.core },
      );
    } catch {
      throw new Error(
        `the visible terminal never showed ${JSON.stringify(substring)}; last read:\n${last}`,
      );
    }
    return last;
  },
};
