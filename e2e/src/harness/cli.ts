import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { browser } from "@wdio/globals";
import { appDataDir } from "./appData.js";
import { WAIT } from "./waits.js";

const dir = path.dirname(fileURLToPath(import.meta.url));

/**
 * The real `soloist` CLI, built by `onPrepare` into the e2e run's own cargo target dir. A separate
 * binary from the app — `cargo tauri build` produces only the desktop one — so the config builds it
 * too and checks it landed here.
 */
export const soloistCli = path.resolve(dir, "../../../target/e2e/debug/soloist-cli");

// The file the running HTTP server records itself in — its bound port and the launch's auth token —
// inside the app's data directory. The one definition on this side of the boundary; the app's own
// is `RUNTIME_FILE` in `crates/ipc/src/http.rs`.
const RUNTIME_FILE = "http-api.json";

/**
 * Runs the real `soloist` CLI against the app under test and returns what it printed.
 *
 * This is the cross-surface walk's whole point, so unlike `harness/tauri.ts` it exists to **act**:
 * the CLI is a real user surface, and driving it is the behavior under test — one core command
 * reached from outside the window. It is a separate process talking to the app over its loopback
 * HTTP API, discovering the port and the launch's token from the runtime file.
 *
 * A non-zero exit throws, so a broken discovery or a refused token surfaces as itself rather than
 * as a later assertion's mystery timeout.
 */
export async function soloist(...args: string[]): Promise<string> {
  const dataDir = await appDataDir();
  const runtime = path.join(dataDir, RUNTIME_FILE);

  // The server binds and records itself asynchronously at boot, and the CLI refuses to guess a
  // port it has not read (`Client::from_runtime`), so a CLI run that beat the write would fail as
  // "not running". Wait for the record rather than race it.
  try {
    await browser.waitUntil(() => existsSync(runtime), { timeout: WAIT.core });
  } catch {
    throw new Error(
      `the app never recorded ${runtime}, so its loopback HTTP API never bound — the CLI has no ` +
        `port or token to reach it with`,
    );
  }

  const result = spawnSync(soloistCli, args, {
    env: { ...process.env, SOLOIST_APP_DATA_DIR: dataDir },
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(
      `soloist ${args.join(" ")} failed (exit ${result.status}): ${result.stderr.trim()}`,
    );
  }
  return result.stdout.trim();
}
