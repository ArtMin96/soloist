import { $, browser } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";
import { waitUntilOr } from "../harness/waitUntilOr.js";

// The scratchpad board: the project's shared notes as a list of cards, and the detail panel one card
// hands the whole pane over to. Selectors live only here, addressed by each document's own
// `data-scratchpad-*` handles and by the board handles it shares with the to-do board
// (`data-panel-route`, `data-card-trigger`, `data-detail-back`, `data-detail-done`) — so neither
// structure nor styling can move a handle out from under this file.
//
// A row is addressed by the raw name handle its `<li>` carries. The card *reads* as a humanized
// title (a slug handle is shown as prose), so the handle the core, the `solo://` link and the lead
// agent all name the document by is the stable way to find one.
//
// **Both panels are mounted at all times** — the one not showing is translated off the track and
// made `inert`, never unmounted — so the existence of a panel proves nothing about which one the
// user is on. `data-panel-route` on the viewport is the only honest read of that, and every detail
// handle here is scoped under `[data-scratchpad-detail]`, because `inert` does not remove a node
// from `querySelectorAll` and the card and the detail header deliberately wear the same meta chips
// (the revision among them).
//
// **Reading mounts the same editor as writing.** The rendered body is the rich-text editor held
// read-only, so `[data-editor="rich-text"]` is on screen in *both* modes: waiting for it would let
// `startEdit` return before anything is editable. The editable surface's own proof is the Done
// control, which exists only while the document is being written to.
//
// That editor is a `contenteditable`, so an edit is driven as a deterministic toolbar toggle and the
// save is flushed with Ctrl+S. WebKitGTK/WebDriver does not deliver the `beforeinput`/text events
// ProseMirror needs to insert typed characters, so the edit is made by clicking a formatting control
// (a real mouse interaction, which does land) rather than by typing — and the autosave debounce is
// the backstop should the Ctrl+S keydown be dropped.

/** The attribute the board's transition viewport names the panel currently on screen with. */
const ROUTE_ATTR = "data-panel-route";
/** The row handle each card's `<li>` carries: the durable id the core gave the document. */
const ROW_ATTR = "data-scratchpad-id";
/**
 * The raw name handle a row carries. The card *reads* as a humanized title, so the handle the core
 * and the lead agent both name the document by is carried as a stable structural attribute rather
 * than being recovered from the displayed text.
 */
const NAME_ATTR = "data-scratchpad-name";
/** The revision chip a card and the detail header both wear; its text reads `rN`. */
const REVISION_ATTR = "data-scratchpad-revision";
/**
 * The detail panel's root, valued with the open scratchpad's name. Unlike the panel slot it sits in,
 * the root exists only while a scratchpad is open, and it names *which* — so it settles both
 * questions the route alone cannot.
 */
const DETAIL_ATTR = "data-scratchpad-detail";
const DETAIL = `[${DETAIL_ATTR}]`;
/** The detail's return control — and where the board parks focus when it opens the panel. */
const BACK = "[data-detail-back]";
/** The control that leaves edit mode: present only while the editable surface is mounted. */
const DONE = "[data-detail-done]";
/** The card's own button — the whole row, which hands the pane to this scratchpad's detail. */
const TRIGGER = "[data-card-trigger]";
/**
 * The board's own landmark: the search field is named for the documents this board holds, so it
 * settles only once the pane really switched to the scratchpads board. The toolbar's structural
 * handle is shared with the to-do board and would settle on that one too.
 */
const SEARCH = "aria/Search scratchpads";
/** The control that switches the open document from reading to writing. */
const EDIT = "aria/Edit";
/**
 * The rich-text region. Marked with a stable structural handle the editor sets, not a styling-coupled
 * one — and present in both modes, which is why `startEdit` waits on `DONE` instead.
 */
const BODY = '[data-editor="rich-text"]';
/** The Heading-1 formatting control in the editor toolbar. Clicking it toggles the caret's block to a
 *  heading — a document change autosave marks dirty — without relying on dropped keystrokes. */
const TOOLBAR_H1 = 'button[aria-label="Heading 1"]';
const RELOAD = "aria/Reload";
/** The advisory strip a refused revision-guarded save raises, with its Reload beside it. */
const CONFLICT = "[data-advisory-notice]";

