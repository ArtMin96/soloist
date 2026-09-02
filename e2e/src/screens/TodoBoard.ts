import { $, browser } from "@wdio/globals";
import type { TodoStatus } from "@domain";
import { WAIT } from "../harness/waits.js";
import { waitUntilOr } from "../harness/waitUntilOr.js";

// The to-do board: the project's shared work items as a list of cards, and the detail panel one
// card hands the whole pane over to. Selectors live only here, addressed by the row's `data-todo-*`
// handles rather than DOM position — the card's own button and, on a locked row, the agent control
// are siblings, so neither structure nor styling can move a handle out from under this file.
//
// A row is found by the exact text of its title (`data-todo-title`), the same justified structural
// handle the sidebar uses for a process row, and then addressed by the id the core gave it.
//
// **Both panels are mounted at all times** — the one that is not showing is translated off the
// track and made `inert`, never unmounted — so the existence of a panel proves nothing about which
// one the user is on. `data-todo-route` on the viewport is the only honest read of that, and every
// detail handle here is scoped under `[data-todo-detail]`, because `inert` does not remove a node
// from `querySelectorAll` and the row and the detail deliberately carry some of the same handles
// (`data-todo-status`, `data-todo-agent`).

/** The attribute the board's transition viewport names the panel currently on screen with. */
const ROUTE_ATTR = "data-todo-route";
/**
 * The attribute the detail panel's root names its todo with. Unlike the panel slot it sits in, the
 * root exists only while a todo is open, and it names *which* — so it settles both questions the
 * route alone cannot.
 */
const DETAIL_ATTR = "data-todo-detail";
const DETAIL = `[${DETAIL_ATTR}]`;
/** The detail's return control — and where the board parks focus when it opens the panel. */
const BACK = "[data-todo-back]";

/** Which of the board's two panels the user is on. */
type TodoRoute = "list" | "detail";

/** The agent control a locked row carries: who it names, and the process it opens. */
interface TodoLock {
  owner: string;
  process: number;
}

/**
 * One todo row's readable state: the id the core assigned it, its declared status, whether the
 * blocked gate shows, and the lock the core reports (or `null` while it is unlocked).
 */
interface TodoState {
  id: number;
  status: string;
  blocked: boolean;
  lock: TodoLock | null;
}

/** What the open detail panel says about the todo it is showing. */
interface TodoDetailState {
  /** The todo the panel is open on, as the panel's own handle names it. */
  id: number;
  /** The panel's heading — the whole title, which the row truncates. */
  title: string;
  /** The declared status its chip carries, off `data-status`. */
  status: string;
  /** The Details grid's Scratchpad field: the document this todo derives from, as prose. */
  scratchpad: string;
}

/**
 * The board as one atomic read: which panel is showing, what the detail panel holds, and where
 * focus is. Read in a single pass rather than one query at a time — a route change moves focus and
 * swaps two panels' `inert` state in the same commit, so reading those separately can catch the
 * board mid-swap and report a state it was never actually in.
 */
interface BoardView {
  /** The panel on screen, or `null` when the board is not rendered at all. */
  route: TodoRoute | null;
  /** The todo the detail panel holds, or `null` once the panel has been dropped. */
  detail: TodoDetailState | null;
  /** Whether DOM focus is on the detail's Back control. */
  backFocused: boolean;
  /** The id of the row that holds DOM focus, or `null` when no row does. */
  focusedRow: number | null;
}

/** How wide one box's content is against how wide the box is — a horizontal-fit reading. */
export interface FitReading {
  what: string;
  scrollWidth: number;
  clientWidth: number;
}

/** A fit reading whose box was not found — the shape a measurement comes back in before checking. */
type AttemptedFitReading = { what: string; scrollWidth: number | null; clientWidth: number | null };

/**
 * Every box a fit read asked for, or an error naming the ones that were not there.
 *
 * A missing box must never read as "nothing overflows". Dropping it would leave the caller filtering
 * an empty list and passing — so a surface that has been restructured out from under the walk would
 * be reported as fitting rather than as unmeasured, which is the one shape of green this suite
 * exists to prevent.
 */
function measured(readings: AttemptedFitReading[]): FitReading[] {
  const missing = readings.filter((box) => box.scrollWidth === null).map((box) => box.what);
  if (missing.length > 0) {
    throw new Error(
      `nothing to measure for ${JSON.stringify(missing)} — the surface moved, so its fit is ` +
        `unknown rather than fine`,
    );
  }
  return readings as FitReading[];
}

