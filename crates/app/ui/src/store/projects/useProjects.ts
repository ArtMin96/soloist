import { useCallback, useEffect, useState } from "react";
import {
  onDomainEvent,
  openProjectDirectory,
  projectList,
  projectLoad,
  projectRemove,
} from "@/api";
import { CacheKey } from "@/store/cache/persistentCache";
import { usePersistentSnapshot } from "@/store/cache/usePersistentSnapshot";
import type { ProjectLoad, ProjectView } from "@/domain";

export interface ProjectStore {
  /** The opened projects, most-recently-opened first; the sidebar groups the tree by these. */
  projects: ProjectView[];
  /** Pick a project folder and load its stack; a cancelled picker is a no-op. */
  open: () => void;
  /**
   * Remove a project from Soloist (the core closes its processes and forgets its state;
   * files on disk are untouched). The caller confirms first — this action just routes.
   */
  remove: (project: number) => void;
  /** A plain-language note about the last open (auto-created config, or no commands). */
  notice: string | null;
}

// The leaf name of a path, for naming the folder in a notice ("/home/dev/app" → "app").
function folderName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

// The single source of the post-open copy: plain language for someone who may not have
// written the config, derived only from the load's facts. Returns null when the stack
// simply populated (nothing to say).
function noticeFor(folder: string, { created, processes }: ProjectLoad): string | null {
  if (created) {
    if (processes === 0) {
      return `Created a starter solo.yml in “${folder}”. Add the commands you want Soloist to run.`;
    }
    const commands = processes === 1 ? "1 command" : `${processes} commands`;
    return `Created a solo.yml in “${folder}” with the ${commands} Soloist detected — pick the ones you want to run.`;
  }
  if (processes === 0) {
    return `“${folder}” has a solo.yml but no commands yet. Add the ones you want Soloist to run.`;
  }
  return null;
}

// The projects store: the opened-project read model plus the action that loads one. The
// rendered project snapshot is a persisted stale-while-revalidate cache — the sidebar paints
// the last-known projects instantly on launch, then reconciles to the core (which always
// wins); a `ProjectOpened` re-reads it (the snapshot already carries each project's loaded
// icon, so there is no separate icon fetch). The process events a load triggers repopulate
// the process read model elsewhere. Each load's facts (auto-created config, declared-command
// count) become a `notice` so opening a folder is never silent; failures surface on the
// shared error sink.
export function useProjects(reportError: (reason: unknown) => void): ProjectStore {
  const [notice, setNotice] = useState<string | null>(null);
  const { value, revalidate } = usePersistentSnapshot(CacheKey.projects, () => projectList(), {
    onError: reportError,
  });
  const projects = value ?? [];

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    // `ProjectOpened`/`ProjectRemoved` just signal "projects changed"; re-read the snapshot.
    onDomainEvent((event) => {
      if (event.type === "ProjectOpened" || event.type === "ProjectRemoved") revalidate();
    })
      .then((stopListening) => {
        if (cancelled) {
          stopListening();
          return;
        }
        unlisten = stopListening;
      })
      .catch(reportError);
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [revalidate, reportError]);

  const open = useCallback(() => {
    openProjectDirectory()
      .then((path) => {
        if (path === null) return;
        setNotice(null);
        return projectLoad(path).then((load) => {
          setNotice(noticeFor(folderName(path), load));
        });
      })
      .catch(reportError);
  }, [reportError]);

  // The row disappearing (via the `ProjectRemoved` re-read) is the confirmation; only a
  // failure needs surfacing.
  const remove = useCallback(
    (project: number) => {
      projectRemove(project).catch(reportError);
    },
    [reportError],
  );

  return { projects, open, remove, notice };
}
