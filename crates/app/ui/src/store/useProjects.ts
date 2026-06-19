import { useCallback, useState } from "react";
import { openProjectDirectory, projectLoad } from "@/api";

export interface ProjectStore {
  /** Pick a project folder and load its stack; a cancelled picker is a no-op. */
  open: () => void;
  /** Set when the last opened folder declared no processes; shown in the empty state. */
  notice: string | null;
}

// The leaf name of a path, for naming the folder in a notice ("/home/dev/app" → "app").
function folderName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

// Project actions: load a project's stack from a chosen folder. Routes through the core
// (`project_load`); the resulting process events repopulate the read model, so this holds
// no process state of its own. A load that declares no processes (no solo.yml, or one
// declaring none) sets `notice` so opening a folder is never silent. Failures surface
// through the shared error sink.
export function useProjects(reportError: (reason: unknown) => void): ProjectStore {
  const [notice, setNotice] = useState<string | null>(null);

  const open = useCallback(() => {
    openProjectDirectory()
      .then((path) => {
        if (path === null) return;
        setNotice(null);
        return projectLoad(path).then(({ processes }) => {
          if (processes === 0) {
            setNotice(
              `No processes found in “${folderName(path)}”. Add a solo.yml with a processes: map to supervise its stack.`,
            );
          }
        });
      })
      .catch(reportError);
  }, [reportError]);

  return { open, notice };
}
