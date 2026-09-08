import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { browser } from "@wdio/globals";
import { WAIT } from "./waits.js";
import { waitUntilOr } from "./waitUntilOr.js";

const dir = path.dirname(fileURLToPath(import.meta.url));

// The app's own window configuration — the one place its minimum size is declared, read rather
// than restated so the harness can never shrink the window to a number the app no longer means.
const TAURI_CONF = path.resolve(dir, "../../../crates/app/tauri.conf.json");

/** The main window's declared size limits, as `tauri.conf.json` states them. */
interface WindowConfig {
  app: { windows: { label: string; minWidth: number; minHeight: number }[] };
}

/** A viewport's inner size in CSS pixels. */
export interface Viewport {
  width: number;
  height: number;
}

/** The smallest size the app lets its main window take. */
export function minimumWindowSize(): Viewport {
  const config: WindowConfig = JSON.parse(readFileSync(TAURI_CONF, "utf8"));
  const main = config.app.windows.find((window) => window.label === "main");
  if (!main) throw new Error(`no "main" window declared in ${TAURI_CONF}`);
  return { width: main.minWidth, height: main.minHeight };
}

/**
 * Shrinks the window to the smallest size the app allows and waits until the page is laid out at
 * that width, returning the viewport as the page then measures it. The window ships undecorated,
 * so the WebDriver outer size and the page's inner size are the same number; the wait is on the
 * page's own report, since the resize is a window-manager round trip that lands after the driver
 * call returns.
 */
export async function shrinkWindowToMinimum(): Promise<Viewport> {
  const minimum = minimumWindowSize();
  await browser.setWindowSize(minimum.width, minimum.height);
  let viewport: Viewport = { width: 0, height: 0 };
  await waitUntilOr(
    async () => {
      viewport = await browser.execute(() => ({
        width: window.innerWidth,
        height: window.innerHeight,
      }));
      return viewport.width <= minimum.width;
    },
    () =>
      `the window never shrank to its minimum width of ${minimum.width}px; the viewport is still ` +
      `${viewport.width}×${viewport.height}`,
    WAIT.render,
  );
  return viewport;
}
