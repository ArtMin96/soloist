import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vitest/config";
import react, { reactCompilerPreset } from "@vitejs/plugin-react";
import babel from "@rolldown/plugin-babel";
import tailwindcss from "@tailwindcss/vite";
import { visualizer } from "rollup-plugin-visualizer";

const dir = path.dirname(fileURLToPath(import.meta.url));

// Set ANALYZE=1 (see `just ui-analyze`) to emit dist/bundle-stats.html, a treemap of what
// fills the frontend bundle. Empty by default, so a normal build stays byte-identical.
const bundleReport = process.env.ANALYZE
  ? [
      visualizer({
        filename: "dist/bundle-stats.html",
        template: "treemap",
        gzipSize: true,
        brotliSize: true,
      }),
    ]
  : [];

// Set VITE_E2E=1 (see `just e2e`) to prepend the WebdriverIO frontend plugin to the entry module.
// The e2e harness drives the app through it — it installs the globals the wdio Tauri service's
// eval bridge looks for, and without them the bridge cannot reach the page at all. It
// must never reach a shipped bundle, so it is injected here rather than imported by `main.tsx`:
// empty by default, which leaves a normal build byte-identical and never even resolves the
// dependency. A static prepended import also runs the plugin before React mounts without forcing
// top-level await into the entry (the `safari13` build target below has none).
const entryModule = path.resolve(dir, "./src/main.tsx");
const e2ePlugin: Plugin[] = process.env.VITE_E2E
  ? [
      {
        name: "wdio-tauri-plugin",
        enforce: "pre",
        transform(code, id) {
          return id.split("?")[0] === entryModule ? `import "@wdio/tauri-plugin";\n${code}` : null;
        },
      },
    ]
  : [];

export default defineConfig({
  plugins: [
    react(),
    // reactCompilerPreset() ships its own rolldown.filter, so this only runs babel over
    // files whose content looks like a component or hook — not every module in the graph.
    babel({ presets: [reactCompilerPreset()] }),
    tailwindcss(),
    ...e2ePlugin,
    ...bundleReport,
  ],
  resolve: {
    alias: {
      "@": path.resolve(dir, "./src"),
      // The diff viewer's fallback highlighter, which this app never reaches and which links
      // every grammar highlight.js publishes. Measured: it is 785 kB raw / 274 kB gzip of the
      // diff chunk. See the stub for what it answers and why that is safe.
      lowlight: path.resolve(dir, "./src/lib/diff/lowlight.ts"),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: process.env.TAURI_DEV_HOST || "localhost",
    watch: { ignored: ["**/target/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    target: "safari13",
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    setupFiles: ["./vitest.setup.ts"],
    // Each suite is a jsdom component test: mounting a React tree, running its effects, and
    // tearing it down all compete for CPU with whatever else the machine is doing, so a test's
    // wall time is a function of contention, not of any defect in the test. Measured worst case
    // across repeated full-suite runs under ordinary desktop load (load average 10-18 on an
    // 8-core box) was ~6.2s; vitest's 5s default leaves no margin against that.
    testTimeout: 20_000,
  },
});
