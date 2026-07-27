// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import type { IImageAddonOptions, ImageAddon } from "@xterm/addon-image";
import { Terminal } from "@xterm/xterm";
import {
  activateTerminalAddons,
  TERMINAL_IMAGE_STORAGE_LIMIT_MB,
  type ImageModule,
  type TerminalAddonLoaders,
} from "@/components/terminal/terminalAddons";

// A ZWJ sequence: three emoji joined into one grapheme cluster. Without grapheme-aware widths the
// emulator stores each component in its own cell and the text after it is pushed a column right —
// which is what shears a TUI's columns. The cluster is the smallest input that tells the two apart.
const FAMILY = "\u{1F468}‍\u{1F469}‍\u{1F467}";

function terminal() {
  return new Terminal({ allowProposedApi: true, cols: 40 });
}

function write(term: Terminal, data: string) {
  return new Promise<void>((resolve) => term.write(data, resolve));
}

// The cells of the first row, as (contents, columns-occupied) pairs, up to `count`.
function cells(term: Terminal, count: number) {
  const line = term.buffer.active.getLine(0);
  return Array.from({ length: count }, (_, x) => {
    const cell = line?.getCell(x);
    return { chars: cell?.getChars() ?? "", width: cell?.getWidth() ?? -1 };
  });
}

// The real addons, with the image one captured on the way through so the limits it ended up
// running with can be read back off its own public surface.
function realLoaders() {
  const captured: { image?: ImageAddon } = {};
  const loaders: TerminalAddonLoaders = {
    unicode: () => import("@xterm/addon-unicode-graphemes"),
    image: async () => {
      const module = await import("@xterm/addon-image");
      return {
        ImageAddon: class extends module.ImageAddon {
          constructor(options?: IImageAddonOptions) {
            super(options);
            captured.image = this;
          }
        },
      } as ImageModule;
    },
  };
  return { loaders, captured };
}

const rejects = () => Promise.reject(new Error("chunk unavailable"));

describe("activateTerminalAddons", () => {
  it("stores a ZWJ sequence as one double-width cluster instead of splitting it", async () => {
    const term = terminal();
    await activateTerminalAddons(term, realLoaders().loaders);
    await write(term, `${FAMILY}|`);

    expect(cells(term, 3)).toEqual([
      { chars: FAMILY, width: 2 },
      { chars: "", width: 0 },
      { chars: "|", width: 1 },
    ]);
  });

  it("leaves the cluster split when the unicode chunk cannot be fetched", async () => {
    const term = terminal();
    const { loaders } = realLoaders();
    await activateTerminalAddons(term, { ...loaders, unicode: rejects });
    await write(term, `${FAMILY}|`);

    // Each component in its own cell, and the following character pushed a column right — the
    // pre-addon behavior, reached without throwing.
    expect(cells(term, 4).map((c) => c.width)).toEqual([1, 1, 1, 1]);
    expect(cells(term, 4)[3].chars).toBe("|");
  });

  it("runs the image storage at our limit, not the addon's much larger default", async () => {
    const term = terminal();
    const { loaders, captured } = realLoaders();
    await activateTerminalAddons(term, loaders);

    // The addon only reports a limit once it has been activated against a terminal, so this also
    // says the addon really attached rather than merely being constructed.
    expect(captured.image?.storageLimit).toBe(TERMINAL_IMAGE_STORAGE_LIMIT_MB);
  });

  it("still loads the image addon when the unicode chunk fails, and the reverse", async () => {
    const term = terminal();
    const { loaders, captured } = realLoaders();
    await activateTerminalAddons(term, { ...loaders, unicode: rejects });
    expect(captured.image?.storageLimit).toBe(TERMINAL_IMAGE_STORAGE_LIMIT_MB);

    const other = terminal();
    const second = realLoaders();
    await activateTerminalAddons(other, { ...second.loaders, image: rejects });
    await write(other, FAMILY);
    expect(cells(other, 1)[0]).toEqual({ chars: FAMILY, width: 2 });
  });

  it("gives the widths back when the handle is disposed, leaving nothing behind on the pane", async () => {
    const term = terminal();
    const handle = await activateTerminalAddons(term, realLoaders().loaders);
    handle.dispose();
    await write(term, `${FAMILY}|`);

    expect(cells(term, 4).map((c) => c.width)).toEqual([1, 1, 1, 1]);
  });

  it("degrades to a working terminal when neither chunk can be fetched", async () => {
    const term = terminal();
    const handle = await activateTerminalAddons(term, { unicode: rejects, image: rejects });
    await write(term, "ok");

    expect(cells(term, 2).map((c) => c.chars)).toEqual(["o", "k"]);
    expect(() => handle.dispose()).not.toThrow();
  });

  it("cannot reach the unicode tables at all without the proposed-API gate open", async () => {
    // The prerequisite, stated as a test: with the gate shut the emulator throws on the very
    // property the addon activates through, so the addon degrades instead of taking effect.
    const gated = new Terminal({ cols: 40 });
    await activateTerminalAddons(gated, realLoaders().loaders);
    await write(gated, `${FAMILY}|`);

    expect(cells(gated, 4).map((c) => c.width)).toEqual([1, 1, 1, 1]);
  });
});
