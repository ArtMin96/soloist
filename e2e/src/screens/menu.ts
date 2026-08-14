import { $, $$, browser } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";

// Any menu the window has mounted, including one still on screen while it closes.
const MENU = '[role="menu"]';

// The menu that is actually open — not the same set. `data-state` flips to "closed" on the frame a
// menu starts closing, while the content stays mounted and reads as *displayed* for the whole exit
// animation, down to and including `opacity: 0`, before it is removed. Reading visibility instead
// therefore counts a menu that is already leaving as one that can be chosen from.
const OPEN_MENU = `${MENU}[data-state="open"]`;

/**
 * Opens a menu the way a keyboard user does — focus its trigger, press Enter — and chooses the item
 * carrying `label`. `menu` names it in the failure message, and `trigger` is a lookup rather than an
 * element so that nothing is held across a wait.
 *
 * Under WebKitGTK the synthetic pointer neither reliably triggers the `:hover` that reveals a row's
 * trigger nor fires the `pointerdown` a menu opens on, so focus is set outright — the row's
 * `:focus-within` answers it — and the menu is opened from the keyboard. That press is racy: one
 * dispatched before the synthetic focus settles is dropped and no menu appears, and the suite runs
 * with no retries, so a single dropped press would take a whole spec file.
 *
 * The whole handshake repeats, though, not only the press — because a menu can also *leave* under
 * the harness, closed by whatever the window is doing as the app's own events land. An attempt that
 * finds it gone starts over, where committing to the element found during the wait fails the spec
 * outright on the stale lookup that follows. A press lands only while no menu is mounted at all, so
 * an attempt can never press into one that is already up; and the item is clicked once, outside the
 * retry, so a repeat can never run an action twice.
 */
export async function chooseFromMenu(
  menu: string,
  trigger: () => ReturnType<typeof $>,
  label: string,
): Promise<void> {
  await trigger().waitForExist({ timeout: WAIT.render });

  const item = await browser.waitUntil(
    async () => {
      const [open] = await $$(OPEN_MENU);
      if (!open) {
        // A menu still mounted is one closing; let it go rather than pressing across it.
        if (await $(MENU).isExisting()) return false;
        await browser.execute(
          (element: HTMLElement) => element.focus(),
          await trigger(),
        );
        await browser.keys("Enter");
        return false;
      }

      // Scoped to the open menu rather than looked up globally: a pane an item opens can carry the
      // same name once rendered (the orchestration one names its own view switch "Orchestration
      // views"), so a global lookup could mis-target on a re-open. Exact text keeps the match on the
      // one menu item — the menu's wrapper holds every label's text, so it can never match exactly.
      const [candidate] = await open.$$(`div=${label}`);
      if (!candidate || !(await candidate.isClickable())) return false;
      return candidate;
    },
    {
      timeout: WAIT.core,
      timeoutMsg: `never reached "${label}" in the ${menu} menu`,
    },
  );

  await item.click();
}
