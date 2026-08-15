import { $, browser } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";
import { waitUntilOr } from "../harness/waitUntilOr.js";

// The per-project settings page, shown in the main pane. Its sections are a single-select
// segmented control — a group of radios, each named by the section a user reads.
const SECTIONS = '[role="group"][aria-label="Project settings sections"]';
// The notification-level choices. One control name serves both the project's level and a
// command's, because the sections that hold them are never on screen together.
const LEVELS = '[role="radiogroup"][aria-label="Notify me about"]';
const LEVEL = '[role="radio"]';

/** The pane's sections, by the name each is called in the app. */
export type ProjectSection =
  | "Overview"
  | "Settings"
  | "Notifications"
  | "Commands";

/** The per-project settings pane: choosing a section, and reading or setting a notify-me level. */
export const projectSettingsPane = {
  /**
   * Shows one of the pane's sections, waiting until the control reports it chosen — the control's
   * own state, so the wait cannot pass on a click it dropped.
   */
  async showSection(section: ProjectSection): Promise<void> {
    const tab = $(SECTIONS).$(`button=${section}`);
    await tab.waitForClickable({ timeout: WAIT.render });
    await tab.click();
    await waitUntilOr(
      async () => (await tab.getAttribute("aria-checked")) === "true",
      () => `the "${section}" section never became the chosen one`,
      WAIT.render,
    );
  },

  /**
   * Expands a command's editor, which is where its own notification level lives.
   *
   * Found by the text of its name span: the row's accessible name is its whole line — name,
   * command and storage badges together — so nothing in it names the command alone. Same reason
   * a sidebar row is reached this way.
   */
  async expandCommand(name: string): Promise<void> {
    const row = await $(
      `.//button[@aria-expanded][.//span[normalize-space(text())="${name}"]]`,
    );
    await row.waitForClickable({ timeout: WAIT.render });
    if ((await row.getAttribute("aria-expanded")) === "true") return;
    await row.click();
    await waitUntilOr(
      async () => (await row.getAttribute("aria-expanded")) === "true",
      () => `the "${name}" command's editor never opened`,
      WAIT.render,
    );
  },

  /** The level currently chosen, by the name the user reads on it. */
  async chosenLevel(): Promise<string> {
    const chosen = await this.levels();
    const marked = chosen.filter((level) => level.chosen);
    if (marked.length !== 1) {
      throw new Error(
        `expected exactly one notification level to be chosen, found ${marked.length}; ` +
          `offered: ${JSON.stringify(chosen.map((level) => level.label))}`,
      );
    }
    return (marked[0] as LevelChoice).label;
  },

  /** Chooses the level named `label`, clicking the radio the way a user does. */
  async chooseLevel(label: string): Promise<void> {
    const levels = await this.levels();
    const wanted = levels.find((level) => level.label === label);
    if (wanted === undefined) {
      throw new Error(
        `no notification level named ${JSON.stringify(label)} is offered; ` +
          `offered: ${JSON.stringify(levels.map((level) => level.label))}`,
      );
    }
    const radio = await $(`[id="${wanted.id}"]`);
    await radio.waitForClickable({ timeout: WAIT.render });
    await radio.click();
    await waitUntilOr(
      async () => (await this.chosenLevel()) === label,
      async () =>
        `choosing "${label}" left "${await this.chosenLevel()}" chosen`,
      WAIT.core,
    );
  },

  /**
   * Every level the control offers, read in one pass — each radio's own id, the name assistive
   * tech announces for it, and whether it is the chosen one. The page reloads from the core after
   * every change, so the control re-renders and a read of it has to be atomic.
   */
  async levels(): Promise<LevelChoice[]> {
    const choices: LevelChoice[] | null = await browser.execute(
      (group: string, item: string) => {
        const root = document.querySelector(group);
        if (!root) return null;
        return [...root.querySelectorAll(item)].map((node) => {
          const labelled = node.getAttribute("aria-labelledby");
          const label =
            labelled === null ? null : document.getElementById(labelled);
          return {
            id: node.id,
            label: label?.textContent?.trim() ?? "",
            chosen: node.getAttribute("aria-checked") === "true",
          };
        });
      },
      LEVELS,
      LEVEL,
    );
    if (choices === null) {
      throw new Error("no notification-level control is on screen");
    }
    return choices;
  },
};

/** One level the control offers, as the pane renders it. */
interface LevelChoice {
  id: string;
  label: string;
  chosen: boolean;
}
