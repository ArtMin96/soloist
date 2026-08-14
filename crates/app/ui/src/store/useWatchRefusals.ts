import { useEffect, useState } from "react";
import { onDomainEvent } from "@/api";
import type { WatchRefusals } from "@/store/watchContext";

// Tracks which projects the OS has refused a filesystem watch for, so the sidebar can say that a
// project's restart-on-change and live git status have stopped. Fed by WatchRefusalChanged, which
// the core edge-triggers in both directions: a refusal arrives once however many times the
// reactors retry it, and a watch established later arrives as a null refusal that clears the row.
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
        const { project, refusal } = event;
        setRefusals((prev) => {
          if (prev.get(project) === (refusal ?? undefined)) return prev;
          const next = new Map(prev);
          if (refusal) next.set(project, refusal);
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