/** The board, its rows, and the detail panel one of them opens. */
export const todoBoard = {
  /**
   * One todo row's state — its declared status label and derived blocked gate — read in one pass by
   * the exact title, re-querying the DOM so a live re-render (a completion elsewhere) cannot stale
   * the read. `null` when no row carries that title yet.
   */
  async read(title: string): Promise<TodoState | null> {
    return browser.execute((todoTitle: string) => {
      const item = [...document.querySelectorAll("[data-todo-id]")].find(
        (candidate) => candidate.querySelector("[data-todo-title]")?.textContent?.trim() === todoTitle,
      );
      if (!item) return null;
      const status = item.querySelector("[data-todo-status]");
      const agent = item.querySelector("[data-todo-agent]");
      return {
        id: Number(item.getAttribute("data-todo-id")),
        status: status?.getAttribute("data-status") ?? "",
        blocked: item.querySelector("[data-todo-blockers]") !== null,
        lock:
          agent === null
            ? null
            : {
                owner: agent.textContent?.trim() ?? "",
                process: Number(agent.getAttribute("data-process-id")),
              },
      };
    }, title);
  },

  /**
   * Waits until the row titled `title` carries its agent control — rendered only once the core
   * reports the todo locked — and returns who it names and which process it opens.
   */
  async waitForLock(title: string): Promise<TodoLock> {
    let lock: TodoLock | null | undefined;
    await waitUntilOr(
      async () => {
        lock = (await this.read(title))?.lock;
        return lock != null;
      },
      () =>
        `todo "${title}" never showed an agent control; last seen: ${
          lock === undefined ? "no such todo" : "unlocked"
        }`,
    );
    return lock as TodoLock;
  },

  /** The row's declared status as the domain enum — read directly off `data-status`. */
  async status(title: string): Promise<TodoStatus | null> {
    const state = await this.read(title);
    if (state === null) return null;
    return (state.status as TodoStatus) || null;
  },

  /** Waits until a row titled `title` is rendered. */
  async waitForTodo(title: string): Promise<void> {
    await waitUntilOr(
      async () => (await this.read(title)) !== null,
      async () =>
        `no todo titled "${title}" appeared; rendered todos: ${JSON.stringify(await this.titles())}`,
    );
  },

  /** Every rendered todo title, for reporting a miss. */
  async titles(): Promise<string[]> {
    return browser.execute(() =>
      [...document.querySelectorAll("[data-todo-title]")].map(
        (title) => title.textContent?.trim() ?? "",
      ),
    );
  },

  /** Waits until the row titled `title` reports the declared `status`. */
  async waitForStatus(title: string, status: TodoStatus): Promise<void> {
    let last: string | undefined;
    await waitUntilOr(
      async () => {
        last = (await this.read(title))?.status;
        return last === status;
      },
      () =>
        `todo "${title}" never reported status "${status}"; last seen: ${last ?? "no such todo"}`,
    );
  },

  /** Waits until the row titled `title` shows its blocked gate as `blocked`. */
  async waitForBlocked(title: string, blocked: boolean): Promise<void> {
    let last: boolean | undefined;
    await waitUntilOr(
      async () => {
        last = (await this.read(title))?.blocked;
        return last === blocked;
      },
      () =>
        `todo "${title}" never showed blocked=${blocked}; last seen: ${last ?? "no such todo"}`,
    );
  },

  /**
   * The board's route, its detail panel and its focus, in one pass. Everything here is read from
   * what the engine actually settled on — the viewport's own route attribute and
   * `document.activeElement` — never from a component's idea of where it put things.
   */
  async view(): Promise<BoardView> {
    return browser.execute(
      (routeAttr: string, detailAttr: string, backSel: string) => {
        const viewport = document.querySelector(`[${routeAttr}]`);
        const detail = document.querySelector(`[${detailAttr}]`);
        const active = document.activeElement;
        const focusedRow = active?.closest("[data-todo-id]")?.getAttribute("data-todo-id") ?? null;
        return {
          route: (viewport?.getAttribute(routeAttr) ?? null) as "list" | "detail" | null,
          detail:
            detail === null
              ? null
              : {
                  id: Number(detail.getAttribute(detailAttr)),
                  // The heading is read for the failure message only — which todo the panel is on
                  // is settled by its own handle above, so a retitled todo is not a failed walk.
                  title: detail.querySelector("h2")?.textContent?.trim() ?? "",
                  status:
                    detail.querySelector("[data-todo-status]")?.getAttribute("data-status") ?? "",
                  scratchpad:
                    detail.querySelector("[data-todo-scratchpad]")?.textContent?.trim() ?? "",
                },
          backFocused: active !== null && detail?.querySelector(backSel) === active,
          focusedRow: focusedRow === null ? null : Number(focusedRow),
        };
      },
      ROUTE_ATTR,
      DETAIL_ATTR,
      BACK,
    );
  },

  /** Waits until the board is on `route`. */
  async waitForRoute(route: TodoRoute): Promise<void> {
    let last: BoardView | undefined;
    await waitUntilOr(
      async () => {
        last = await this.view();
        return last.route === route;
      },
      () =>
        `the board never showed its ${route} panel; last read: ${JSON.stringify(last ?? null)}`,
      WAIT.render,
    );
  },

  /**
   * Waits until the detail panel is showing the todo titled `title`, and returns the board as read
   * at that moment — including where focus ended up, which the caller asserts.
   *
   * Focus is not part of the condition on purpose: the board opens the panel and moves focus into
   * it in one layout effect, so a snapshot that sees the panel has already seen wherever focus
   * settled. Waiting on focus too would turn a focus that never moved into a timeout instead of the
   * failed assertion it is.
   */
  async waitForDetail(title: string): Promise<BoardView> {
    const id = await this.idOf(title);
    let last: BoardView | undefined;
    await waitUntilOr(
      async () => {
        last = await this.view();
        return last.route === "detail" && last.detail?.id === id;
      },
      () =>
        `the detail panel never opened on "${title}" (todo ${id}); last read: ${JSON.stringify(
          last ?? null,
        )}`,
    );
    return last as BoardView;
  },

  /**
   * Waits until the detail panel has been dropped — not merely moved off screen. The board keeps
   * the todo rendered for the length of the slide back and unmounts it on the track's own
   * `transitionend`, so this settles only if that movement really ran and really ended.
   */
  async waitForDetailDropped(): Promise<void> {
    let last: BoardView | undefined;
    await waitUntilOr(
      async () => {
        last = await this.view();
        return last.route === "list" && last.detail === null;
      },
      () =>
        `the detail panel was never dropped after returning to the list; last read: ${JSON.stringify(
          last ?? null,
        )}`,
    );
  },

  /** Waits until the row with `id` holds DOM focus — where the board returns focus from a detail. */
  async waitForFocusedRow(id: number): Promise<void> {
    let last: BoardView | undefined;
    await waitUntilOr(
      async () => {
        last = await this.view();
        return last.focusedRow === id;
      },
      () =>
        `focus never landed on todo ${id}; last focused todo: ${last?.focusedRow ?? "none"}`,
    );
  },

  /**
   * Opens the todo titled `title` — the way a user does, by activating its card — and returns the
   * board once its detail panel is on screen.
   *
   * Returns to the list first when another todo is open: while the detail shows, the list panel is
   * translated off the track and `inert`, so its rows are neither visible nor interactive, and that
   * is the same order a user has to move in.
   */
  async open(title: string): Promise<BoardView> {
    const showing = await this.view();
    if (showing.route === "detail") {
      if (showing.detail?.id === (await this.idOf(title))) return showing;
      await this.back();
    }
    const trigger = await this.trigger(title);
    await trigger.waitForClickable({ timeout: WAIT.render });
    await trigger.click();
    return this.waitForDetail(title);
  },

  /**
   * Returns to the list from the open detail panel. Waits only for the route, not for the panel to
   * be dropped: the drop happens a slide later and is its own observable fact, asserted where it is
   * the point rather than folded into every navigation.
   */
  async back(): Promise<void> {
    const back = $(DETAIL).$(BACK);
    await back.waitForClickable({ timeout: WAIT.render });
    await back.click();
    await this.waitForRoute("list");
  },

  /**
   * Brings the list back on screen when the detail panel is showing. Everything that *acts* on a row
   * goes through this first — the panel that is not showing is translated off the track and `inert`,
   * so its rows are neither visible nor clickable, and returning is the order a user moves in too.
   * Reads need no such thing: the off-screen panel stays mounted and stays live.
   */
  async showList(): Promise<void> {
    if ((await this.view()).route === "detail") await this.back();
  },

  /** Activates the row's agent control — the way a user opens the agent that locked it. */
  async openAgent(title: string): Promise<void> {
    await this.showList();
    const control = await this.agentControl(title);
    await control.waitForClickable({ timeout: WAIT.render });
    await control.click();
  },

  /**
   * The list panel's horizontal fit, read in one layout pass: its toolbar and the scroll container
   * the rows live in, each with its content width against its own. A box whose content is wider
   * than itself is one the user would have to scroll sideways. The rows are measured through their
   * scroll container rather than one by one: a truncated title clips on purpose, and counting each
   * row would report the ellipsis as a defect.
   */
  async horizontalOverflow(): Promise<FitReading[]> {
    return measured(
      await browser.execute(() => {
        const fit = (what: string, box: Element | null) => ({
          what,
          scrollWidth: box?.scrollWidth ?? null,
          clientWidth: box?.clientWidth ?? null,
        });
        // The board's own scroll container: the nearest ancestor of a row that actually scrolls,
        // past a group's clipping wrapper (which hides overflow to animate its disclosure).
        let scroller: Element | null = document.querySelector("[data-todo-id]");
        while (scroller && !/^(auto|scroll)$/.test(getComputedStyle(scroller).overflowX)) {
          scroller = scroller.parentElement;
        }
        return [
          fit("toolbar", document.querySelector("[data-todo-toolbar]")),
          fit("rows", scroller),
        ];
      }),
    );
  },

  /**
   * The open detail panel's horizontal fit: its pinned header and the region that scrolls under it.
   * Measured apart from the list because they are different surfaces at the same width — the header
   * carries the whole title beside a status chip and an action cluster that may not wrap, and the
   * regions below sit in a grid whose label column does not shrink.
   */
  async detailOverflow(): Promise<FitReading[]> {
    return measured(
      await browser.execute((detailSel: string) => {
        const detail = document.querySelector(detailSel);
        const fit = (what: string, box: Element | null | undefined) => ({
          what,
          scrollWidth: box?.scrollWidth ?? null,
          clientWidth: box?.clientWidth ?? null,
        });
        // The panel's own scroller — the one region that is meant to move, and only vertically.
        const scroller = detail
          ? [...detail.children].find((child) =>
              /^(auto|scroll)$/.test(getComputedStyle(child).overflowY),
            )
          : null;
        return [
          fit("detail header", detail?.querySelector("header")),
          fit("detail body", scroller),
        ];
      }, DETAIL),
    );
  },

  /** Opens the todo and clicks Complete in its detail panel — the write that routes to the core's gate. */
  async complete(title: string): Promise<void> {
    await this.open(title);
    const button = $(DETAIL).$("button=Complete");
    await button.waitForClickable({ timeout: WAIT.render });
    await button.click();
  },

  /**
   * Opens the todo and waits for the refusal alert in its detail panel, returning its text — the
   * core's verbatim `TodoBlocked` message, surfaced when a blocked todo is completed (the UI never
   * pre-empts it).
   */
  async waitForRefusal(title: string): Promise<string> {
    await this.open(title);
    const alert = $(DETAIL).$('[role="alert"]');
    await alert.waitForDisplayed({ timeout: WAIT.core });
    return alert.getText();
  },

  /**
   * Opens the todo and returns everything its detail panel renders — used to read a comment and its
   * author, which the row never showed and the panel now owns.
   */
  async detailText(title: string): Promise<string> {
    await this.open(title);
    return $(DETAIL).getText();
  },

  /** The `#<id>` text a row carries, read off its exact title. */
  async todoRef(title: string): Promise<string> {
    return (await this.rowElement(title)).$("[data-todo-ref]").getText();
  },

  /** The unmet-blocker text a row carries (`"1 unmet blocker"` / `"<n> unmet blockers"`), or null. */
  async blockerText(title: string): Promise<string | null> {
    const el = (await this.rowElement(title)).$("[data-todo-blockers]");
    if (!(await el.isExisting())) return null;
    return el.getText();
  },

  /**
   * The id the core gave the todo titled `title`, resolved from the exact text of its title (the
   * justified structural handle, as in the sidebar). Every other handle here is then a plain
   * attribute match on that id rather than a text predicate over some element's shape — and it is
   * the same id the detail panel names itself with, so the two surfaces are compared by identity.
   */
  async idOf(title: string): Promise<number> {
    const id = (await this.read(title))?.id;
    if (id === undefined) {
      throw new Error(
        `no todo titled "${title}" is rendered; rendered todos: ${JSON.stringify(
          await this.titles(),
        )}`,
      );
    }
    return id;
  },

  /** The row element itself, addressed by the id the core gave the todo. */
  async rowElement(title: string) {
    return $(`[data-todo-id="${await this.idOf(title)}"]`);
  },

  /** The card's own button — the whole row, which hands the pane to this todo's detail panel. */
  async trigger(title: string) {
    return (await this.rowElement(title)).$("[data-todo-trigger]");
  },

  /**
   * The row's agent control — present only while it is locked. A sibling of the card's button,
   * never a descendant, so activating it never also opens the todo. The detail panel carries its
   * own; this one is deliberately scoped to the row.
   */
  async agentControl(title: string) {
    return (await this.rowElement(title)).$("[data-todo-agent]");
  },
};
