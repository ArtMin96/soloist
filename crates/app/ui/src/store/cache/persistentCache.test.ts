// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";

// tauri-plugin-store is the disk boundary; back it with an in-memory map the test controls,
// so the envelope/versioning logic runs without a Tauri host. `failRead` flips reads to throw,
// to prove a read error degrades to a miss rather than propagating.
const backing = new Map<string, unknown>();
let failRead = false;

vi.mock("@tauri-apps/plugin-store", () => ({
  LazyStore: class {
    async get(key: string) {
      if (failRead) throw new Error("no tauri host");
      return backing.get(key);
    }
    async set(key: string, value: unknown) {
      backing.set(key, value);
    }
  },
}));

import { CacheKey, readSnapshot, writeSnapshot } from "@/store/cache/persistentCache";

beforeEach(() => {
  backing.clear();
  failRead = false;
});

describe("persistentCache", () => {
  it("reads back a written snapshot (hit)", async () => {
    const projects = [{ id: 1, name: "storefront", root: "/p", icon: null }];
    await writeSnapshot(CacheKey.projects, projects);
    expect(await readSnapshot(CacheKey.projects)).toEqual(projects);
  });

  it("returns null for an absent key (miss)", async () => {
    expect(await readSnapshot(CacheKey.appInfo)).toBeNull();
  });

  it("treats a snapshot written under a different schema version as a miss", async () => {
    // A blob from an older build: right key, stale envelope version.
    backing.set(CacheKey.agents, { version: 999, value: [{ tool: { name: "x" } }] });
    expect(await readSnapshot(CacheKey.agents)).toBeNull();
  });

  it("degrades a read failure to a miss instead of throwing", async () => {
    backing.set(CacheKey.projects, { version: 1, value: [] });
    failRead = true;
    await expect(readSnapshot(CacheKey.projects)).resolves.toBeNull();
  });
});
