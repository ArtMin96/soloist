import type { ProcStatus, ProjectView } from "@domain";
import { openProject } from "../../src/flows/openProject.js";
import { startInBackground } from "../../src/flows/startInBackground.js";
import { CUE, give } from "../../src/harness/cues.js";
import { requireWindowFocus } from "../../src/harness/windowFocus.js";
import { attentionControl } from "../../src/screens/AttentionControl.js";
import { sidebar } from "../../src/screens/Sidebar.js";
import { toastStack } from "../../src/screens/ToastStack.js";

// The fixture's two cue-driven stubs, and a third process to look at instead of them.
const ECHO = "Echo";
const FAULTY = "Faulty";
const SIGNALLER = "Signaller";
const FAULTY_COMMAND = "./bin/cued-crasher.sh";
const SIGNALLER_COMMAND = "./bin/signaller.sh";

const CRASHED: ProcStatus = "Crashed";

// The words the core writes for a crash, and the ones the Signaller writes for itself in its
// OSC 777 sequence — a title Soloist never composes, so a toast showing it can only have come
// through the parser.
const CRASH_ALERT = `${FAULTY} crashed`;
const BELL_ALERT = `${SIGNALLER} rang the bell`;
const SCRIPT_ALERT = "Build";

// The walk for everything that happens while the user is looking elsewhere: an alert reaches the
// window as a toast, the process it came from is marked unread until it is looked at, the title
// bar counts what is waiting, clearing empties every indicator at once, and acting on the toast
// takes the user to the process. Driven against the real supervisor and the real parser — the
// crash is a real process exiting nonzero and the second alert is a real escape sequence a stub
// printed, neither of which the window can fake.
describe("alerting about processes the user is not watching", () => {
  let project: ProjectView;

  // Both stubs are running and out of view before anything signals. Arranging them up front is
  // not only tidiness: starting a process is a click on its row, and a click landing while an
  // alert is on screen is a click the window may be too busy to take.
  before(async () => {
    project = await openProject("basic");
    await requireWindowFocus();
    await startInBackground(FAULTY, FAULTY_COMMAND, ECHO);
    await startInBackground(SIGNALLER, SIGNALLER_COMMAND, ECHO);
  });

  after(async () => {
    await sidebar.stopIfRunning(SIGNALLER);
    await sidebar.stopIfRunning(FAULTY);
  });

  it("raises a toast naming the process that crashed", async () => {
    give(project.root, CUE.crash);
    await sidebar.waitForRowStatus(FAULTY, CRASHED);

    const alert = await toastStack.waitForToast(CRASH_ALERT);

    expect(alert.body).toContain("exited unexpectedly");
  });

  it("marks the crashed process's row and its project as unread", async () => {
    const row = (await sidebar.rows()).find((candidate) => candidate.label === FAULTY);

    expect(row?.unread).toBe(true);
    expect(await sidebar.projectUnread(project.name)).toBe(true);
  });

  it("raises a toast in the words a process wrote for itself", async () => {
    give(project.root, CUE.notify);

    const alert = await toastStack.waitForToast(SCRIPT_ALERT);

    expect(alert.body).toBe("done");
  });

  it("counts every waiting process in the title bar, and names them", async () => {
    await attentionControl.waitForCount(2);
    await attentionControl.open();

    expect(await attentionControl.entries()).toEqual([FAULTY, SIGNALLER]);
  });

  it("clears every indicator at once", async () => {
    await attentionControl.clearAll();
    await attentionControl.waitUntilAbsent();

    expect((await sidebar.rows()).filter((row) => row.unread)).toEqual([]);
    expect(await sidebar.projectUnread(project.name)).toBe(false);
  });

  it("raises a toast for a bare terminal bell too", async () => {
    give(project.root, CUE.bell);

    const alert = await toastStack.waitForToast(BELL_ALERT);

    expect(alert.body).toContain("signalled for your attention");
  });

  it("takes the user to the process when its toast is acted on", async () => {
    // The crash toast is still on screen: the kinds that leave a process down and waiting stay
    // until they are acted on, which is what makes this walk's click free of any countdown.
    await toastStack.open(CRASH_ALERT);

    await sidebar.waitForRowSelected(FAULTY);
  });
});