// The W3C WebDriver code point for the left Control key (WebdriverIO's `Key.Control`), held as the
// modifier of a chord when it leads a `keys` array. Used to press Ctrl+S — the editor's deterministic
// save flush. Even if the chord were dropped, the autosave debounce is the backstop that still saves.
const CONTROL = "\uE009";

/** Which of the board's two panels the user is on. */
type BoardRoute = "list" | "detail";

/** One scratchpad as its card summarises it. */
interface BoardRow {
  name: string;
  /** The revision its chip reads, or `NaN` when the card rendered no chip at all. */
  revision: number;
}

/** What the open detail panel says about the scratchpad it is showing. */
interface ScratchpadDetailState {
  /** The scratchpad the panel is open on, as the panel's own handle names it. */
  name: string;
  /** The revision its header rail reads — carried for the failure message, not asserted. */
  revision: number;
}

/**
 * The board as one atomic read: which panel is showing, what the detail panel holds, and where
 * focus is. Read in a single pass rather than one query at a time — a route change moves focus and
 * swaps two panels' `inert` state in the same commit, so reading those separately can catch the
 * board mid-swap and report a state it was never actually in.
 */
interface BoardView {
  /** The panel on screen, or `null` when the board is not rendered at all. */
  route: BoardRoute | null;
  /** The scratchpad the detail panel holds, or `null` once the panel has been dropped. */
  detail: ScratchpadDetailState | null;
  /** Whether DOM focus is on the detail's Back control. */
  backFocused: boolean;
  /** The name handle of the row that holds DOM focus, or `null` when no row does. */
  focusedRow: string | null;
}

/**
 * Parses a `rN` revision chip. `NaN` when there is no chip to read, which callers refuse rather than
 * treat as a revision: a marker that vanished must never read as "unchanged", or a card whose meta
 * rail was restructured would be reported as a scratchpad nobody wrote to.
 */
function parseChipRevision(text: string | null): number {
  const match = text === null ? null : /^r(\d+)$/.exec(text.trim());
  return match === null ? Number.NaN : Number(match[1]);
}

/** Parses the "revision N" the conflict banner names to its number. */
function parseNoticeRevision(text: string): number | null {
  const match = /revision (\d+)/.exec(text);
  return match === null ? null : Number(match[1]);
}

/** One card's own button, as a single selector — chained queries cost a driver round trip each. */
function rowTrigger(name: string): string {
  return `[${ROW_ATTR}][${NAME_ATTR}="${name}"] ${TRIGGER}`;
}

