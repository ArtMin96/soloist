import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
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

export default defineConfig({
  plugins: [react(), tailwindcss(), ...bundleReport],
  resolve: {
    alias: { "@": path.resolve(dir, "./src") },
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
  },
});
