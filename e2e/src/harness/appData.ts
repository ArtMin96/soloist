import path from "node:path";
import { fileURLToPath } from "node:url";
import { invoke } from "./tauri.js";

const dir = path.dirname(fileURLToPath(import.meta.url));

// Everything a run writes lives under here. The app under test is confined to it, so a data
// directory outside it means the sandboxing failed.
const scratch = path.resolve(dir, "../../.tmp");

/** What the app reports about where it resolved its data directory to. */
interface DataDirReport {
  data_dir: string;
}

/**
 * The data directory the app under test **actually** resolved to, as the app itself reports it.
 *
 * Asking the app is the only reliable answer. Reading the harness's own `SOLOIST_APP_DATA_DIR`
 * would report whatever this process last set, which is not necessarily what the app was spawned
 * with — the app inherits the launcher's environment, not its worker's. The app's own report is
 * correct however that is wired.
 *
 * Carries the isolation tripwire every out-of-window surface shares: the CLI resolves the app it
 * drives from this directory, and the lead stub's close-signal is dropped inside it, so handing
 * either a real one would drive the developer's real Soloist rather than fail. Abort first.
 */
export async function appDataDir(): Promise<string> {
  const { data_dir: dataDir } = await invoke<DataDirReport>("mcp_setup_info");

  if (!dataDir.startsWith(scratch + path.sep)) {
    throw new Error(
      `harness isolation broken: the app reports its data directory as ${dataDir}, outside the ` +
        `e2e scratch tree — a surface driven against it would touch the developer's real Soloist; ` +
        `aborting before anything touches real state`,
    );
  }
  return dataDir;
}
