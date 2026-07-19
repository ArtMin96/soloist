import { $, browser } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";
import { waitUntilOr } from "../harness/waitUntilOr.js";

// The scratchpad surface: a roster listbox on the left, and a rich-text editor on the right for the
// open document. Every handle reads what the user reads or a stable structural marker — the roster's
// accessible listbox (whose rows are addressed by the raw name handle they carry, the visible title
// being that handle's humanized reading), the editor's contenteditable body by the stable
// `data-editor` handle, the revision the editor and roster show, and the conflict banner's alert
// role. Selectors live only here (charter §3.2); the
// spec asserts on the values these return. The editor is a `contenteditable`, so an edit is driven
// as a deterministic toolbar toggle and the save is flushed with Ctrl+S. WebKitGTK/WebDriver does
// not deliver the `beforeinput`/text events ProseMirror needs to insert typed characters, so the
// edit is made by clicking a formatting control (a real mouse interaction, which does land) rather
// than by typing — and the autosave debounce is the backstop should the Ctrl+S keydown be dropped.
const ROSTER = '[role="listbox"][aria-label="Scratchpads"]';
const OPTION = '[role="option"]';
// The raw name handle a roster row addresses. The row *reads* as a humanized title (a slug handle is
// shown as prose), so the handle the core and the lead agent both name the document by is carried as
// a stable structural attribute rather than being recovered from the displayed text.
const NAME_ATTR = "data-scratchpad-name";
// The editable rich-text region — a ProseMirror contenteditable, marked with a stable structural
// handle the editor sets (`data-editor="rich-text"`), not styling-coupled.
const BODY = '[data-editor="rich-text"]';
// The Heading-1 formatting control in the editor toolbar. Clicking it toggles the caret's block to a
// heading — a document change autosave marks dirty — without relying on dropped keystrokes.
const TOOLBAR_H1 = 'button[aria-label="Heading 1"]';
const RELOAD = "aria/Reload";
const CONFLICT = '[role="alert"]';

// The W3C WebDriver code point for the left Control key (WebdriverIO's `Key.Control`), held as the
// modifier of a chord when it leads a `keys` array. Used to press Ctrl+S — the editor's deterministic
// save flush. Even if the chord were dropped, the autosave debounce is the backstop that still saves.
const CONTROL = "\uE009";

/** One scratchpad as the roster summarises it. */
interface RosterRow {
  name: string;
  revision: number;
}

/** Parses the roster's `rN` revision read-out to its number, or `null` when it is not present. */
function parseRosterRevision(text: string | null): number | null {
  const match = text === null ? null : /^r(\d+)$/.exec(text.trim());
  return match === null ? null : Number(match[1]);
}

/** Parses the "revision N" the conflict banner names to its number. */
function parseRevision(text: string): number | null {
  const match = /revision (\d+)/.exec(text);
  return match === null ? null : Number(match[1]);
}

