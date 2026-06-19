import { useCallback } from "react";
import { openProjectDirectory, projectLoad } from "@/api";

export interface ProjectStore {
  /** Pick a project folder and load its stack; a cancelled picker is a no-op. */
  open: () => void;
}

// Project actions: load a project's stack from a chosen folder. Routes through the core
// (`project_load`); the resulting process events repopulate the read model, so this holds
// no process state of its own. Failures surface through the shared error sink.
export function useProjects(reportError: (reason: unknown) => void): ProjectStore {
  const open = useCallback(() => {
    openProjectDirectory()
      .then((path) => {
        if (path !== null) return projectLoad(path);
      })
      .catch(reportError);
  }, [reportError]);

  return { open };
}
