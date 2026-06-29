import { describe, expect, it, vi } from "vitest";
import type { Terminal } from "@xterm/xterm";
import { activateTerminalRenderer, type WebglModule } from "@/lib/terminalRenderer";

// Stands in for the real WebglAddon: jsdom has no WebGL2 context, so the tests inject this
// in place of the dynamic import to drive the fallback logic deterministically.
class FakeWebglAddon {
  contextLoss?: () => void;
  disposed = 0;
  onContextLoss(listener: () => void) {
    this.contextLoss = listener;
  }
  dispose() {
    this.disposed += 1;
  }
}

function fakeTerminal(loadAddon = vi.fn()) {
  return { loadAddon } as unknown as Terminal;
}

const succeeds = () => Promise.resolve({ WebglAddon: FakeWebglAddon } as unknown as WebglModule);

function loadedAddon(term: Terminal): FakeWebglAddon {
  return vi.mocked(term.loadAddon).mock.calls[0][0] as unknown as FakeWebglAddon;
}

describe("activateTerminalRenderer", () => {
  it("loads the WebGL addon and reports the webgl renderer", async () => {
    const term = fakeTerminal();
    const handle = await activateTerminalRenderer(term, succeeds);
    expect(handle.renderer).toBe("webgl");
    expect(term.loadAddon).toHaveBeenCalledTimes(1);
  });

  it("reverts to the DOM renderer when the GPU context is lost", async () => {
    const term = fakeTerminal();
    await activateTerminalRenderer(term, succeeds);
    const addon = loadedAddon(term);
    expect(addon.disposed).toBe(0);
    addon.contextLoss?.();
    expect(addon.disposed).toBe(1);
  });

  it("disposes the addon when the handle is disposed", async () => {
    const term = fakeTerminal();
    const handle = await activateTerminalRenderer(term, succeeds);
    const addon = loadedAddon(term);
    handle.dispose();
    expect(addon.disposed).toBe(1);
  });

  it("falls back to DOM when WebGL2 is unavailable at activation", async () => {
    const term = fakeTerminal(
      vi.fn(() => {
        throw new Error("WebGL2 not supported");
      }),
    );
    const handle = await activateTerminalRenderer(term, succeeds);
    expect(handle.renderer).toBe("dom");
    expect(() => handle.dispose()).not.toThrow();
  });

  it("falls back to DOM when the addon chunk fails to load", async () => {
    const term = fakeTerminal();
    const handle = await activateTerminalRenderer(term, () =>
      Promise.reject(new Error("chunk load failed")),
    );
    expect(handle.renderer).toBe("dom");
    expect(term.loadAddon).not.toHaveBeenCalled();
  });
});
