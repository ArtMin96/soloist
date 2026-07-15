import type { ProjectLoad, ProjectView } from "@domain";
import { materializeProject } from "../harness/fixtureProject.js";
import { invoke } from "../harness/tauri.js";
import { sidebar } from "../screens/Sidebar.js";

/**
 * Opens a fixture project and waits until the window shows it. Returns the project as the app
 * itself reports it, so a spec asserts against the app's own naming rather than restating the
 * fixture's.
 *
 * The user opens a project through the OS folder picker, which a WebDriver session cannot drive —
 * so this calls the same core command that picker's handler calls, then reads the window the way a
 * user would. The load is real: it parses the fixture's `solo.yml`, writes durable rows, and
 * registers the commands the sidebar renders.
 */
export async function openProject(fixture: string): Promise<ProjectView> {
  const path = materializeProject(fixture);
  const { id } = await invoke<ProjectLoad>("project_load", { path });

  const project = (await invoke<ProjectView[]>("project_list")).find((view) => view.id === id);
  if (!project) {
    throw new Error(`project_load reported id ${id}, but project_list does not list it`);
  }

  await sidebar.waitUntilReady();
  await sidebar.waitForProject(project.name);
  return project;
}
