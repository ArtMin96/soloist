import type { ProjectLoad, ProjectView } from "@domain";
import { isScratchPath, materializeProject } from "../harness/fixtureProject.js";
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
  // Let the shell finish rendering before the first IPC call: driving the bridge while the
  // webview is still booting is where slow evals and their retries live.
  await sidebar.waitUntilReady();

  const path = materializeProject(fixture);
  const { id } = await invoke<ProjectLoad>("project_load", { path });

  const projects = await invoke<ProjectView[]>("project_list");
  // The isolation tripwire. An app under test can only ever know fixture projects; anything else
  // means the sandboxing failed and the run is driving the developer's real Soloist state —
  // observed once, when the data-dir override stopped reaching the app. Abort before any spec
  // clicks something real.
  const foreign = projects.filter((view) => !isScratchPath(view.root));
  if (foreign.length > 0) {
    throw new Error(
      `harness isolation broken: the app lists projects outside the e2e scratch dir: ` +
        `${foreign.map((view) => `${view.name} (${view.root})`).join(", ")} — ` +
        `it is running against a real data dir; aborting before any spec touches real state`,
    );
  }

  const project = projects.find((view) => view.id === id);
  if (!project) {
    throw new Error(`project_load reported id ${id}, but project_list does not list it`);
  }

  await sidebar.waitUntilReady();
  await sidebar.waitForProject(project.name);
  return project;
}
