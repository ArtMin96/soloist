// The emulator's two optional capabilities — grapheme-aware character widths and inline images —
// kept beside the terminal hook rather than inside it so the stream lifecycle and the addons that
// decorate it stay separately readable. Both are heavy relative to what they add, and neither is
// needed for a terminal to work, so both are fetched with a dynamic import: each lands in its own
// bundle chunk, downloaded when a pane first mounts. Each loader is a parameter so the degrade
// path is testable without the real addon.
//
// Type-only imports below are erased at build time, so they do not statically pull either addon
// into the main bundle — the runtime loads stay code-split chunks.
import type { IImageAddonOptions, ImageAddon } from "@xterm/addon-image";
import type { UnicodeGraphemesAddon } from "@xterm/addon-unicode-graphemes";
import type { IDisposable, Terminal } from "@xterm/xterm";
import { TERMINAL_POOL_CAP } from "@/store/useTerminalPool";

// Where the terminal's bounds live: a limit an addon's constructor takes is stated beside the
// module that constructs that addon, as these two are; a limit that is an xterm option belongs with
// the rest of the emulator's option surface in `lib/appearance`.

// Megabytes of decoded image the whole keep-alive pool may hold. The emulator's limit is per
// terminal — 128 MB by default — so the figure that decides the app's footprint is this one, and a
// per-pane number stated on its own would let the ceiling move silently the next time the pool
// grows. Held to a fraction of the runtime budget, trading a shorter image history for a footprint
// that stays inside it.
const TERMINAL_IMAGE_POOL_BUDGET_MB = 96;

// Megabytes of decoded image one terminal keeps before evicting the oldest: the pool's budget
// shared out, floored so the panes together never exceed it. Widening the pool therefore tightens
// each pane rather than raising the total.
export const TERMINAL_IMAGE_STORAGE_LIMIT_MB = Math.floor(
  TERMINAL_IMAGE_POOL_BUDGET_MB / TERMINAL_POOL_CAP,
);

// Ceiling on the pixels a single image may decode to; anything larger is discarded rather than
// drawn. Bounds the transient cost of decoding, which the addon documents as up to this many
// pixels times four bytes held by its decoder, plus the same again while pixels are transferred.
// The addon defaults to 4096×4096, four times this.
export const TERMINAL_IMAGE_PIXEL_LIMIT = 2048 * 2048;

const IMAGE_OPTIONS: IImageAddonOptions = {
  storageLimit: TERMINAL_IMAGE_STORAGE_LIMIT_MB,
  pixelLimit: TERMINAL_IMAGE_PIXEL_LIMIT,
};

export type UnicodeModule = { UnicodeGraphemesAddon: new () => UnicodeGraphemesAddon };
export type ImageModule = { ImageAddon: new (options?: IImageAddonOptions) => ImageAddon };

export interface TerminalAddonLoaders {
  unicode: () => Promise<UnicodeModule>;
  image: () => Promise<ImageModule>;
}

const DEFAULT_LOADERS: TerminalAddonLoaders = {
  unicode: () => import("@xterm/addon-unicode-graphemes"),
  image: () => import("@xterm/addon-image"),
};

type Disposer = () => void;
const NOT_LOADED: Disposer = () => {};

// Load one addon onto the terminal, degrading to a terminal without it rather than throwing. A
// failed load is not an error worth surfacing: the pane still renders its output, just without
// grapheme widths or inline images. `loadAddon` runs the addon's activation synchronously, so a
// rejected proposed-API gate or a broken addon surfaces here rather than later.
async function activate<T>(
  term: Terminal,
  load: () => Promise<T>,
  construct: (module: T) => { dispose(): void },
): Promise<Disposer> {
  let addon: { dispose(): void } | undefined;
  try {
    addon = construct(await load());
    term.loadAddon(addon as Parameters<Terminal["loadAddon"]>[0]);
  } catch {
    return NOT_LOADED;
  }
  const loaded = addon;
  return () => {
    try {
      loaded.dispose();
    } catch {
      // The addons reach back into the terminal as they release (the unicode one restores the
      // width tables it replaced), so a disposal racing a disposed terminal must not break the
      // rest of the pane's teardown.
    }
  };
}

/**
 * Load the optional addons onto an already-opened terminal.
 *
 * Grapheme clustering has to be activated, not merely loaded: registering the wider tables also
 * selects them as the emulator's active unicode version, which is what makes a ZWJ sequence or a
 * skin-tone modifier occupy the cells it actually paints instead of shearing the line.
 *
 * The two are independent — one failing to load leaves the other active — and both are loaded in
 * parallel so neither waits on the other's chunk.
 */
export async function activateTerminalAddons(
  term: Terminal,
  loaders: TerminalAddonLoaders = DEFAULT_LOADERS,
): Promise<IDisposable> {
  const disposers = await Promise.all([
    activate(term, loaders.unicode, (module) => new module.UnicodeGraphemesAddon()),
    activate(term, loaders.image, (module) => new module.ImageAddon(IMAGE_OPTIONS)),
  ]);
  return {
    dispose: () => {
      for (const dispose of disposers) dispose();
    },
  };
}
