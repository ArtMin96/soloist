import type { ProcStatus, ProjectView } from "@domain";
import { editConfigExternally } from "../../src/flows/editConfigExternally.js";
import { openProject } from "../../src/flows/openProject.js";
import { sidebar } from "../../src/screens/Sidebar.js";
import { trustDialog } from "../../src/screens/TrustDialog.js";

const ECHO = "Echo";
const RUNNING: ProcStatus = "Running";

// The fixture's config with Echo's command changed (an argument the stub ignores, so the
// command still runs) — a different command variant, which the app must refuse to run until
// it is re-trusted. The other processes are untouched, so only Echo needs review.
const CHANGED_CONFIG = `name: e2e-basic
processes:
  Echo:
    command: ./bin/echo-loop.sh changed
    auto_start: false
  Crasher:
    command: ./bin/crasher.sh
    auto_start: false
  Listener:
    command: ./bin/listener.sh
    auto_start: false
`;

// The trust-review walk: editing an open project's solo.yml outside the app — an editor, a
// teammate's pull — must reach the running app by itself (the config watcher), raise the
// review dialog for the changed command, and keep that command blocked until it is trusted
// from the dialog. Every step is observable app behavior: the dialog opening proves the
// watcher → reload → trust-review chain end to end, and the disabled-then-enabled Start
// control proves trusting is what unblocks the start.
describe("reviewing an external solo.yml change", () => {
  let project: ProjectView;

  before(async () => {
    project = await openProject("basic");
  });

  after(async () => {
    await sidebar.stopIfRunning(ECHO);
  });

  it("an external edit raises the trust review for the changed command", async () => {
    editConfigExternally(project.root, CHANGED_CONFIG);

    await trustDialog.waitUntilOpen();
    expect(await trustDialog.listsCommand(ECHO)).toBe(true);
  });

  it("the changed command cannot start until trusted from the review", async () => {
    // The untrusted row keeps its controls visible even unselected, and the modal review
    // blocks sidebar clicks anyway — so the gate is read, not clicked, while the dialog is up.
    expect(await sidebar.startEnabled(ECHO)).toBe(false);

    await trustDialog.trust(ECHO);
    await trustDialog.waitUntilClosed();

    await sidebar.select(ECHO);
    await sidebar.start(ECHO);
    await sidebar.waitForRowStatus(ECHO, RUNNING);
  });
});
