import { describe, expect, it } from "vitest";
import { createAppearanceMutationQueue } from "@/store/appearanceMutationQueue";

describe("createAppearanceMutationQueue", () => {
  it("serializes task commands with rebased and coalesced document updates", async () => {
    let current = { theme: "default", scale: 1 };
    const started: string[] = [];
    const release: Array<() => void> = [];
    const queue = createAppearanceMutationQueue({
      current: () => current,
      adopt: (value) => {
        current = value;
      },
      read: async () => current,
      write: async (value) => {
        started.push(`write:${value.theme}:${value.scale}`);
        return value;
      },
    });

    const theme = queue.task(async () => {
      started.push("theme");
      await new Promise<void>((resolve) => release.push(resolve));
      return { theme: "poimandres", scale: current.scale };
    });
    const scaleTwo = queue.update((value) => ({ ...value, scale: 2 }));
    const scaleThree = queue.update((value) => ({ ...value, scale: 3 }));

    expect(started).toEqual(["theme"]);
    release.shift()?.();
    await Promise.all([theme, scaleTwo, scaleThree]);

    expect(started).toEqual(["theme", "write:poimandres:3"]);
    expect(current).toEqual({ theme: "poimandres", scale: 3 });
  });
});
