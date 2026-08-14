import { $$, browser } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";
import { waitUntilOr } from "../harness/waitUntilOr.js";

// The in-app alert stack, top-right. The list element only exists once something has been raised,
// so an absent stack is "nothing has alerted", not a broken read.
const STACK = "[data-sonner-toaster]";
const TOAST = "[data-sonner-toast]";
// The card's own action — the region that takes the user to the process. Told apart from the
// Dismiss button beside it by carrying no accessible name of its own: what names it is the alert
// it renders, which is exactly what a spec is looking for.
const OPEN = "button:not([aria-label])";
const LINE = ":scope > span";

/** One alert as the window shows it: the two lines the core wrote. */
export interface ToastHandle {
  title: string;
  body: string;
}

/** What one toast's DOM carries, before an unreadable one is rejected. */
interface ToastSnapshot {
  title: string | null;
  body: string | null;
}

/** The alert stack: what is on screen, and acting on one of them. */
export const toastStack = {
  /**
   * Every alert currently on screen, read in one pass.
   *
   * Atomic for the same reason the sidebar's rows are: the stack re-renders as toasts arrive and
   * expire, so walking it one driver call at a time races that and dies on a stale element. A
   * toast whose lines cannot be read fails the read rather than counting as an untitled alert —
   * otherwise a markup change would answer "no toast named that" for the app.
   */
  async toasts(): Promise<ToastHandle[]> {
    const snapshots: ToastSnapshot[] = await browser.execute(
      (stack: string, toast: string, open: string, line: string) => {
        const root = document.querySelector(stack);
        if (!root) return [];
        return [...root.querySelectorAll(toast)].map((node) => {
          const lines = [
            ...(node.querySelector(open)?.querySelectorAll(line) ?? []),
          ];
          return {
            title: lines[0]?.textContent ?? null,
            body: lines[1]?.textContent ?? null,
          };
        });
      },
      STACK,
      TOAST,
      OPEN,
      LINE,
    );
    return snapshots.map(({ title, body }) => {
      if (title === null || body === null) {
        throw new Error(
          "a toast rendered neither a title nor a body line — the alert markup changed and the " +
            "harness can no longer read what it says",
        );
      }
      return { title, body };
    });
  },

  /**
   * Waits until an alert titled exactly `title` is on screen, then returns it. Only the kinds that
   * stay until acted on are safe to read again later; everything else expires on its own, so a
   * caller that needs the words keeps the handle this returns.
   */
  async waitForToast(title: string): Promise<ToastHandle> {
    let found: ToastHandle | undefined;
    let seen: string[] = [];
    await waitUntilOr(
      async () => {
        const toasts = await this.toasts();
        seen = toasts.map((toast) => toast.title);
        found = toasts.find((toast) => toast.title === title);
        return found !== undefined;
      },
      () =>
        `no alert titled ${JSON.stringify(title)} appeared; on screen: ${JSON.stringify(seen)}`,
    );
    return found as ToastHandle;
  },

  /**
   * Clicks the alert titled `title` — what a user does to be taken to the process it is about.
   *
   * The click is a real one on the real card, so it proves the region is reachable rather than
   * that a handler exists. The button is found by its position in the stack, which the same DOM
   * order gives both reads; a title that names more than one alert is refused rather than
   * guessed at.
   */
  async open(title: string): Promise<void> {
    const toasts = await this.toasts();
    const matches = toasts.filter((toast) => toast.title === title);
    if (matches.length !== 1) {
      throw new Error(
        `expected exactly one alert titled ${JSON.stringify(title)} to click, found ` +
          `${matches.length}; on screen: ${JSON.stringify(toasts.map((toast) => toast.title))}`,
      );
    }
    const index = toasts.findIndex((toast) => toast.title === title);
    const button = (await $$(`${TOAST} ${OPEN}`))[index];
    if (button === undefined) {
      throw new Error(
        `the alert titled ${JSON.stringify(title)} has no clickable card`,
      );
    }
    await button.waitForClickable({ timeout: WAIT.render });
    await button.click();
  },
};
