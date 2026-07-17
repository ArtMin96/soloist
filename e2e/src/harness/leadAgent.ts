import { writeFileSync } from "node:fs";
import path from "node:path";
import { appDataDir } from "./appData.js";

// The lead stub and the worker it spawns, as the built-in agent registry names them — the single TS
// source, shared with `wdio.conf.ts` (which passes the worker choice to the stub through the app's
// environment) and the spec (which asserts the rendered labels). The harness shadows these tools'
// CLIs with fixture stubs on PATH: `codex` is a wrapper that hands off to the compiled lead stub,
// `opencode` is the worker whose visible-output idle heuristic makes the activity flip deterministic.
export const LEAD_AGENT = {
  /** The manually-launched lead whose stub binds its session and spawns the worker. */
  lead: "Codex",
  /**
   * The worker the lead spawns over the real `spawn_agent` path. Exactly one: its rendered label
   * stays unique, so the walk targets and cleans it up unambiguously, and the nested-tree shape is
   * the same for one child or many.
   */
  worker: "OpenCode",
} as const;

// The trigger file the running lead stub polls for, inside the app's data directory. One named const
// per side of the boundary (the Rust stub names it `CLOSE_SIGNAL_FILE`).
const CLOSE_SIGNAL_FILE = "lead-close-signal";

/**
 * Asks the running lead stub to close its own bound-session process — the one core action that
 * removes it from the registry and re-roots its workers. Drops the trigger file the stub polls; the
 * stub then issues `close_process(self)` over its real IPC session, and the window reflects the
 * re-root.
 *
 * This is the cross-surface half of the re-root assertion: single-agent removal is reachable only
 * through a bound MCP/IPC session (never the local UI, HTTP, or CLI), so the close arrives from
 * outside the window and the tree is asserted to reflect it — the same shape as the CLI-restart walk.
 */
export async function closeLeadFromOutside(): Promise<void> {
  const dataDir = await appDataDir();
  writeFileSync(path.join(dataDir, CLOSE_SIGNAL_FILE), "");
}