/** The scratchpad board: its cards, and the detail panel one of them opens. */
export const scratchpadBoard = {
  /** Waits for the board to render — the pane has switched to the scratchpads view. */
  async waitForBoard(): Promise<void> {
    await $(SEARCH).waitForDisplayed({ timeout: WAIT.core });
  },

  /**
   * Every card currently rendered, read in one pass.
   *
   * Read atomically rather than row-by-row: the list re-renders on every `ScratchpadChanged` (a
   * concurrent write bumps a revision), so walking the cards one driver call at a time races the
   * re-render and dies on a stale element reference.
   */
  async rows(): Promise<BoardRow[]> {
    const raw: { name: string; revision: string | null }[] = await browser.execute(
      (rowAttr: string, nameAttr: string, revisionAttr: string) =>
        [...document.querySelectorAll(`[${rowAttr}]`)].map((row) => ({
          name: row.getAttribute(nameAttr) ?? "",
          revision:
            row.querySelector(`[${revisionAttr}]`)?.textContent?.trim() ?? null,
        })),
      ROW_ATTR,
      NAME_ATTR,
      REVISION_ATTR,
    );
    return raw.map(({ name, revision }) => ({
      name,
      revision: parseChipRevision(revision),
    }));
  },

  /** Waits until a card named `name` is rendered, then returns the revision it reads. */
  async waitForRow(name: string): Promise<number> {
    let row: BoardRow | undefined;
    let seen: string[] = [];
    await waitUntilOr(
      async () => {
        const rows = await this.rows();
        seen = rows.map((candidate) => candidate.name);
        row = rows.find((candidate) => candidate.name === name);
        return row !== undefined;
      },
      () => `no scratchpad row named "${name}" appeared; rendered rows: ${JSON.stringify(seen)}`,
    );
    const revision = (row as BoardRow).revision;
    if (Number.isNaN(revision)) {
      throw new Error(
        `scratchpad row "${name}" rendered no revision chip — the card's meta rail moved, so its ` +
          `revision is unknown rather than unchanged`,
      );
    }
    return revision;
  },

  /**
   * Waits until the card named `name` shows a revision other than `previous` — a concurrent write
   * landing — then returns it. The list refreshes on `ScratchpadChanged`, so this is a core round
   * trip. A card that stopped rendering its revision reads `NaN` and is refused rather than counted
   * as a change.
   */
  async waitForRevisionChange(name: string, previous: number): Promise<number> {
    let revision = previous;
    await waitUntilOr(
      async () => {
        const row = (await this.rows()).find((candidate) => candidate.name === name);
        revision = row?.revision ?? previous;
        return revision !== previous && !Number.isNaN(revision);
      },
      () => `scratchpad "${name}" never moved off revision ${previous}; last seen: r${revision}`,
    );
    return revision;
  },

  /**
   * The board's route, its detail panel and its focus, in one pass. Everything here is read from
   * what the engine actually settled on — the viewport's own route attribute and
   * `document.activeElement` — never from a component's idea of where it put things.
   */
  async view(): Promise<BoardView> {
    const raw = await browser.execute(
      (
        routeAttr: string,
        detailAttr: string,
        revisionAttr: string,
        rowAttr: string,
        nameAttr: string,
        backSel: string,
      ) => {
        const viewport = document.querySelector(`[${routeAttr}]`);
        const detail = document.querySelector(`[${detailAttr}]`);
        const active = document.activeElement;
        return {
          route: (viewport?.getAttribute(routeAttr) ?? null) as "list" | "detail" | null,
          detail:
            detail === null
              ? null
              : {
                  name: detail.getAttribute(detailAttr) ?? "",
                  revision:
                    detail.querySelector(`[${revisionAttr}]`)?.textContent?.trim() ?? null,
                },
          backFocused: active !== null && detail?.querySelector(backSel) === active,
          focusedRow: active?.closest(`[${rowAttr}]`)?.getAttribute(nameAttr) ?? null,
        };
      },
      ROUTE_ATTR,
      DETAIL_ATTR,
      REVISION_ATTR,
      ROW_ATTR,
      NAME_ATTR,
      BACK,
    );
    return {
      ...raw,
      detail:
        raw.detail === null
          ? null
          : { name: raw.detail.name, revision: parseChipRevision(raw.detail.revision) },
    };
  },

  /** Waits until the board is on `route`. */
  async waitForRoute(route: BoardRoute): Promise<void> {
    let last: BoardView | undefined;
    await waitUntilOr(
      async () => {
        last = await this.view();
        return last.route === route;
      },
      () => `the board never showed its ${route} panel; last read: ${JSON.stringify(last ?? null)}`,
      WAIT.render,
    );
  },

  /**
   * Waits until the detail panel is showing the scratchpad named `name`, and returns the board as
   * read at that moment — including where focus ended up, which the caller asserts.
   *
   * Focus is not part of the condition on purpose: the board opens the panel and moves focus into
   * it in one layout effect, so a snapshot that sees the panel has already seen wherever focus
   * settled. Waiting on focus too would turn a focus that never moved into a timeout instead of the
   * failed assertion it is.
   */
  async waitForDetail(name: string): Promise<BoardView> {
    let last: BoardView | undefined;
    await waitUntilOr(
      async () => {
        last = await this.view();
        return last.route === "detail" && last.detail?.name === name;
      },
      () =>
        `the detail panel never opened on "${name}"; last read: ${JSON.stringify(last ?? null)}`,
    );
    return last as BoardView;
  },

  /**
   * Opens the scratchpad named `name` — the way a user does, by activating its card — and returns
   * the board once its detail panel is on screen. The panel lands in reading; `startEdit` is what
   * opens the editor.
   *
   * Returns to the list first when another scratchpad is open: while the detail shows, the list
   * panel is translated off the track and `inert`, so its cards are neither visible nor
   * interactive, and that is the same order a user has to move in.
   */
  async open(name: string): Promise<BoardView> {
    const showing = await this.view();
    if (showing.route === "detail") {
      if (showing.detail?.name === name) return showing;
      await this.back();
    }
    const trigger = $(rowTrigger(name));
    await trigger.waitForClickable({ timeout: WAIT.render });
    await trigger.click();
    return this.waitForDetail(name);
  },

  /**
   * Returns to the list from the open detail panel. Waits only for the route: the panel is dropped a
   * slide later, which is its own observable fact.
   */
  async back(): Promise<void> {
    const back = $(DETAIL).$(BACK);
    await back.waitForClickable({ timeout: WAIT.render });
    await back.click();
    await this.waitForRoute("list");
  },

  /**
   * Switches the open detail panel from reading to writing, returning once the editable surface is
   * live — proven by the Done control, which exists only while editing. Waiting for the body would
   * settle immediately and wrongly: reading mounts the same editor held read-only.
   *
   * The Edit control is disabled until the body read lands, so waiting for it to become clickable is
   * also the wait for the document itself.
   */
  async startEdit(): Promise<void> {
    const edit = $(DETAIL).$(EDIT);
    await edit.waitForClickable({ timeout: WAIT.core });
    await edit.click();
    await $(DETAIL).$(DONE).waitForDisplayed({ timeout: WAIT.render });
  },

  /**
   * Makes a real edit in the open editor by toggling the first block to a heading from the toolbar —
   * a document change the editor emits and autosave marks dirty. It is driven by a mouse click, not
   * typed text, because WebKitGTK/WebDriver does not deliver the input events ProseMirror needs to
   * insert characters. Focusing the body first places the caret the toggle acts on. It confirms the
   * edit by the heading the toggle produced — a persistent structural change — rather than the
   * fleeting "unsaved" status, which autosave clears within ~1 s (by then raising the conflict).
   */
  async edit(): Promise<void> {
    const body = $(DETAIL).$(BODY);
    await body.waitForClickable({ timeout: WAIT.render });
    await body.click();
    const heading = $(DETAIL).$(TOOLBAR_H1);
    await heading.waitForClickable({ timeout: WAIT.render });
    await heading.click();
    await $(DETAIL).$(BODY).$("h1").waitForDisplayed({ timeout: WAIT.render });
  },

  /**
   * Flushes the pending edit deterministically with Ctrl+S — the editor intercepts it and saves
   * immediately, so the save never depends on the autosave debounce timing. The stale write is what
   * the core refuses, raising the conflict.
   */
  async save(): Promise<void> {
    await $(DETAIL).$(BODY).click();
    await browser.keys([CONTROL, "s"]);
  },

  /**
   * Waits for the stale-write conflict advisory and returns the revision it names — the revision the
   * scratchpad now sits at, which only the concurrent writer produced. Its presence is the guard
   * firing: the core refused the window's stale save.
   */
  async waitForConflictRevision(): Promise<number> {
    const notice = $(DETAIL).$(CONFLICT);
    await notice.waitForDisplayed({ timeout: WAIT.core });
    const text = await notice.getText();
    const revision = parseNoticeRevision(text);
    if (revision === null) {
      throw new Error(`the conflict banner named no revision; its text was "${text}"`);
    }
    return revision;
  },

  /**
   * Reloads the open scratchpad fresh, discarding the window's local edits (the conflict fix).
   * WebKitGTK/WebDriver can drop a single click under parallel load, so it re-clicks Reload until the
   * conflict advisory clears — the same retry the pane-open uses for a dropped menu keystroke.
   */
  async reload(): Promise<void> {
    const conflict = $(DETAIL).$(CONFLICT);
    await browser.waitUntil(
      async () => {
        if (!(await conflict.isDisplayed())) return true;
        await $(DETAIL).$(RELOAD).click();
        return !(await conflict.isDisplayed());
      },
      {
        timeout: WAIT.core,
        timeoutMsg: "the conflict banner never cleared after Reload",
      },
    );
  },

  /**
   * Waits until the open document's body settles on `expected` — used after a reload, whose read is
   * asynchronous (the conflict advisory clears before the fresh document arrives). Reaching the
   * expected content is the assertion that the reloaded body is what the concurrent writer wrote,
   * with the window's rejected edit gone.
   */
  async waitForBody(expected: string): Promise<void> {
    let last = "";
    await waitUntilOr(
      async () => {
        // The remounted editor re-serializes the body, which may add block whitespace around the
        // prose; a containment check reads the concurrent writer's content without coupling to it.
        last = await $(DETAIL).$(BODY).getText();
        return last.includes(expected);
      },
      () => `the document body never settled on "${expected}"; last seen: "${last}"`,
    );
  },
};
