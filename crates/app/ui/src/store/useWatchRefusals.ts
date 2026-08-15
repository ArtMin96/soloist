import { useEffect, useState } from "react";
import { onDomainEvent } from "@/api";
import type { PurposeRefusals, WatchPurpose } from "@/domain";
import type { WatchRefusals } from "@/store/watchContext";

// Whether an announcement says anything the row does not already hold. It arrives as a fresh object
// however many times the core repeats it, so recognizing a repeat by identity would hand every
// project header a new map and re-render the sidebar for nothing.
function alreadyHeld(held: PurposeRefusals | undefined, announced: PurposeRefusals): boolean {
  const purposes = Object.keys(announced) as WatchPurpose[];
  if (purposes.length !== Object.keys(held ?? {}).length) return false;
  return purposes.every((purpose) => held?.[purpose] === announced[purpose]);
}

// Tracks which of each project's filesystem watches the OS has refused, so the sidebar can say what
// stopped working. Fed by WatchRefusalChanged, which the core edge-triggers in both directions: a
// refusal arrives once however many times the reactors retry it, and a watch established later
// arrives as an empty set of refusals that clears the row.
//
// A removed project is dropped as well, because the core withdraws a refusal only for a project it
// still knows about — its rows go with it either way, and a stale key would keep a notice alive for
// a project that is no longer listed.
//
// App-level, like the orphan store: a refusal belongs to a project, not to a process, and every
// project's header can show one at once.
export function useWatchRefusals(): WatchRefusals {
  const [refusals, setRefusals] = useState<WatchRefusals>(() => new Map());

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    onDomainEvent((event) => {
      if (event.type === "WatchRefusalChanged") {
        const { project, refusals: announced } = event;
        setRefusals((prev) => {
          if (alreadyHeld(prev.get(project), announced)) return prev;
          const next = new Map(prev);
          if (Object.keys(announced).length > 0) next.set(project, announced);
          else next.delete(project);
          return next;
        });
      }
      if (event.type === "ProjectRemoved") {
        const { id } = event;
        setRefusals((prev) => {
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

  return refusals;
}
