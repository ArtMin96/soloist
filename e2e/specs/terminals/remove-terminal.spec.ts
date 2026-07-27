import type { ProcStatus } from "@domain";
import { openProject } from "../../src/flows/openProject.js";
import { openTerminal, TERMINAL_LABEL } from "../../src/flows/launch.js";
import { quickActionsPalette } from "../../src/screens/QuickActionsPalette.js";
import { removeProcessDialog } from "../../src/screens/RemoveProcessDialog.js";
import { sidebar } from "../../src/screens/Sidebar.js";

// A terminal runs its login shell until told otherwise, so Running is where an opened one settles
// and the state a removal has to reap out from under.
const RUNNING: ProcStatus = "Running";
const STOPPED: ProcStatus = "Stopped";

// The second terminal, numbered by the core. Used for the resting-removal walk so it is
// independent of whatever the first walk left behind.
const SECOND_TERMINAL = "Terminal 2";

describe("removing a terminal from a project", () => {
  before(async () => {
    await openProject("basic");
  });

  after(async () => {
    // Whatever a failed assertion left behind must not outlive this app session, or the next
    // session's app raises its orphan dialog over the leftover shell.
    await sidebar.stopIfRunning(TERMINAL_LABEL);
    await sidebar.stopIfRunning(SECOND_TERMINAL);
  });

  it("asks before removing a terminal that is still running", async () => {
    await openTerminal();
    await sidebar.waitForRowStatus(TERMINAL_LABEL, RUNNING);

    await sidebar.remove(TERMINAL_LABEL);

    await removeProcessDialog.waitUntilOpen();
    // Naming the process is the whole point of the confirmation — a dialog that does not say
    // which terminal it is about cannot be answered honestly.
    expect(await removeProcessDialog.names(TERMINAL_LABEL)).toBe(true);
  });

  it("leaves the terminal running when the confirmation is cancelled", async () => {
    await removeProcessDialog.cancel();
    await removeProcessDialog.waitUntilClosed();

    const row = await sidebar.waitForRow(TERMINAL_LABEL);
    expect(row.status).toBe(RUNNING);
  });

  it("takes the row out of the sidebar once confirmed", async () => {
    await sidebar.remove(TERMINAL_LABEL);
    await removeProcessDialog.waitUntilOpen();
    await removeProcessDialog.confirm();
    await removeProcessDialog.waitUntilClosed();

    // The row leaving is the observable outcome of a full core round trip: the supervisor reaps
    // the shell's process group, drops the registry entry, and publishes ProcessRemoved, which
    // the read-model projection folds in. A UI that merely hid the row would still be Running
    // in the core, so this is the assertion that separates "removed" from "hidden".
    await sidebar.waitForRowGone(TERMINAL_LABEL);
  });

  it("frees the terminal's number for the next one opened", async () => {
    // Numbering is computed from the labels actually in use, so removing "Terminal" must hand
    // its name back — a stale reservation would push this one to "Terminal 2".
    const row = await openTerminal(TERMINAL_LABEL);

    expect(row.label).toBe(TERMINAL_LABEL);
    await sidebar.waitForRowStatus(TERMINAL_LABEL, RUNNING);
  });

  it("removes a resting terminal outright, with nothing to confirm", async () => {
    await openTerminal(SECOND_TERMINAL);
    await sidebar.waitForRowStatus(SECOND_TERMINAL, RUNNING);
    await sidebar.stop(SECOND_TERMINAL);
    await sidebar.waitForRowStatus(SECOND_TERMINAL, STOPPED);

    await sidebar.remove(SECOND_TERMINAL);

    // Clearing finished work out of the sidebar is the point of the feature, so a resting row
    // costs no dialog. Asserting the row is gone *and* that nothing opened separates the
    // intended fast path from a confirmation that merely auto-answered itself.
    await sidebar.waitForRowGone(SECOND_TERMINAL);
    expect(await removeProcessDialog.isOpen()).toBe(false);
  });

  it("leaves the first terminal untouched throughout", async () => {
    // Removal targets one row; a walk that removed two would still pass every assertion above.
    const row = await sidebar.waitForRow(TERMINAL_LABEL);
    expect(row.status).toBe(RUNNING);
  });

  it("reaches the same confirmation from the quick-actions palette", async () => {
    // The palette is the second surface Remove reaches the user through, and the only one that
    // has to hand a live process off to a dialog *while closing itself* — two overlays trading
    // focus. jsdom renders each alone, so this is the walk that proves the handoff works.
    // The palette lists the *active* project's processes, and the project is active because
    // something in it is selected — so selecting the row first is the user's own path here (the
    // previous step removed the row that had been carrying the selection). Focus has to land on
    // the row rather than the terminal it opens, or the hotkey is typed into the shell instead.
    await sidebar.focusRow(TERMINAL_LABEL);
    await quickActionsPalette.open();
    await quickActionsPalette.run(TERMINAL_LABEL, "Remove");

    await quickActionsPalette.waitUntilClosed();
    await removeProcessDialog.waitUntilOpen();
    expect(await removeProcessDialog.names(TERMINAL_LABEL)).toBe(true);
  });

  it("is answerable from that handoff, leaving the terminal running on cancel", async () => {
    // If focus stayed trapped in the closing palette, the dialog would be up but unanswerable.
    await removeProcessDialog.cancel();
    await removeProcessDialog.waitUntilClosed();

    const row = await sidebar.waitForRow(TERMINAL_LABEL);
    expect(row.status).toBe(RUNNING);
  });

  it("removes from the palette once confirmed", async () => {
    await sidebar.focusRow(TERMINAL_LABEL);
    await quickActionsPalette.open();
    await quickActionsPalette.run(TERMINAL_LABEL, "Remove");
    await removeProcessDialog.waitUntilOpen();
    await removeProcessDialog.confirm();
    await removeProcessDialog.waitUntilClosed();

    await sidebar.waitForRowGone(TERMINAL_LABEL);
  });
});