/** The scratchpad panel: the roster and the open document's editor. */
export const scratchpadPanel = {
  /** Waits for the roster to render — the pane has switched to the scratchpads view. */
  async waitForRoster(): Promise<void> {
    await $(ROSTER).waitForDisplayed({ timeout: WAIT.core });
  },

  /**
   * Every roster row currently rendered, read in one pass.
   *
   * Read atomically rather than row-by-row: the roster re-renders on every `ScratchpadChanged`
   * (a concurrent write bumps a revision), so walking the rows one driver call at a time races the
   * re-render and dies on a stale element reference.
   */
  async rows(): Promise<RosterRow[]> {
    const raw: { name: string; revision: string | null }[] = await browser.execute(
      (rosterSel: string, optionSel: string, nameAttr: string) => {
        const roster = document.querySelector(rosterSel);
        if (!roster) return [];
        return [...roster.querySelectorAll(optionSel)].map((option) => {
          // The row's first inner wrapper span holds the title then the `rN` revision, in order;
          // the name is the handle attribute, since the title is the humanized reading of it.
          const wrapper = option.querySelector("span");
          return {
            name: option.getAttribute(nameAttr) ?? "",
            revision: wrapper?.children[1]?.textContent?.trim() ?? null,
          };
        });
      },
      ROSTER,
      OPTION,
      NAME_ATTR,
    );
    return raw.map(({ name, revision }) => ({
      name,
      revision: parseRosterRevision(revision) ?? Number.NaN,
    }));
  },

  /** Waits until a roster row named `name` is rendered, then returns its revision. */
  async waitForRow(name: string): Promise<number> {
    let row: RosterRow | undefined;
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
    return (row as RosterRow).revision;
  },

  /**
   * Waits until the roster row named `name` shows a revision other than `previous` — a concurrent
   * write landing — then returns it. The roster refreshes on `ScratchpadChanged`, so this is a core
   * round trip.
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
   * Opens the roster row named `name` into the editor and waits for the document to load — the
   * rich-text body appears only once the editor has read the document and its lazy chunk has
   * mounted, so it is the loaded signal. The editor loads at the row's current revision, which the
   * caller reads from the roster.
   */
  async open(name: string): Promise<void> {
    await this.optionElement(name).waitForClickable({ timeout: WAIT.render });
    await this.optionElement(name).click();
    await $(BODY).waitForDisplayed({ timeout: WAIT.core });
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
    await $(BODY).waitForClickable({ timeout: WAIT.render });
    await $(BODY).click();
    await $(TOOLBAR_H1).waitForClickable({ timeout: WAIT.render });
    await $(TOOLBAR_H1).click();
    await $(BODY).$("h1").waitForDisplayed({ timeout: WAIT.render });
  },

  /**
   * The open editor's rendered body text — what the contenteditable shows, with block structure
   * flattened to text. Used after a reload to read the concurrent writer's content back.
   */
  async bodyText(): Promise<string> {
    return $(BODY).getText();
  },

  /**
   * Waits until the editor body settles on `expected` — used after a reload, whose read is
   * asynchronous (the conflict banner clears before the fresh document arrives). Reaching the
   * expected content is the assertion that the reloaded body is what the concurrent writer wrote,
   * with the window's rejected edit gone.
   */
  async waitForBody(expected: string): Promise<void> {
    let last = "";
    await waitUntilOr(
      async () => {
        last = await this.bodyText();
        // The remounted editor re-serializes the body, which may add block whitespace around the
        // prose; a containment check reads the concurrent writer's content without coupling to it.
        return last.includes(expected);
      },
      () => `the editor body never settled on "${expected}"; last seen: "${last}"`,
    );
  },

  /**
   * Flushes the pending edit deterministically with Ctrl+S — the editor intercepts it and saves
   * immediately, so the save never depends on the autosave debounce timing. The stale write is what
   * the core refuses, raising the conflict.
   */
  async save(): Promise<void> {
    await $(BODY).click();
    await browser.keys([CONTROL, "s"]);
  },

  /**
   * Waits for the stale-write conflict banner and returns the revision it names — the revision the
   * scratchpad now sits at, which only the concurrent writer produced. Its presence is the guard
   * firing: the core refused the window's stale save.
   */
  async waitForConflictRevision(): Promise<number> {
    await $(CONFLICT).waitForDisplayed({ timeout: WAIT.core });
    const text = await $(CONFLICT).getText();
    const revision = parseRevision(text);
    if (revision === null) {
      throw new Error(`the conflict banner named no revision; its text was "${text}"`);
    }
    return revision;
  },

  /**
   * Reloads the open scratchpad fresh, discarding the window's local edits (the conflict fix).
   * WebKitGTK/WebDriver can drop a single click under parallel load, so it re-clicks Reload until the
   * conflict banner clears — the same retry the pane-open uses for a dropped menu keystroke.
   */
  async reload(): Promise<void> {
    const conflict = $(CONFLICT);
    await browser.waitUntil(
      async () => {
        if (!(await conflict.isDisplayed())) return true;
        await $(RELOAD).click();
        return !(await conflict.isDisplayed());
      },
      { timeout: WAIT.core, timeoutMsg: "the conflict banner never cleared after Reload" },
    );
  },

  /**
   * The roster row element itself, found by the raw name handle it carries. The row's visible title
   * is the humanized reading of that handle, so the handle — the name the core, the `solo://` link,
   * and the lead agent all use — is the stable way to address one row. Breaks only if the row stops
   * carrying its handle.
   */
  optionElement(name: string) {
    return $(ROSTER).$(`button[role="option"][${NAME_ATTR}="${name}"]`);
  },
};
