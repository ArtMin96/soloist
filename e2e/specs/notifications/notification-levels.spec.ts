import type { ProcStatus, ProjectView } from "@domain";
import { openProject } from "../../src/flows/openProject.js";
import {
  setProjectNotificationLevel,
  showCommandNotificationLevel,
} from "../../src/flows/projectNotifications.js";
import { startInBackground } from "../../src/flows/startInBackground.js";
import { CUE, give } from "../../src/harness/cues.js";
import { requireWindowFocus } from "../../src/harness/windowFocus.js";
import { attentionControl } from "../../src/screens/AttentionControl.js";
import { projectSettingsPane } from "../../src/screens/ProjectSettingsPane.js";
import { sidebar } from "../../src/screens/Sidebar.js";
import { toastStack } from "../../src/screens/ToastStack.js";

const ECHO = "Echo";
const FAULTY = "Faulty";
const SIGNALLER = "Signaller";
const FAULTY_COMMAND = "./bin/cued-crasher.sh";
const SIGNALLER_COMMAND = "./bin/signaller.sh";

const CRASHED: ProcStatus = "Crashed";

// What the Signaller writes for itself in its OSC 777 sequence.
const SCRIPT_ALERT = "Build";

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
    await sidebar.stopIfRunning(SIGNALLER);
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

  // Runs after the walk above, which leaves nothing on screen: the alerts this one raises land
  // over the settings pane's own section switch.
  it("drops a crash while the project is set to None, and reports a signal once it is All", async () => {
    await setProjectNotificationLevel(project.name, NONE);
    await startInBackground(FAULTY, FAULTY_COMMAND, ECHO);
    await startInBackground(SIGNALLER, SIGNALLER_COMMAND, ECHO);

    give(project.root, CUE.crash);
    await sidebar.waitForRowStatus(FAULTY, CRASHED);

    // Nothing is waiting on the user yet. Read on its own this could simply be early, so it is not
    // what proves the crash was refused — the unread list at the end is, since nothing clears a
    // mark once it is made. This is here so a level that gated nothing fails on the state it got
    // wrong, rather than further down on an alert standing over the settings pane's own controls.
    expect(await attentionControl.count()).toBe(null);

    await setProjectNotificationLevel(project.name, ALL);

    // A signal from the other process, now that the project admits one. Its alert is what shows
    // the window could have reported the crash all along, so the silence over that crash was a
    // decision and not an incapacity.
    give(project.root, CUE.notify);
    await toastStack.waitForToast(SCRIPT_ALERT);
    await requireWindowFocus();

    // Only the admitted signal is waiting on the user. The refused crash left no mark at all —
    // None is refused outright, before the question of which surface would have shown it arises —
    // and nothing has looked at the crashed process since, so a mark it made would still be here.
    await attentionControl.open();
    expect(await attentionControl.entries()).toEqual([SIGNALLER]);
  });
});
