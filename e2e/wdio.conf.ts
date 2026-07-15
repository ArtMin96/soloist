import { rmSync, mkdirSync, existsSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { browser } from "@wdio/globals";
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
// Each spec file's worker re-imports this config, so this runs in every worker before the service
// spawns the app — a lifecycle hook is too late, the app is already up (observed: it launched
// against the developer's real data dir and listed their real projects). Every session gets its
// own data dir, so no session boots into another's durable state (restored projects, orphan
// bookkeeping, trust); the launcher's value is never used to spawn anything. A journey that needs
// persistence across launches will share a dir deliberately; nothing inherits one by accident.
const sessionDataDir = path.join(scratchDir, "app-data", process.env.WDIO_WORKER_ID ?? "launcher");
rmSync(sessionDataDir, { recursive: true, force: true });
mkdirSync(sessionDataDir, { recursive: true });
process.env.SOLOIST_APP_DATA_DIR = sessionDataDir;

// The app's own SIGTERM→SIGKILL grace for its children, mirrored for the app itself.
const APP_EXIT_GRACE_MS = 5_000;

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
    rmSync(scratchDir, { recursive: true, force: true });
    mkdirSync(scratchDir, { recursive: true });

    // Three switches, none of which any other build path sets, so no ordinary build can produce
    // this binary by accident: `--features wdio` links the in-app WebDriver server, `--config`
    // merges the e2e overlay (withGlobalTauri + the wdio capabilities), and `VITE_E2E` makes the
    // frontend build inject the wdio plugin the harness drives the app through.
    const build = spawnSync(
      "cargo",
      [
        "tauri",
        "build",
        "--debug",
        "--no-bundle",
        "--features",
        "wdio",
        "--config",
        "tauri.e2e.conf.json",
      ],
      {
        cwd: path.join(repoRoot, "crates", "app"),
        env: { ...process.env, VITE_E2E: "1", CARGO_TARGET_DIR: targetDir },
        stdio: "inherit",
      },
    );
    if (build.status !== 0) {
      throw new Error(`Failed to build the Soloist app for e2e (exit ${build.status})`);
    }
    if (!existsSync(appBinary)) {
      throw new Error(`Built app binary not found at ${appBinary}`);
    }
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
