import { createContext, use } from "react";
import type { PurposeRefusals } from "@/domain";

/** The projects the OS is currently refusing watches for, and which of their watches it turned down. */
export type WatchRefusals = ReadonlyMap<number, PurposeRefusals>;

const NOTHING_REFUSED: WatchRefusals = new Map();

// Which projects have lost their filesystem watch. Read at the leaves — one project header deep in
// the sidebar — but it would otherwise drill through the pass-through components between the
// sidebar and that header, so it travels by context the way the attention marks do. The default is
// nothing refused, so a component rendered without the provider (a focused test) shows no notice
// rather than throwing.
export const WatchContext = createContext<WatchRefusals>(NOTHING_REFUSED);

/** Which of this project's watches are refused and why, or `undefined` while it holds them all. */
export function useWatchRefusal(id: number): PurposeRefusals | undefined {
  return use(WatchContext).get(id);
}
