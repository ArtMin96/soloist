import type { ProcStatus } from "@domain";
import { sidebar } from "../screens/Sidebar.js";

const RUNNING: ProcStatus = "Running";

/**
 * Trusts and starts a command, then looks at a different process — the arrangement every walk
 * about a background alert needs.
 *
 * Starting a command from its row selects it, and the core says nothing about the process the
 * user is already watching, so the second half is not tidying: without it a signal raised next
 * would be suppressed and the walk would assert against silence it arranged itself.
 *
 * Selecting waits for the row to actually report itself selected, which matters more here than
 * anywhere else: the shell tells the core where the user is looking from that render, so a cue
 * given before it lands could be answered while the core still believes the user is watching the
 * process about to signal.
 */
export async function startInBackground(
  label: string,
  command: string,
  watching: string,
): Promise<void> {
  await sidebar.select(label);
  await sidebar.trust(label, command);
  await sidebar.start(label);
  await sidebar.waitForRowStatus(label, RUNNING);

  await sidebar.select(watching);
}
