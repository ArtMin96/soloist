import { createContext, use } from "react";
import type { PurposeLimits } from "@/domain";

/** The projects the OS is currently limiting watches for, and which of their watches are affected. */
export type WatchLimits = ReadonlyMap<number, PurposeLimits>;

const NOTHING_LIMITED: WatchLimits = new Map();

// Which projects have a limited filesystem watch. Read at the leaves — one project header deep in
// the sidebar — but it would otherwise drill through the pass-through components between the
// sidebar and that header, so it travels by context the way the attention marks do. The default is
// nothing limited, so a component rendered without the provider (a focused test) shows no notice
// rather than throwing.
export const WatchContext = createContext<WatchLimits>(NOTHING_LIMITED);

/** Which of this project's watches are limited and how, or `undefined` while it holds them all. */
export function useWatchLimit(id: number): PurposeLimits | undefined {
  return use(WatchContext).get(id);
}
