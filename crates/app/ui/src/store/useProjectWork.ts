import { useCallback, useEffect, useState } from "react";
import { onDomainEvent, projectWork } from "@/api";
import { useLatestRef } from "@/store/useLatestRef";
import { useReconcile } from "@/store/useReconcile";
import type { ProcessView, ProjectView, ProjectWork } from "@/domain";

type Work = ReadonlyMap<number, ProjectWork>;

// Drops any entry whose key is not in `ids` — a project removed from the store loses its rows
// without waiting for an event about it. Returns the same map when nothing needed dropping, so a
// caller comparing identity sees no change.
function withKnownIds(work: Work, ids: number[]): Work {
  const known = new Set(ids);
  let changed = false;
  const next = new Map(work);
  for (const key of next.keys()) {
    if (!known.has(key)) {
      next.delete(key);
      changed = true;
    }
  }
  return changed ? next : work;
}

// The cross-project coordination-work store the sidebar's Todos and Scratchpads groups read
// through: one `ProjectWork` per project, seeded on mount and kept live by SessionWorkChanged,
// TodoChanged and ScratchpadChanged. Modeled on `useWatchLimits` — one app-scope hook holding a
// map fed by a single domain-event listener — rather than a hook per project, so opening or
// closing project nodes never multiplies subscriptions.
export function useProjectWork(projects: ProjectView[], processes: ProcessView[]): Work {
  const [work, setWork] = useState<Work>(() => new Map());
  const projectsRef = useLatestRef(projects);
  const processesRef = useLatestRef(processes);

  // Merges freshly read entries into the map in one update. A read for a project that left the
  // store while the read was in flight is dropped rather than resurrecting a removed row.
  const merge = useCallback(
    (entries: ReadonlyArray<readonly [number, ProjectWork]>) => {
      setWork((prev) => {
        const known = new Set(projectsRef.current.map((project) => project.id));
        let next: Map<number, ProjectWork> | null = null;
        for (const [id, read] of entries) {
          if (!known.has(id)) continue;
          next ??= new Map(prev);
          next.set(id, read);
        }
        return next ?? prev;
      });
    },
    [projectsRef],
  );

  // Reads `ids` and merges the results in one update; a rejected read is dropped rather than
  // surfacing an error — a missing sidebar group must never raise a banner.
  const refresh = useCallback(
    (ids: number[]) => {
      if (ids.length === 0) return;
      Promise.all(
        ids.map((id) =>
          projectWork(id)
            .then((read): readonly [number, ProjectWork] | null => [id, read])
            .catch(() => null),
        ),
      ).then((results) => {
        const settled = results.filter(
          (entry): entry is readonly [number, ProjectWork] => entry !== null,
        );
        if (settled.length > 0) merge(settled);
      });
    },
    [merge],
  );

  const refreshAll = useCallback(() => {
    refresh(projectsRef.current.map((project) => project.id));
  }, [refresh, projectsRef]);

  // A joined key, not the array itself, so a re-rendered `projects` with the same ids does not
  // trigger a re-read.
  const projectIdsKey = projects.map((project) => project.id).join(",");

  useEffect(() => {
    const ids = projectsRef.current.map((project) => project.id);
    setWork((prev) => withKnownIds(prev, ids));
    refresh(ids);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- projectIdsKey is the intended dependency; projectsRef/refresh are stable across renders.
  }, [projectIdsKey]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    let frame: number | null = null;
    const pending = new Set<number>();

    // Coalesce a burst of events into a single re-read on the next frame, so a chatty run costs
    // the sidebar at most one batch of reads per frame.
    const scheduleRefresh = () => {
      if (frame != null) return;
      frame = requestAnimationFrame(() => {
        frame = null;
        const ids = Array.from(pending);
        pending.clear();
        refresh(ids);
      });
    };

    const queue = (id: number) => {
      pending.add(id);
      scheduleRefresh();
    };

    const queueEveryProject = () => {
      for (const project of projectsRef.current) pending.add(project.id);
      scheduleRefresh();
    };

    onDomainEvent((event) => {
      if (event.type === "SessionWorkChanged") {
        const project = processesRef.current.find(
          (process) => process.id === event.process,
        )?.project;
        // A process the store no longer knows about (it left before its close event arrived)
        // must still clear its rows; refreshing every project is the only way to reach them.
        if (project !== undefined) queue(project);
        else queueEveryProject();
        return;
      }
      if (event.type === "TodoChanged" || event.type === "ScratchpadChanged") {
        queue(event.project);
      }
    })
      .then((stop) => {
        if (cancelled) {
          stop();
          return;
        }
        unlisten = stop;
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      unlisten?.();
      if (frame != null) cancelAnimationFrame(frame);
    };
  }, [refresh, processesRef, projectsRef]);

  useReconcile(refreshAll);

  return work;
}
