import { useCallback, useEffect, useState } from "react";
import { onDomainEvent, sessionWork } from "@/api";
import { useLatestRef } from "@/store/useLatestRef";
import { useReconcile } from "@/store/useReconcile";
import type { SessionWork } from "@/domain";

export interface SessionWorkStore {
  work: SessionWork | null;
  error: string | null;
}

type Snapshot = { forProcess: number | null; work: SessionWork | null };

const EMPTY: Snapshot = { forProcess: null, work: null };

// The session-work read model for one agent process — the coordination documents it holds now or
// touched this run, for the terminal header's context. `enabled` gates the whole hook: a non-agent
// process or a hidden pooled pane passes false, so it reads nothing and holds no live subscription
// — a pool of terminals does not each carry a chatty orchestration listener. Seeds from the
// snapshot, then re-reads on a SessionWorkChanged for this process, or a TodoChanged/ScratchpadChanged
// for the project the last read belonged to, coalesced to one re-read per animation frame
// (CLAUDE.md §6). A `snapshotRef` (rather than a `work` dependency) lets the event subscription stay
// attached across a re-read instead of re-subscribing on every change.
export function useSessionWork(process: number, enabled: boolean): SessionWorkStore {
  const [snapshot, setSnapshot] = useState(EMPTY);
  const [error, setError] = useState<string | null>(null);
  const snapshotRef = useLatestRef(snapshot);

  const fail = useCallback((reason: unknown) => setError(String(reason)), []);

  const refresh = useCallback(() => {
    if (!enabled) return;
    sessionWork(process)
      .then((work) => setSnapshot({ forProcess: process, work }))
      .catch(fail);
  }, [process, enabled, fail]);

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    let frame: number | null = null;

    // Coalesce a burst of events into a single re-read on the next frame, so the header updates at
    // most once per frame however chatty the agent is.
    const scheduleRefresh = () => {
      if (frame != null) return;
      frame = requestAnimationFrame(() => {
        frame = null;
        refresh();
      });
    };

    // Attach the listener before the first read, so an event emitted between the snapshot and the
    // subscription cannot be lost (snapshot-then-deltas).
    onDomainEvent((event) => {
      if (event.type === "SessionWorkChanged") {
        if (event.process === process) scheduleRefresh();
        return;
      }
      if (event.type !== "TodoChanged" && event.type !== "ScratchpadChanged") return;
      if (event.project === snapshotRef.current.work?.project) scheduleRefresh();
    })
      .then((stop) => {
        if (cancelled) {
          stop();
          return;
        }
        unlisten = stop;
        refresh();
      })
      .catch(fail);

    return () => {
      cancelled = true;
      unlisten?.();
      if (frame != null) cancelAnimationFrame(frame);
    };
  }, [process, enabled, refresh, fail, snapshotRef]);

  // Re-read on a backend resync signal or window focus, so a dropped SessionWorkChanged never
  // leaves the header stale. A no-op while disabled.
  useReconcile(refresh);

  // A response captured for another process (or before the first load) is stale: surface null
  // until this process's own data arrives, so switching processes never flashes the previous
  // header — deriving staleness here means no effect resets state.
  const work = enabled && snapshot.forProcess === process ? snapshot.work : null;
  return { work, error };
}
