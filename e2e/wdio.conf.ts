import { rmSync, mkdirSync, existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const dir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(dir, "..");

// The app under test: the debug binary built by `onPrepare` with the `wdio` feature, which links the
// in-app WebDriver server the embedded provider attaches to. Release builds link neither plugin.
const appBinary = path.join(repoRoot, "target", "debug", "soloist");

// A run-scoped data dir, wiped before every run, so a spec never reads or writes the developer's real
// Soloist state. Soloist honours this override in place of the XDG default.
const appDataDir = path.join(dir, ".tmp", "app-data");

// The service spawns the app with the launcher's own environment, so these reach the app under test.
// They are set here rather than as a capability because the published Tauri capability type has no
// `env` field, even though the launcher honours one.
process.env.SOLOIST_APP_DATA_DIR = appDataDir;
// WebKitGTK needs the X11 backend to accept automation under a Wayland session.
process.env.GDK_BACKEND = "x11";

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
  waitforTimeout: 10_000,
  connectionRetryTimeout: 120_000,
  mochaOpts: { ui: "bdd", timeout: 120_000 },

  onPrepare: () => {
    rmSync(appDataDir, { recursive: true, force: true });
    mkdirSync(appDataDir, { recursive: true });

    // `--config` merges the e2e overlay (withGlobalTauri + the wdio capability) over
    // tauri.conf.json, and `--features wdio` links the in-app WebDriver server. Neither is set by
    // any other build path, so no other build can produce this binary by accident.
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
      { cwd: path.join(repoRoot, "crates", "app"), stdio: "inherit" },
    );
    if (build.status !== 0) {
      throw new Error(`Failed to build the Soloist app for e2e (exit ${build.status})`);
    }
    if (!existsSync(appBinary)) {
      throw new Error(`Built app binary not found at ${appBinary}`);
    }
  },
};
