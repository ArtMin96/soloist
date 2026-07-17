import { writeFileSync } from "node:fs";
import path from "node:path";

const CONFIG_FILENAME = "solo.yml";

/**
 * Edits an open project's `solo.yml` the way an external editor would: a plain file write
 * the app never sees coming. The app's own config watcher is the feature under test — the
 * write is the stimulus, and everything observable (the reload, the trust review) must come
 * from the real window. `root` is the project root as the app itself reports it, so the
 * write lands exactly where the watch is.
 */
export function editConfigExternally(root: string, yml: string): void {
  writeFileSync(path.join(root, CONFIG_FILENAME), yml);
}
