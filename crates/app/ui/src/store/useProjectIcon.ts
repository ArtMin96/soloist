import { useEffect, useState } from "react";
import { projectIcon } from "@/api";

// Loads a project's icon as a data: URL for its sidebar avatar, or null when the project
// has no icon or it can't be read. The core resolves the path (`ProjectView.icon`); this
// only fetches it and tracks the result, ignoring a load that resolves after the path
// changed or the component unmounted.
export function useProjectIcon(path: string | null): string | null {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    if (path === null) {
      setSrc(null);
      return;
    }
    let active = true;
    projectIcon(path)
      .then((url) => {
        if (active) setSrc(url);
      })
      .catch(() => {
        if (active) setSrc(null);
      });
    return () => {
      active = false;
    };
  }, [path]);
  return src;
}
