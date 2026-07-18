import { $, browser } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";
import { waitUntilOr } from "../harness/waitUntilOr.js";

// The scratchpad surface: a roster listbox on the left, and a structured editor on the right for the
// open document. Every handle reads what the user reads — the roster's accessible listbox, the
// editor's objective field by its label's accessible name, the revision the editor and roster show,
// and the conflict banner's alert role. Selectors live only here (charter §3.2); the spec asserts on
// the values these return.
const ROSTER = '[role="listbox"][aria-label="Scratchpads"]';
const OPTION = '[role="option"]';
// The objective field. Its accessible name ("Objective") is shared with its own <label>'s text
// span, so `aria/Objective` resolves to the span, not the input (verified: setValue silently no-ops
// on it). The input is instead reached by its stable id suffix — a structural handle the editor sets
// (`id={`${fieldId}-objective`}`), the only "-objective" input on the surface. Not styling-coupled.
const OBJECTIVE = 'input[id$="-objective"]';
const SAVE = "aria/Save";
const RELOAD = "aria/Reload";
const CONFLICT = '[role="alert"]';

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
      (rosterSel: string, optionSel: string) => {
        const roster = document.querySelector(rosterSel);
        if (!roster) return [];
        return [...roster.querySelectorAll(optionSel)].map((option) => {
          // The row's first inner wrapper span holds the name then the `rN` revision, in order.
          const wrapper = option.querySelector("span");
          return {
            name: wrapper?.children[0]?.textContent?.trim() ?? "",
            revision: wrapper?.children[1]?.textContent?.trim() ?? null,
          };
        });
      },
      ROSTER,
      OPTION,
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
   * Opens the roster row named `name` into the editor and waits for the document to load — the Save
   * button appears only once the editor has read the document, so it is the loaded signal. The
   * editor loads at the row's current revision, which the caller reads from the roster.
   */
  async open(name: string): Promise<void> {
    await this.optionElement(name).waitForClickable({ timeout: WAIT.render });
    await this.optionElement(name).click();
    await $(SAVE).waitForDisplayed({ timeout: WAIT.core });
  },

  /** Replaces the objective field's contents. */
  async setObjective(text: string): Promise<void> {
    await $(OBJECTIVE).setValue(text);
  },

  /** The objective field's current value. */
  async objectiveValue(): Promise<string> {
    return $(OBJECTIVE).getValue();
  },

  /**
   * Waits until the objective field settles on `expected` — used after a reload, whose read is
   * asynchronous (the conflict banner clears before the fresh document arrives). Reaching the
   * expected value is the assertion that the reloaded content is what the concurrent writer wrote,
   * with the window's rejected edit gone.
   */
  async waitForObjective(expected: string): Promise<void> {
    let last = "";
    await waitUntilOr(
      async () => {
        last = await this.objectiveValue();
        return last === expected;
      },
      () => `the objective never settled on "${expected}"; last seen: "${last}"`,
    );
  },

  /** Clicks Save on the open editor. */
  async save(): Promise<void> {
    await $(SAVE).click();
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

  /** Reloads the open scratchpad fresh, discarding the window's local edits (the conflict fix). */
  async reload(): Promise<void> {
    await $(RELOAD).click();
    await $(CONFLICT).waitForDisplayed({ reverse: true, timeout: WAIT.core });
  },

  /**
   * The roster row element itself, found by the exact text of its name span — the same justified
   * structural handle the sidebar uses for a process row (no accessible name distinguishes one
   * roster option from another; the name does). Breaks only if the row stops rendering its name.
   */
  optionElement(name: string) {
    return $(ROSTER).$(
      `.//button[@role="option"][.//span[normalize-space(text())="${name}"]]`,
    );
  },
};
