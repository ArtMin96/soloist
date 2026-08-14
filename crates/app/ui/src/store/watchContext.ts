import { createContext, use } from "react";
import type { WatchError } from "@/domain";

/** The projects the OS is currently refusing to watch, and why. */
export type WatchRefusals = ReadonlyMap<number, WatchError>;

const NOTHING_REFUSED: WatchRefusals = new Map();

// Which projects have lost their filesystem watch. Read at the leaves — one project header deep in
// the sidebar — but it would otherwise drill through the pass-through components between the
// sidebar and that header, so it travels by context the way the attention marks do. The default is
// nothing refused, so a component rendered without the provider (a focused test) shows no notice
// rather than throwing.
export const WatchContext = createContext<WatchRefusals>(NOTHING_REFUSED);

/** Why this project's directories are not being watched, or `undefined` while they are. */
export function useWatchRefusal(id: number): WatchError | undefined {
  return use(WatchContext).get(id);
}
