import type { ProcStatus, ProjectView } from "@domain";
import { openProject } from "../../src/flows/openProject.js";
import {
  setProjectNotificationLevel,
  showCommandNotificationLevel,
} from "../../src/flows/projectNotifications.js";
import { startInBackground } from "../../src/flows/startInBackground.js";
import { CUE, give, withdraw } from "../../src/harness/cues.js";
import { requireWindowFocus } from "../../src/harness/windowFocus.js";
import { attentionControl } from "../../src/screens/AttentionControl.js";
import { projectSettingsPane } from "../../src/screens/ProjectSettingsPane.js";
import { sidebar } from "../../src/screens/Sidebar.js";
import { toastStack } from "../../src/screens/ToastStack.js";

const ECHO = "Echo";
const FAULTY = "Faulty";
const FAULTY_COMMAND = "./bin/cued-crasher.sh";

const RUNNING: ProcStatus = "Running";
const CRASHED: ProcStatus = "Crashed";

const CRASH_ALERT = `${FAULTY} crashed`;

// The levels as the user reads them. "Same as project" is not a fourth level but the absence of a
// command's own — the distinction the second walk exists for.
const ALL = "All";
const NONE = "None";
const INHERIT = "Same as project";

// How loud a project is, and how a single command can be held quieter than it. Both walks need
// the real store behind the window: the levels are a projection the pane re-reads from the core,
// and inheriting is stored as *nothing at all* rather than as a value — so the two states that
// read most alike on screen are the two most easily confused in storage.
describe("choosing how much a project notifies", () => {
  let project: ProjectView;

  before(async () => {
    project = await openProject("basic");
    await requireWindowFocus();
  });

  after(async () => {
    await sidebar.stopIfRunning(FAULTY);
  });

  it("keeps a command's own level, and keeps inheriting apart from silence, across a reopen", async () => {
    await showCommandNotificationLevel(project.name, ECHO);
    await projectSettingsPane.chooseLevel(NONE);

    // Selecting a process takes the pane off screen entirely, so reopening it builds a new one
    // from what the core stored rather than re-rendering what it was holding.
    await sidebar.select(FAULTY);
    await showCommandNotificationLevel(project.name, ECHO);

    expect(await projectSettingsPane.chosenLevel()).toBe(NONE);

    await projectSettingsPane.chooseLevel(INHERIT);
    await sidebar.select(FAULTY);
    await showCommandNotificationLevel(project.name, ECHO);

    // Inheriting is stored by storing nothing, so a core that wrote a value for it would read
    // back as the level it wrote — silence being the one that looks most like it.
    expect(await projectSettingsPane.chosenLevel()).toBe(INHERIT);
  });

  // Runs after the walk above, which leaves nothing on screen: the crash alert this one raises
  // stays until it is acted on, and it lands over the settings pane's own section switch.
  it("drops a crash while the project is set to None, and reports the same crash once it is All", async () => {
    await setProjectNotificationLevel(project.name, NONE);

    await startInBackground(FAULTY, FAULTY_COMMAND, ECHO);
    give(project.root, CUE.crash);
    await sidebar.waitForRowStatus(FAULTY, CRASHED);

    // The same crash again at All. It is the pair that proves the first half: an alert here is
    // what shows the window could have rendered one all along, and both crashes word themselves
    // identically — so one alert on screen means exactly one of the two was reported.
    await setProjectNotificationLevel(project.name, ALL);
    withdraw(project.root, CUE.crash);
    await sidebar.restart(FAULTY);
    await sidebar.waitForRowStatus(FAULTY, RUNNING);
    await sidebar.select(ECHO);
    give(project.root, CUE.crash);
    await sidebar.waitForRowStatus(FAULTY, CRASHED);

    await toastStack.waitForToast(CRASH_ALERT);
    await requireWindowFocus();

    const reported = (await toastStack.toasts()).filter((toast) => toast.title === CRASH_ALERT);
    expect(reported).toHaveLength(1);
    // Nor did the dropped crash leave anything waiting: a level of None is refused outright,
    // before the question of which surface would have shown it ever arises.
    expect(await attentionControl.count()).toBe(1);
  });
});
