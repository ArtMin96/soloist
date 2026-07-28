import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { readClipboard, writeClipboard } from "@/lib/clipboard";

// The system clipboard the plugin commands reach, as a value a test can read back. `refuse` is how
// the app process turns a call down — a dropped capability grant, or a read of a clipboard holding
// nothing or holding something that is not text.
const { system } = vi.hoisted(() => ({
  system: { text: "", refuse: false },
}));
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: vi.fn(async (text: string) => {
    if (system.refuse) throw new Error("clipboard-manager.write_text not allowed");
    system.text = text;
  }),
  readText: vi.fn(async () => {
    if (system.refuse) throw new Error("clipboard-manager.read_text not allowed");
    return system.text;
  }),
}));

beforeEach(() => {
  system.text = "";
  system.refuse = false;
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("the terminal's clipboard seam", () => {
  it("writes text to the system clipboard", async () => {
    await writeClipboard("npm run dev");

    expect(system.text).toBe("npm run dev");
  });

  it("reads back what the system clipboard holds", async () => {
    system.text = "echo hello";

    expect(await readClipboard()).toBe("echo hello");
  });

  // Both callers are fire-and-forget: the key handler starts the work and returns so the chord is
  // swallowed before xterm can forward it to the PTY, and the selection listener runs inside the
  // emulator's own dispatch. Neither has anywhere to put a rejection, so neither may get one.
  it("settles rather than rejecting when a write is refused", async () => {
    system.text = "something the user copied earlier";
    system.refuse = true;

    await expect(writeClipboard("npm run dev")).resolves.toBeUndefined();
    expect(system.text).toBe("something the user copied earlier");
  });

  it("yields no text rather than rejecting when a read is refused", async () => {
    system.refuse = true;

    await expect(readClipboard()).resolves.toBe("");
  });
});
