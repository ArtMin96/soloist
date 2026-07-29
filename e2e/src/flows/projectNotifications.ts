import { projectSettingsPane } from "../screens/ProjectSettingsPane.js";
import { sidebar } from "../screens/Sidebar.js";

/**
 * Sets how much a project notifies, the way a user does: the project's own ••• menu, its
 * Notifications section, and the level.
 */
export async function setProjectNotificationLevel(
  project: string,
  level: string,
): Promise<void> {
  await sidebar.openProjectSettings(project);
  await projectSettingsPane.showSection("Notifications");
  await projectSettingsPane.chooseLevel(level);
}

/**
 * Shows one command's own notification level: the project's settings pane, its Commands section,
 * that command's editor.
 *
 * Called again to re-read a stored level, which is why the caller leaves the pane first — asking
 * for it while it is already open would re-render the state the pane is holding, and a walk about
 * what the core stored must not be able to pass on that.
 */
export async function showCommandNotificationLevel(
  project: string,
  command: string,
): Promise<void> {
  await sidebar.openProjectSettings(project);
  await projectSettingsPane.showSection("Commands");
  await projectSettingsPane.expandCommand(command);
}
