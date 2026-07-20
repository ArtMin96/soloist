import { rmSync, mkdirSync, existsSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { browser } from "@wdio/globals";
import { soloistCli } from "./src/harness/cli.js";
import { LEAD_AGENT } from "./src/harness/leadAgent.js";
import { WAIT } from "./src/harness/waits.js";

const dir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(dir, "..");

// The e2e build gets its own cargo target dir: the `wdio` feature links a WebDriver server into the
// binary, and building that into `target/debug` would both leave a driveable binary where `just dev`
// puts the ordinary one and force a full feature-flip rebuild every time a dev build and an e2e run
// alternate. Isolation costs disk, not correctness.
const targetDir = path.join(repoRoot, "target", "e2e");

// The app under test: the debug binary built by `onPrepare` with the `wdio` feature, which links the
// in-app WebDriver server the embedded provider attaches to. Release builds link neither plugin.
const appBinary = path.join(targetDir, "debug", "soloist");

// The stand-in lead agent (its own workspace under fixtures), built by `onPrepare`. The `codex`
// PATH stub execs it: launched by the app as an agent, it binds its MCP session and spawns a worker
// over the real IPC path, so the orchestration walk gets a genuine lead→worker lineage.
const leadAgentManifest = path.join(dir, "fixtures", "lead-agent", "Cargo.toml");
const leadAgentBin = path.join(targetDir, "debug", "lead-agent");

// Everything a run scribbles lives under here — per-session app data dirs and fixture scratch
// copies — wiped whole before every run, so a spec never reads or writes the developer's real
// Soloist state and one run's leftovers never decide the next run's result.
const scratchDir = path.join(dir, ".tmp");

// Failure evidence lands here: a screenshot and the page source per failed test. CI uploads this
// directory as an artifact, so a red CI run shows what the window actually looked like.
const logsDir = path.join(dir, "logs");

// The service spawns the app with the launcher's own environment, so these reach the app under test.
// They are set here rather than as a capability because the published Tauri capability type has no
// `env` field, even though the launcher honours one.
// WebKitGTK needs the X11 backend to accept automation under a Wayland session.
process.env.GDK_BACKEND = "x11";
// Stub agent CLIs shadow any real ones: `claude` on this PATH is the fixture stand-in, so the
// launch journey behaves identically on a developer box (real Claude installed) and in CI (none),
// and never touches a real agent session. The app inherits this PATH when the service spawns it.
process.env.PATH = `${path.join(dir, "fixtures", "bin")}${path.delimiter}${process.env.PATH ?? ""}`;
// The app captures a launch environment from `$SHELL -ilc env`, and that capture outranks the
// app's own environment when a process spawns — a real login shell would put a real `claude`
// back ahead of the stubs. The stand-in shell skips profiles, so the capture returns this exact
// environment.
process.env.SHELL = path.join(dir, "fixtures", "bin", "shell");
// The lead stub reads these from the app-inherited (and shell-captured) environment: where its
// compiled binary is, and which worker tool to spawn. The `codex` PATH stub execs the binary; the
// stub itself spawns the worker over MCP. One TS source (`leadAgent.ts`), shared with the spec
// that asserts the rendered labels.
process.env.SOLOIST_E2E_LEAD_BIN = leadAgentBin;
process.env.SOLOIST_E2E_WORKER_TOOL = LEAD_AGENT.worker;
// Every app data dir a run uses lives under here; `onWorkerStart` gives each session its own.
const appDataRoot = path.join(scratchDir, "app-data");

// The webview's own storage — `localStorage`, caches — which WebKitGTK keys by the bundle
// identifier under `XDG_DATA_HOME`, reaching none of it through `SOLOIST_APP_DATA_DIR`: that
// override moves the app's state, not its webview's. Unset, this resolves to the developer's real
// `~/.local/share/dev.soloist.app`, which the suite then reads and writes (observed: a run
// persisted a collapsed sidebar there, so every later run booted with no visible rows and failed
// three spec files with the harness and the product both innocent). `onWorkerStart` gives each
// session its own, so the webview starts at its defaults exactly like the app does.
const xdgDataRoot = path.join(scratchDir, "xdg-data");

// Set here, at module load, only so the variables are never unset: an app that resolved either
// would otherwise fall through to the developer's real `~/.local/share` (observed once, when the
// override stopped reaching the app — it listed their real projects). `onWorkerStart` replaces
// these with the session's real directories before any app is spawned.
process.env.SOLOIST_APP_DATA_DIR = path.join(appDataRoot, "unassigned");
process.env.XDG_DATA_HOME = path.join(xdgDataRoot, "unassigned");

// The app's own SIGTERM→SIGKILL grace for its children, mirrored for the app itself.
const APP_EXIT_GRACE_MS = 5_000;

/**
 * Runs one cargo build the suite depends on, failing the whole run loudly if the build errors or
 * the binary it was meant to produce did not appear where the harness will look for it.
 */
function buildBinary(
  what: string,
  args: string[],
  options: { cwd?: string; env: NodeJS.ProcessEnv },
  binary: string,
): void {
  // A build inherits the harness's environment but never its TypeScript loader. This config is read
  // through tsx, which advertises itself to child processes as `NODE_OPTIONS=--import <loader>`; the
  // app build shells out to `pnpm`, and pnpm looks for an optional `.pnpmfile.mjs` by importing it.
  // Under the loader an absent file comes back as a failed import rather than as absent, so pnpm
  // reports a pnpmfile error and the build dies on a file that was never meant to exist.
  const env = { ...options.env };
  delete env.NODE_OPTIONS;

  const build = spawnSync("cargo", args, { stdio: "inherit", ...options, env });
  if (build.status !== 0) {
    throw new Error(`Failed to build ${what} for e2e (exit ${build.status})`);
  }
  if (!existsSync(binary)) {
    throw new Error(`Built ${what} not found at ${binary}`);
  }
}

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./specs/**/*.spec.ts"],
  // One window at a time: the app is a single-instance supervisor, and a second launch would
  // forward its arguments to the first rather than opening a rival window.
  maxInstances: 1,
  capabilities: [
    {
      browserName: "tauri",
      "wdio:enforceWebDriverClassic": true,
      "wdio:tauriServiceOptions": {
        appBinaryPath: appBinary,
        driverProvider: "embedded",
      },
    },
  ],
  services: [["tauri", { driverProvider: "embedded" }]],
  framework: "mocha",
  reporters: ["spec"],
  logLevel: "warn",
  waitforTimeout: WAIT.render,
  connectionRetryTimeout: WAIT.session,
  mochaOpts: { ui: "bdd", timeout: WAIT.spec },

  onPrepare: () => {
    // Everything a previous run left, app data included. The only wipe: it runs once, before any
    // app exists, so it can never delete a running app's files.
    rmSync(scratchDir, { recursive: true, force: true });
    mkdirSync(scratchDir, { recursive: true });

    // Three switches, none of which any other build path sets, so no ordinary build can produce
    // this binary by accident: `--features wdio` links the in-app WebDriver server, `--config`
    // merges the e2e overlay (withGlobalTauri + the wdio capabilities), and `VITE_E2E` makes the
    // frontend build inject the wdio plugin the harness drives the app through.
    buildBinary(
      "the Soloist app",
      ["tauri", "build", "--debug", "--no-bundle", "--features", "wdio", "--config", "tauri.e2e.conf.json"],
      {
        cwd: path.join(repoRoot, "crates", "app"),
        env: { ...process.env, VITE_E2E: "1", CARGO_TARGET_DIR: targetDir },
      },
      appBinary,
    );

    // The `soloist` CLI the cross-surface walk drives the app from. A separate binary that
    // `cargo tauri build` does not produce, built into the same target dir so it shares the
    // workspace artifacts the app build just produced.
    buildBinary(
      "the soloist CLI",
      ["build", "-p", "soloist-cli"],
      { cwd: repoRoot, env: { ...process.env, CARGO_TARGET_DIR: targetDir } },
      soloistCli,
    );

    // The stand-in lead agent the orchestration walk launches. Its own workspace (kept out of the
    // product one), built by manifest path into the same target dir so it shares the workspace
    // artifacts the builds above just produced.
    buildBinary(
      "the lead-agent stub",
      ["build", "--manifest-path", leadAgentManifest],
      { env: { ...process.env, CARGO_TARGET_DIR: targetDir } },
      leadAgentBin,
    );
  },

  // Point the next session's app at its own fresh directories, named for the worker that will
  // drive it — its state, and its webview's. This is the only place that can: the app inherits the
  // *launcher's* environment, not its worker's, and this hook runs in the launcher before either is
  // spawned — a worker-side module or hook is too late, the app is already up.
  //
  // A directory per session is what keeps sessions independent, and it replaces a wipe that could
  // not be made safe. Wiping one shared dir raced the app it was isolating: the app boots ~3 s
  // before its worker loads this config, so the wipe deleted the running app's database, socket,
  // and HTTP runtime file out from under it. The app survived (an open SQLite handle keeps working
  // on an unlinked inode), which is why the walks stayed green while durable state was silently
  // non-durable and every on-disk artifact was invisible to anything looking for one.
  onWorkerStart: (cid) => {
    process.env.SOLOIST_APP_DATA_DIR = path.join(appDataRoot, cid);
    process.env.XDG_DATA_HOME = path.join(xdgDataRoot, cid);
  },

  // The embedded server's DELETE /session does not reliably quit the app. Left alive, the next
  // session's launch forwards to it (the app is single-instance) and the service eventually
  // SIGKILLs it as a port squatter — leaking its children as exactly the orphans a later
  // session's app then (rightly) raises its modal dialog over. Reap it the way a logout would:
  // SIGTERM, the app's own grace, then SIGKILL. Matching the e2e binary's full path can never
  // touch an ordinary dev instance.
  afterSession: async () => {
    spawnSync("pkill", ["-TERM", "-f", appBinary]);
    const deadline = Date.now() + APP_EXIT_GRACE_MS;
    while (spawnSync("pgrep", ["-f", appBinary]).status === 0) {
      if (Date.now() > deadline) {
        spawnSync("pkill", ["-KILL", "-f", appBinary]);
        break;
      }
      await delay(100);
    }
  },

  afterTest: async (test, _context, { passed }) => {
    if (passed) return;
    mkdirSync(logsDir, { recursive: true });
    const slug = `${test.parent} ${test.title}`.replace(/[^a-zA-Z0-9]+/g, "-").toLowerCase();
    await browser.saveScreenshot(path.join(logsDir, `${slug}.png`));
    writeFileSync(path.join(logsDir, `${slug}.html`), await browser.getPageSource());
  },
};
