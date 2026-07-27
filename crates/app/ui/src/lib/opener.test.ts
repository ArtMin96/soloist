import { beforeEach, describe, expect, it, vi } from "vitest";

// Stands in for the desktop: records what actually reached the system opener, so each case asserts
// whether a URL was opened rather than how the guard was written.
const { opened } = vi.hoisted(() => ({ opened: [] as string[] }));
vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(async (url: string) => {
    opened.push(url);
  }),
}));

import { openExternal } from "@/lib/opener";

beforeEach(() => {
  opened.length = 0;
});

describe("openExternal", () => {
  it("opens an https URL in the desktop's browser", async () => {
    await openExternal("https://example.com/docs");

    expect(opened).toEqual(["https://example.com/docs"]);
  });

  it("opens a plain http URL", async () => {
    await openExternal("http://localhost:5173/status");

    expect(opened).toEqual(["http://localhost:5173/status"]);
  });

  // A URL in terminal output is written by whatever process is running there. These three are the
  // schemes that turn "a link was clicked" into "a local file was handed to the desktop" or "script
  // the emitting program chose ran", so none of them may reach the opener.
  it.each([
    "file:///etc/passwd",
    "javascript:alert(1)",
    "data:text/html,<script>alert(1)</script>",
  ])("opens nothing for %s", async (url) => {
    await openExternal(url);

    expect(opened).toEqual([]);
  });

  it("opens nothing for a string that is not a URL at all", async () => {
    await openExternal("not a url");

    expect(opened).toEqual([]);
  });

  it("resolves rather than throwing when the desktop refuses", async () => {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    vi.mocked(openUrl).mockRejectedValueOnce(new Error("no handler"));

    // A refused link leaves the terminal with nothing to handle; the caller is a mouse event.
    await expect(openExternal("https://example.com")).resolves.toBeUndefined();
  });
});
