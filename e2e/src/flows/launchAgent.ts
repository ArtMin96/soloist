import { waitUntilOr } from "../harness/waitUntilOr.js";
import { agentPicker } from "../screens/AgentPicker.js";
import { sidebar } from "../screens/Sidebar.js";
import { titlebar } from "../screens/Titlebar.js";
import type { RowHandle } from "../screens/Sidebar.js";

/**
 * Launches an agent the way a user does: open the picker from the titlebar, pick the tool, and wait
 * for its row to appear. Returns the row, so a spec can assert on what was rendered.
 *
 * With exactly one project open the picker targets it and goes straight to the tool list; with
 * several it asks which project first, so a spec that opens more than one must drive that step.
 */
export async function launchAgent(tool: string): Promise<RowHandle> {
  await openAgentPicker();
  await agentPicker.choose(tool);
  await agentPicker.waitUntilClosed();
  return sidebar.waitForRow(tool);
}

/**
 * Opens the agent picker, re-clicking the titlebar action until it is actually up.
 *
 * A single click and a single wait is not enough here: WebKitGTK under WebDriver drops a click
 * outright when the app is busy, and the picker is lazy-loaded behind a deferred overlay, so opening
 * it also waits on a chunk fetch. Either way the picker simply never appears, and since the suite
 * runs with no retries one dropped click takes a whole spec file with it — observed on both walks
 * that launch an agent, in different runs. Re-clicking is safe because the titlebar action *sets*
 * the picker open rather than toggling it, and the re-click is skipped once it is up, so a picker
 * that has opened is never dismissed. Same remedy the sidebar's actions menu already uses.
 */
export async function openAgentPicker(): Promise<void> {
  await waitUntilOr(
    async () => {
      if (await agentPicker.isOpen()) return true;
      await titlebar.launchAgent();
      return agentPicker.isOpen();
    },
    () => "the agent picker never opened",
  );
}
