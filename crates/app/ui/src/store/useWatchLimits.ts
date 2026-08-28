import { useEffect, useState } from "react";
import { onDomainEvent } from "@/api";
import type { PurposeLimits, WatchLimit, WatchPurpose } from "@/domain";
import type { WatchLimits } from "@/store/watchContext";

// Whether two limits for the same purpose are the same condition. A `WatchLimit` is either the
// string "degraded" or an object carrying the refusal reason, so `===` only recognizes a repeated
// degradation — a repeated refusal arrives as a fresh object each announcement and would compare
// unequal, defeating the point of comparing at all.
function sameLimit(a: WatchLimit, b: WatchLimit): boolean {
  if (typeof a === "string" || typeof b === "string") return a === b;
  return a.refused === b.refused;
}

// Whether an announcement says anything the row does not already hold. It arrives as a fresh object
// however many times the core repeats it, so recognizing a repeat by identity would hand every
// project header a new map and re-render the sidebar for nothing.
function alreadyHeld(held: PurposeLimits | undefined, announced: PurposeLimits): boolean {
  const purposes = Object.keys(announced) as WatchPurpose[];
  if (purposes.length !== Object.keys(held ?? {}).length) return false;
  return purposes.every((purpose) => {
    const current = held?.[purpose];
    const next = announced[purpose];
    return current !== undefined && next !== undefined && sameLimit(current, next);
  });
}

// Tracks which of each project's filesystem watches the OS has limited — refused or degraded — so
// the sidebar can say what changed. Fed by WatchLimitChanged, which the core edge-triggers in both
// directions: a limit arrives once however many times the reactors retry it, and a watch established
// in full later arrives as an empty set of limits that clears the row.
//
// A removed project is dropped as well, because the core withdraws a limit only for a project it
// still knows about — its rows go with it either way, and a stale key would keep a notice alive for
// a project that is no longer listed.
//
// App-level, like the orphan store: a limit belongs to a project, not to a process, and every
// project's header can show one at once.
export function useWatchLimits(): WatchLimits {
  const [limits, setLimits] = useState<WatchLimits>(() => new Map());

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    onDomainEvent((event) => {
      if (event.type === "WatchLimitChanged") {
        const { project, limits: announced } = event;
        setLimits((prev) => {
          if (alreadyHeld(prev.get(project), announced)) return prev;
          const next = new Map(prev);
          if (Object.keys(announced).length > 0) next.set(project, announced);
          else next.delete(project);
          return next;
        });
      }
      if (event.type === "ProjectRemoved") {
        const { id } = event;
        setLimits((prev) => {
          if (!prev.has(id)) return prev;
          const next = new Map(prev);
          next.delete(id);
          return next;
        });
      }
    })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return limits;
}
