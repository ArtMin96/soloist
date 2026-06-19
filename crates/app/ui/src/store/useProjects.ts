import { useCallback, useEffect, useState } from "react";
import { onDomainEvent, openProjectDirectory, projectList, projectLoad } from "@/api";
import type { DomainEvent, ProjectLoad, ProjectView } from "@/domain";

export interface ProjectStore {
  /** The opened projects, most-recently-opened first; the sidebar groups the tree by these. */
  projects: ProjectView[];
  /** Pick a project folder and load its stack; a cancelled picker is a no-op. */
  open: () => void;
  /** A plain-language note about the last open (auto-created config, or no commands). */
  notice: string | null;
}

type ProjectOpened = Extract<DomainEvent, { type: "ProjectOpened" }>;

// Upserts an opened project into the read model, newest first (matching the durable
// registry's order). Pure, so the fold is unit-testable without the event stream.
export function mergeProject(projects: ProjectView[], opened: ProjectOpened): ProjectView[] {
  const view: ProjectView = {
    id: opened.id,
    name: opened.name,
    root: opened.root,
    icon: opened.icon,
  };
  const existing = projects.findIndex((project) => project.id === view.id);
  if (existing === -1) return [view, ...projects];
  const next = projects.slice();
  next[existing] = view;
  return next;
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

// The projects store: the opened-project read model plus the action that loads one. Seeds
// from a snapshot then folds `ProjectOpened` deltas (snapshot-then-deltas, like
// `useProcesses`); the process events that loading triggers repopulate the process read
// model elsewhere. Each load's facts (auto-created config, declared-command count) become a
// `notice` so opening a folder is never silent; failures surface on the shared error sink.
export function useProjects(reportError: (reason: unknown) => void): ProjectStore {
  const [projects, setProjects] = useState<ProjectView[]>([]);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    // Subscribe before the snapshot read, so an open between the two is not lost.
    onDomainEvent((event) => {
      if (event.type === "ProjectOpened") {
        setProjects((prev) => mergeProject(prev, event));
      }
    })
      .then((stopListening) => {
        if (cancelled) {
          stopListening();
          return;
        }
        unlisten = stopListening;
        projectList().then(setProjects).catch(reportError);
      })
      .catch(reportError);
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [reportError]);

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

  return { projects, open, notice };
}
