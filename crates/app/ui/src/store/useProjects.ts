import { useCallback, useState } from "react";
import { openProjectDirectory, projectLoad } from "@/api";
import type { ProjectLoad } from "@/domain";

export interface ProjectStore {
  /** Pick a project folder and load its stack; a cancelled picker is a no-op. */
  open: () => void;
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

// Project actions: load a project's stack from a chosen folder. Routes through the core
// (`project_load`); the resulting process events repopulate the read model, so this holds
// no process state of its own. The load's facts (whether a solo.yml was auto-created, how
// many commands it declares) become a `notice` so opening a folder is never silent.
// Failures surface through the shared error sink.
export function useProjects(reportError: (reason: unknown) => void): ProjectStore {
  const [notice, setNotice] = useState<string | null>(null);

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

  return { open, notice };
}
