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
  await titlebar.launchAgent();
  await agentPicker.waitUntilOpen();
  await agentPicker.choose(tool);
  await agentPicker.waitUntilClosed();
  return sidebar.waitForRow(tool);
}
