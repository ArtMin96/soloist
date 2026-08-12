import { describe, expect, it, vi } from "vitest";
import {
  APPEARANCE_MUTATION_TARGET,
  createAppearanceMutationQueue,
} from "@/store/appearanceMutationQueue";

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

  it("supersedes a queued task for the same target, answering both callers with the winner", async () => {
    let current = { glassOpacity: 40 };
    const started: number[] = [];
    const release: Array<() => void> = [];
    const queue = createAppearanceMutationQueue({
      current: () => current,
      adopt: (value) => {
        current = value;
      },
      read: async () => current,
      write: async (value) => value,
    });
    const setGlassOpacity = (glassOpacity: number) =>
      queue.task(async () => {
        started.push(glassOpacity);
        await new Promise<void>((resolve) => release.push(resolve));
        return { glassOpacity };
      }, APPEARANCE_MUTATION_TARGET.glassOpacity);

    const inFlight = setGlassOpacity(60);
    const superseded = setGlassOpacity(70);
    const winner = setGlassOpacity(80);

    expect(started).toEqual([60]);
    release.shift()?.();
    // Only two commands ever run: the one already in flight, and the last one queued behind it.
    await vi.waitFor(() => expect(started).toEqual([60, 80]));
    release.shift()?.();

    expect(await Promise.all([inFlight, superseded, winner])).toEqual([
      { glassOpacity: 60 },
      { glassOpacity: 80 },
      { glassOpacity: 80 },
    ]);
    expect(current).toEqual({ glassOpacity: 80 });
  });
});
