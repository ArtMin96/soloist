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

// The coordination-walk fixture data, single-sourced here and handed to the lead stub as the
// contents of the plan file below. The lead writes these over the real MCP/IPC wire (a scratchpad,
// a blocker chain, a comment); the spec asserts the panels render exactly them. camelCase keys match
// the lead's serde struct — the values live only here, so there is nothing to keep in sync but the
// field names.
export const COORDINATION = {
  /** The scratchpad the window opens and the lead re-writes to force the revision conflict. */
  scratchpad: "release-readiness",
  /** The objective the lead first creates — the revision the window opens at. */
  objectiveV1: "confirm the release is ready to cut",
  /** The objective the lead re-writes on the trigger — the concurrent edit that must survive. */
  objectiveV2: "release is ready; ship it once CI is green",
  /** The blocker todo's title — the blocked todo cannot complete until this is done. */
  blocker: "Tag the release commit",
  /** The blocked todo's title — gated by the blocker. */
  blocked: "Publish the release notes",
  /** A todo carrying a comment, so the board shows the bound author the core stamped. */
  commented: "Review the changelog",
  /** The comment body the lead writes, authored by its bound session. */
  comment: "Looks ready to ship from my side.",
} as const;

// Present in the data dir → the lead runs its coordination arm; its JSON is `COORDINATION`. One
// named const per side (the Rust stub names it `COORDINATION_PLAN_FILE`).
const COORDINATION_PLAN_FILE = "lead-coordination-plan";

// The trigger the running lead polls for to re-write the scratchpad, bumping its revision under the
// window's stale editor. One named const per side (the Rust stub names it `SCRATCHPAD_REWRITE_FILE`).
const SCRATCHPAD_REWRITE_FILE = "lead-scratchpad-rewrite";

/**
 * Puts the lead stub into its coordination arm before it is launched, by dropping the plan file it
 * reads at startup. Must be called before `launchAgent(LEAD)` so the file is on disk when the lead
 * binds. Without it the lead runs its lineage arm (spawn a worker) instead — the same binary, two
 * walks.
 */
export async function requestLeadCoordination(): Promise<void> {
  const dataDir = await appDataDir();
  writeFileSync(path.join(dataDir, COORDINATION_PLAN_FILE), JSON.stringify(COORDINATION));
}

/**
 * Asks the running lead stub to re-write the coordination scratchpad — a concurrent write over the
 * real MCP/IPC wire that bumps the scratchpad's revision under the window's stale editor. Drops the
 * trigger file the stub polls; the window's next save then loses to this write exactly as an agent's
 * edit would.
 */
export async function triggerScratchpadRewrite(): Promise<void> {
  const dataDir = await appDataDir();
  writeFileSync(path.join(dataDir, SCRATCHPAD_REWRITE_FILE), "");
}

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
