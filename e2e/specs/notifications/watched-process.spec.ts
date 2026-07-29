import type { ProcStatus, ProjectView } from "@domain";
import { openProject } from "../../src/flows/openProject.js";
import { startInBackground } from "../../src/flows/startInBackground.js";
import { CUE, give } from "../../src/harness/cues.js";
import { requireWindowFocus } from "../../src/harness/windowFocus.js";
import { attentionControl } from "../../src/screens/AttentionControl.js";
import { sidebar } from "../../src/screens/Sidebar.js";
import { toastStack } from "../../src/screens/ToastStack.js";

const ECHO = "Echo";
const FAULTY = "Faulty";
const SIGNALLER = "Signaller";
const FAULTY_COMMAND = "./bin/cued-crasher.sh";
const SIGNALLER_COMMAND = "./bin/signaller.sh";

const RUNNING: ProcStatus = "Running";
const CRASHED: ProcStatus = "Crashed";

const CRASH_ALERT = `${FAULTY} crashed`;
const SCRIPT_ALERT = "Build";

// The suppression walk: a user watching a process when it fails is told nothing, because they
// have just seen it. It is the rule with the least headless cover — jsdom can prove the routing
// function, but only the window can prove the app really tells the core where the user is looking
// and really renders nothing when it does.
//
// Both halves of the assertion are load-bearing. Suppression is one of two ways a crash produces
// no toast; the other is an unfocused window, which sends the alert to the desktop instead — and
// that one still leaves the row marked unread. Asserting the marker is absent as well as the
// toast is what separates "the app suppressed it" from "the window had lost focus".
describe("watching the process that fails", () => {
  let project: ProjectView;

  before(async () => {
    project = await openProject("basic");
    await requireWindowFocus();
  });

  after(async () => {
    await sidebar.stopIfRunning(SIGNALLER);
    await sidebar.stopIfRunning(FAULTY);
  });

  it("shows no unread indicator before anything has happened", async () => {
    expect((await sidebar.rows()).filter((row) => row.unread)).toEqual([]);
    expect(await sidebar.projectUnread(project.name)).toBe(false);
    expect(await attentionControl.count()).toBe(null);
  });

  it("says nothing about a crash the user watched happen", async () => {
    await sidebar.select(FAULTY);
    await sidebar.trust(FAULTY, FAULTY_COMMAND);
    await sidebar.start(FAULTY);
    await sidebar.waitForRowStatus(FAULTY, RUNNING);
    // Starting from the row leaves it selected, so the user is watching this one when it goes.
    await sidebar.waitForRowSelected(FAULTY);

    give(project.root, CUE.crash);
    await sidebar.waitForRowStatus(FAULTY, CRASHED);

    // A second process signals from the background, and its alert is what bounds the assertion
    // below: it is raised after the crash on the same bus, so once it is on screen anything the
    // crash was going to render already is. Waiting on a positive is the only honest way to say
    // "and the other one never came" without a sleep.
    await startInBackground(SIGNALLER, SIGNALLER_COMMAND, ECHO);
    give(project.root, CUE.notify);
    await toastStack.waitForToast(SCRIPT_ALERT);
    await requireWindowFocus();

    const titles = (await toastStack.toasts()).map((toast) => toast.title);
    expect(titles).not.toContain(CRASH_ALERT);

    // And nothing is left waiting on the user for it — an alert that had merely gone to the
    // desktop instead would still have marked the row and counted here.
    const marked = (await sidebar.rows()).filter((row) => row.unread).map((row) => row.label);
    expect(marked).toEqual([SIGNALLER]);
    await attentionControl.open();
    expect(await attentionControl.entries()).toEqual([SIGNALLER]);
  });
});
