import { waitUntilOr } from "../harness/waitUntilOr.js";
import {
  agentEntry,
  launchPicker,
  TERMINAL_ENTRY,
} from "../screens/LaunchPicker.js";
import { sidebar } from "../screens/Sidebar.js";
import { startSurface } from "../screens/StartSurface.js";
import type { RowHandle } from "../screens/Sidebar.js";

/** The label the core gives a project's first terminal — what its sidebar row and pane read. */
export const TERMINAL_LABEL = "Terminal";

/**
 * Launches an agent the way a user does: open the picker from the start surface, pick the tool, and wait
 * for its row to appear. Returns the row, so a spec can assert on what was rendered.
 *
 * With exactly one project open the picker targets it and goes straight to the entry list; with
 * several it asks which project first, so a spec that opens more than one must drive that step.
 */
export async function launchAgent(tool: string): Promise<RowHandle> {
  await openLaunchPicker();
  await launchPicker.choose(agentEntry(tool));
  await launchPicker.waitUntilClosed();
  return sidebar.waitForRow(tool);
}

/**
 * Opens a terminal the way a user does: the same picker, choosing the terminal entry instead of an
 * agent tool. Returns its sidebar row.
 *
 * `label` is which terminal to wait for — the first is "Terminal" and later ones are numbered
 * ("Terminal 2", …) by the core, so a spec opening several names the one it expects.
 */
export async function openTerminal(
  label: string = TERMINAL_LABEL,
): Promise<RowHandle> {
  await openLaunchPicker();
  await launchPicker.choose(TERMINAL_ENTRY);
  await launchPicker.waitUntilClosed();
  return sidebar.waitForRow(label);
}

/**
 * Opens the launch picker, returning to the start surface and re-clicking until it is actually up.
 *
 * A single click and a single wait is not enough here: WebKitGTK under WebDriver drops a click
 * outright when the app is busy, and the picker is lazy-loaded behind a deferred overlay, so opening
 * it also waits on a chunk fetch. Either way the picker simply never appears, and since the suite
 * runs with no retries one dropped click takes a whole spec file with it — observed on both walks
 * that launch an agent, in different runs. Re-clicking is safe because the titlebar action *sets*
 * the picker open rather than toggling it, and the re-click is skipped once it is up, so a picker
 * that has opened is never dismissed. Same remedy the sidebar's actions menu already uses.
 */
export async function openLaunchPicker(): Promise<void> {
  await waitUntilOr(
    async () => {
      if (await launchPicker.isOpen()) return true;
      await startSurface.launchAgent();
      return launchPicker.isOpen();
    },
    () => "the launch picker never opened",
  );
}
