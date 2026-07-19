import { $, browser } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";
import { waitUntilOr } from "../harness/waitUntilOr.js";

// The Templates section of Settings: a browse list of both libraries that drills into an editor for
// one template — its description, its body, and, for a prompt, a preview carrying one value field
// per placeholder the core declared.
//
// Every handle is what the user reads or a structural marker the component already carries: a browse
// row is the button showing the template's name, a value field is addressed by the accessible name
// the preview gives it, and the autosave status and Save control are the editor's own data
// attributes. Everything is scoped to the settings tab panel, because the section rail carries a
// "Templates" button of its own that an unscoped text lookup would reach first.
const PANEL = '[role="tabpanel"]';
// Present only in the drill-in editor, so it is the signal that one template is open.
const EDITOR_MARKER = 'button[aria-label="Delete template"]';
const BACK = "button=Templates";
const DESCRIPTION = 'input[aria-label="Template description"]';
const SAVE = "button[data-template-save]";
const AUTOSAVE_STATUS = "[data-autosave-status]";
// The autosave read-out at rest — the editor has no unsaved changes and none in flight.
const SAVED = "Saved";
// The preview section is reached from its heading rather than from a container class: the editor's
// conflict banner is an advisory strip of exactly the same kind, and an unscoped alert read would
// count it as a placeholder notice.
const PREVIEW_TITLE = "Preview";
const RENDERED = "pre";
const NOTICE = '[role="alert"]';
// How the preview names each placeholder's value field (its `aria-label`), used both to address one
// field and to recover the declared placeholder list in rendered order.
const VALUE_LABEL_PREFIX = "Value for ";

/** What the preview shows for one render: the prompt it produced, and what it reports about it. */
export interface Preview {
  /** The rendered prompt, or `null` while the section shows no output at all. */
  prompt: string | null;
  /** The advisories above it, in order — unanswered placeholders, then unmatched values. */
  notices: string[];
}

/** The Templates panel: the browse list, and the editor for one open template. */
export const templatesPanel = {
  /**
   * Opens the template named `name` from the browse list and waits for its editor. The list is a
   * core read, so reaching the row is a round trip; the editor's own marker is what proves the
   * drill-in rendered rather than the click merely landing.
   */
  async openTemplate(name: string): Promise<void> {
    const row = $(PANEL).$(`button*=${name}`);
    await row.waitForClickable({ timeout: WAIT.core });
    await row.click();
    await $(PANEL).$(EDITOR_MARKER).waitForDisplayed({ timeout: WAIT.core });
  },

  /** Leaves the open editor for the browse list, waiting until the editor is gone. */
  async back(): Promise<void> {
    const back = $(PANEL).$(BACK);
    await back.waitForClickable({ timeout: WAIT.render });
    await back.click();
    await $(PANEL).$(EDITOR_MARKER).waitForDisplayed({ timeout: WAIT.render, reverse: true });
  },

  /**
   * The placeholders the open prompt template offers a value for, in the order they are rendered —
   * which is the order the core derived them from the body, never one the window worked out.
   *
   * Read in one pass: every keystroke in a value field re-renders the preview, so walking the fields
   * one driver call at a time races that re-render and dies on a stale element reference.
   */
  async placeholders(): Promise<string[]> {
    return browser.execute((prefix: string) => {
      const fields = document.querySelectorAll(`input[aria-label^="${prefix}"]`);
      return [...fields].map((field) =>
        (field.getAttribute("aria-label") ?? "").slice(prefix.length),
      );
    }, VALUE_LABEL_PREFIX);
  },

  /** Types `value` into the value field for `placeholder`, replacing whatever it held. */
  async fill(placeholder: string, value: string): Promise<void> {
    const field = this.valueField(placeholder);
    await field.waitForClickable({ timeout: WAIT.render });
    await field.setValue(value);
  },

  /** Empties the value field for `placeholder` — not an answer of "", but no answer at all. */
  async clearValue(placeholder: string): Promise<void> {
    const field = this.valueField(placeholder);
    await field.waitForClickable({ timeout: WAIT.render });
    await field.clearValue();
  },

  /**
   * Waits until the rendered prompt settles on `expected`. Each keystroke sends its own render to
   * the core and the results land as they resolve, so the text is polled to its settled value rather
   * than read once.
   */
  async waitForPreview(expected: string): Promise<Preview> {
    let last: Preview = { prompt: null, notices: [] };
    await waitUntilOr(
      async () => {
        last = await this.readPreview();
        return last.prompt === expected;
      },
      () => `the preview never rendered "${expected}"; last seen: ${JSON.stringify(last.prompt)}`,
    );
    return last;
  },

  /**
   * The preview as it stands: the rendered prompt and the advisories over it, read in one pass.
   *
   * One pass because the two are one render's report of itself — reading them separately could
   * pair a prompt with the notices of the keystroke before it. In-page `textContent` rather than
   * the driver's text command, which reports what is *rendered* and answers an empty string for
   * output it does not consider laid out (the same trap the sidebar's telemetry read documents).
   * A `null` prompt means the section showed no output at all, which is a different failure from
   * showing the wrong one.
   */
  async readPreview(): Promise<Preview> {
    return browser.execute(
      (heading: string, rendered: string, notice: string) => {
        const section = [...document.querySelectorAll("h3")]
          .find((title) => title.textContent?.trim() === heading)
          ?.closest("section");
        if (!section) return { prompt: null, notices: [] };
        return {
          prompt: section.querySelector(rendered)?.textContent ?? null,
          notices: [...section.querySelectorAll(notice)].map(
            (strip) => strip.textContent?.trim() ?? "",
          ),
        };
      },
      PREVIEW_TITLE,
      RENDERED,
      NOTICE,
    );
  },

  /** The description the open template loaded with, as its field shows it. */
  async description(): Promise<string> {
    return this.descriptionField().getValue();
  },

  /** Empties the description field — the edit whose round trip the walk follows. */
  async clearDescription(): Promise<void> {
    const field = this.descriptionField();
    await field.waitForClickable({ timeout: WAIT.render });
    await field.clearValue();
  },

  /**
   * Saves the open template and waits for the autosave read-out to report it at rest. Clicking Save
   * rather than leaning on the debounce keeps the write deterministic; the read-out settling is what
   * proves the core accepted it, since a refused save leaves the editor dirty.
   */
  async save(): Promise<void> {
    const save = $(PANEL).$(SAVE);
    await save.waitForClickable({ timeout: WAIT.render });
    await save.click();
    const status = $(PANEL).$(AUTOSAVE_STATUS);
    await status.waitUntil(async () => (await status.getText()) === SAVED, {
      timeout: WAIT.core,
      timeoutMsg: "the editor never reported the save at rest",
    });
  },

  /** One placeholder's value field, by the accessible name the preview gives it. */
  valueField(placeholder: string) {
    return $(PANEL).$(`input[aria-label="${VALUE_LABEL_PREFIX}${placeholder}"]`);
  },

  /** The open editor's description field. */
  descriptionField() {
    return $(PANEL).$(DESCRIPTION);
  },
};
