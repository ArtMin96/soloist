import type { Terminal } from "@xterm/xterm";
// Type-only import: erased at build time, so it does not statically pull the WebGL addon
// into the main bundle — the runtime load below stays a code-split chunk.
import type { WebglAddon } from "@xterm/addon-webgl";

export type TerminalRenderer = "webgl" | "dom";

// The outcome of activating a renderer: which backend is driving the terminal, and a
// disposer the terminal lifecycle calls on unmount.
export interface RendererHandle {
  readonly renderer: TerminalRenderer;
  dispose(): void;
}

// The WebGL addon is a heavy GPU dependency, so it is fetched with a dynamic import: it
// lands in its own bundle chunk and is downloaded only when a terminal first mounts. The
// loader is a parameter so the fallback decision below is unit-testable without a real
// WebGL2 context (jsdom has none).
export type WebglModule = { WebglAddon: new (preserveDrawingBuffer?: boolean) => WebglAddon };
const importWebgl = (): Promise<WebglModule> => import("@xterm/addon-webgl");

// Activate the GPU (WebGL) renderer on an already-opened terminal, degrading to xterm's
// built-in DOM renderer when WebGL cannot drive it. xterm v6 removed the canvas renderer,
// so DOM is the only fallback. Two failure modes are covered:
//   • WebGL2 unavailable at activation (no GPU/driver, blocked context) — `loadAddon` runs
//     the addon's `activate`, which throws; we catch it and the terminal keeps the DOM
//     renderer it was opened with.
//   • the GPU context is lost later (driver reset, sleep/resume) — `onContextLoss` disposes
//     the addon, and xterm reverts to the DOM renderer for the rest of the session.
export async function activateTerminalRenderer(
  term: Terminal,
  load: () => Promise<WebglModule> = importWebgl,
): Promise<RendererHandle> {
  let addon: WebglAddon | undefined;
  try {
    const module = await load();
    addon = new module.WebglAddon();
    addon.onContextLoss(() => addon?.dispose());
    term.loadAddon(addon);
    const active = addon;
    return { renderer: "webgl", dispose: () => active.dispose() };
  } catch {
    // Reclaim a partially-built addon (dispose is idempotent and safe pre-activation), then
    // fall through to the DOM renderer the terminal already has.
    addon?.dispose();
    return { renderer: "dom", dispose: () => {} };
  }
}
