import { useCallback, useEffect, useState } from "react";
import { gitFiles, onDomainEvent } from "@/api";
import type { DomainEvent, ProjectFile } from "@/domain";

// The same announcement the status listens for: a path appearing or disappearing changes the
// listing too.
const SNAPSHOT_EVENTS: ReadonlySet<DomainEvent["type"]> = new Set(["GitStatusChanged"]);

export interface GitFilesStore {
  /** Every path in the repository, or null for a project that is not a repository. */
  files: ProjectFile[] | null;
  /** True until the first read for this project resolves. */
  loading: boolean;
  error: string | null;
}

interface Snapshot {
  forProject: number | null;
  files: ProjectFile[] | null;
}

const EMPTY: Snapshot = { forProject: null, files: null };

/**
 * A project's file listing, read only while `active` — the whole tree costs a walk of the
 * working tree, so it is read when something is showing it and not before. Re-reads on the same
 * announcement the status does, coalesced to one per animation frame.
 */
export function useGitFiles(project: number | null, active: boolean): GitFilesStore {
  const [snapshot, setSnapshot] = useState(EMPTY);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    if (project == null) return;
    gitFiles(project)
      .then((files) => {
        setSnapshot({ forProject: project, files });
        setError(null);
      })
      .catch((reason: unknown) => setError(String(reason)));
  }, [project]);

  useEffect(() => {
    if (project == null || !active) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    let frame: number | null = null;

    const scheduleRefresh = () => {
      if (frame != null) return;
      frame = requestAnimationFrame(() => {
        frame = null;
        refresh();
      });
    };

    onDomainEvent((event) => {
      if (SNAPSHOT_EVENTS.has(event.type)) scheduleRefresh();
    })
      .then((stop) => {
        if (cancelled) {
          stop();
          return;
        }
        unlisten = stop;
        refresh();
      })
      .catch((reason: unknown) => setError(String(reason)));

    return () => {
      cancelled = true;
      unlisten?.();
      if (frame != null) cancelAnimationFrame(frame);
    };
  }, [project, active, refresh]);

  const fresh = snapshot.forProject === project;
  return {
    files: fresh ? snapshot.files : null,
    loading: active && project != null && !fresh,
    error,
  };
}
