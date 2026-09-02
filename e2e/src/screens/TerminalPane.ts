import { $, browser } from "@wdio/globals";
import { ignoringWhitespace } from "../harness/ignoringWhitespace.js";
import { WAIT } from "../harness/waits.js";
import { waitUntilOr } from "../harness/waitUntilOr.js";

// The pane keeps every opened process mounted and hides all but the selected one, so a query for
// "the terminal" must mean the visible one rather than any that exists.
const VISIBLE_PANE = "section:not(.hidden)";
const HOST = '[data-testid="terminal-host"]';
const SESSION_WORK = "[data-session-work]";

/**
 * Activates one session-work item by its exact selector — a todo's `data-session-todo` or a
 * scratchpad's `data-session-scratchpad`. Clicks it inline when the header shows it there;
 * otherwise opens the bar's overflow control first (its items render through a Radix portal, so
 * the item is then searched document-wide rather than scoped to the pane).
 */
async function activateSessionItem(itemSelector: string): Promise<void> {
  const inline = $(`${VISIBLE_PANE} ${SESSION_WORK} ${itemSelector}`);
  if (await inline.isExisting()) {
    await inline.waitForClickable({ timeout: WAIT.render });
    await inline.click();
    return;
  }
  const overflow = $(`${VISIBLE_PANE} [data-session-overflow]`);
  await overflow.waitForClickable({ timeout: WAIT.render });
  await overflow.click();
  const item = $(itemSelector);
  await item.waitForClickable({ timeout: WAIT.render });
  await item.click();
}

/**
 * The visible agent's session-work context as the header shows it: todo ids under "Current
 * work", and todo ids and scratchpad names under "This session".
 */
interface SessionWorkRead {
  currentTodos: number[];
  sessionTodos: number[];
  sessionScratchpads: string[];
}

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
            ? [...container.children]
                .map((row) => (row as HTMLElement).textContent ?? "")
                .join("\n")
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
   * Waits until the visible terminal shows `substring`, then returns the full rendered text as read.
   * Used to observe output the app delivers over a real PTY — the wake body a fired timer writes to
   * the lead's stdin arrives this way — which no repaint can fake.
   *
   * The match ignores whitespace, so it holds wherever the pane happened to wrap the line; a caller
   * asserting further substrings of the returned text owes the same (`ignoringWhitespace`), or it
   * waits on one rule and asserts by another.
   */
  async waitForText(substring: string): Promise<string> {
    let last = "";
    await waitUntilOr(
      async () => {
        last = await this.text();
        return ignoringWhitespace(last).includes(ignoringWhitespace(substring));
      },
      () =>
        `the visible terminal never showed ${JSON.stringify(substring)}; last read:\n${last}`,
    );
    return last;
  },

  /**
   * The visible agent's live session-work context, read off the real `data-session-*` handles:
   * the todo ids under "Current work", and the todo ids and scratchpad names under "This
   * session". Reads the items shown inline in the header row — the small counts every current
   * caller exercises — not whatever additionally sits behind the overflow control.
   *
   * `null` when no terminal pane is visible at all, which is not the same as a visible header
   * with nothing in it: stopping the selected process navigates the window away from its pane,
   * and an "empty" read of a pane that is not on screen would let a clear pass vacuously.
   */
  async sessionWork(): Promise<SessionWorkRead | null> {
    return browser.execute(
      (paneSel: string, workSel: string) => {
        const pane = document.querySelector(paneSel);
        if (!pane) return null;
        const bar = pane.querySelector(workSel);
        const numbers = (selector: string) =>
          bar
            ? [...bar.querySelectorAll(selector)].map((el) =>
                Number(el.getAttribute("data-session-todo")),
              )
            : [];
        const names = (selector: string) =>
          bar
            ? [...bar.querySelectorAll(selector)].map(
                (el) => el.getAttribute("data-session-scratchpad") ?? "",
              )
            : [];
        return {
          currentTodos: numbers(
            '[data-session-group="current"] [data-session-todo]',
          ),
          sessionTodos: numbers(
            '[data-session-group="session"] [data-session-todo]',
          ),
          sessionScratchpads: names(
            '[data-session-group="session"] [data-session-scratchpad]',
          ),
        };
      },
      VISIBLE_PANE,
      SESSION_WORK,
    );
  },

  /**
   * Waits until the visible agent's "Current work" names `todo` — a lock the real core reports
   * this process holding — then returns the whole context as read at that moment.
   */
  async waitForCurrentTodo(todo: number): Promise<SessionWorkRead> {
    let last: SessionWorkRead | null | undefined;
    await waitUntilOr(
      async () => {
        last = await this.sessionWork();
        return last?.currentTodos.includes(todo) ?? false;
      },
      () =>
        `the visible agent's "Current work" never named todo ${todo}; last read: ${
          last === null ? "no terminal pane is visible" : JSON.stringify(last)
        }`,
    );
    return last as SessionWorkRead;
  },

  /**
   * Waits until the visible agent's header carries no session-work context at all — what the
   * per-run record leaves behind once the process has ended.
   */
  async waitForSessionWorkCleared(): Promise<void> {
    let last: SessionWorkRead | null | undefined;
    await waitUntilOr(
      async () => {
        last = await this.sessionWork();
        return (
          last !== null &&
          last.currentTodos.length === 0 &&
          last.sessionTodos.length === 0 &&
          last.sessionScratchpads.length === 0
        );
      },
      () =>
        `the visible agent's session-work context never cleared; last read: ${
          last === null ? "no terminal pane is visible" : JSON.stringify(last)
        }`,
    );
  },

  /** Activates the visible agent's session-work item for `todo`, opening the orchestration pane. */
  async openSessionTodo(todo: number): Promise<void> {
    await activateSessionItem(`[data-session-todo="${todo}"]`);
  },

  /** Activates the visible agent's session-work item for the scratchpad named `name`. */
  async openSessionScratchpad(name: string): Promise<void> {
    await activateSessionItem(`[data-session-scratchpad="${name}"]`);
  },
};
