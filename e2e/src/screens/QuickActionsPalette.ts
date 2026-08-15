import { $, browser } from "@wdio/globals";
import { waitUntilOr } from "../harness/waitUntilOr.js";
import { WAIT } from "../harness/waits.js";

// Lazy-loaded behind a deferred overlay, like the launch picker, so every read waits.
const ROOT = "[cmdk-root]";
const ITEM = "[cmdk-item]";

/**
 * The quick-actions palette: every action currently runnable on a process in the active project.
 *
 * Its entries are the same `runnableProcessActions` the sidebar row renders, so this is the second
 * surface a destructive action reaches the user through — and the one where it has to hand off to a
 * dialog while the palette itself is closing.
 */
export const quickActionsPalette = {
  async waitUntilOpen(): Promise<void> {
    await $(ROOT).waitForDisplayed({ timeout: WAIT.render });
  },

  /**
   * Opens the palette with its hotkey, re-pressing until it is actually up.
   *
   * WebKitGTK under classic WebDriver drops a keystroke outright when the app is busy, and the
   * palette is lazy-loaded behind a deferred overlay, so the first open also waits on a chunk fetch
   * — and the row selection that precedes it attaches a terminal, so the app is genuinely busy at
   * exactly that moment. The suite runs with no retries, so one dropped press would take the whole
   * spec file. Safe to repeat because the hotkey *sets* the palette open rather than toggling it,
   * and the re-press is skipped once it is up — the same remedy the launch picker and the project
   * actions menu use. Waits on the core budget, not the render one: the first open is a chunk
   * fetch racing a terminal attach, not a local repaint.
   */
  async open(): Promise<void> {
    await waitUntilOr(
      async () => {
        if (await this.isOpen()) return true;
        await browser.keys(["Control", "p"]);
        return this.isOpen();
      },
      // Reports what had focus, because the usual cause is a focused terminal eating the chord
      // before it reaches the app's window handler — an answer, rather than a bare timeout.
      async () => {
        const focused = await browser.execute(
          () => document.activeElement?.tagName ?? "none",
        );
        return `the quick-actions palette never opened (focus was on ${focused})`;
      },
    );
  },

  async isOpen(): Promise<boolean> {
    return $(ROOT).isDisplayed();
  },

  async waitUntilClosed(): Promise<void> {
    await $(ROOT).waitForDisplayed({ timeout: WAIT.render, reverse: true });
  },

  /**
   * Runs `action` on the process labelled `label`.
   *
   * Entries are keyed by the palette's own `"<label> <action> <id>"` value, so an action is
   * addressed by the pair rather than by position — two processes offering the same action stay
   * distinguishable. The id tail is matched with a prefix selector because the spec knows the
   * label, not the process id the core assigned.
   */
  async run(label: string, action: string): Promise<void> {
    // Resolved by reading the rendered values and matching case-insensitively rather than by a
    // `^=` attribute selector: cmdk owns how it normalises `data-value`, so pinning the exact
    // casing would couple this harness to that internal. Reading them also turns a miss into a
    // listing of what the palette actually offered, instead of an opaque "not clickable".
    const wanted = `${label} ${action} `.toLowerCase();
    const values: string[] = await browser.execute(
      (selector: string) =>
        [...document.querySelectorAll(selector)].map(
          (node) => node.getAttribute("data-value") ?? "",
        ),
      ITEM,
    );
    const value = values.find((candidate) =>
      candidate.toLowerCase().startsWith(wanted),
    );
    if (value === undefined) {
      throw new Error(
        `the palette offered no "${action}" for "${label}"; entries: ${JSON.stringify(values)}`,
      );
    }
    const item = $(`${ITEM}[data-value="${value}"]`);
    await item.waitForClickable({ timeout: WAIT.render });
    await item.click();
  },
};
