import { useEffect, useState } from "react";
import { appInfo } from "@/api";
import type { AppInfo } from "@/domain";

// Loads static app identity once on mount.
export function useAppInfo(): AppInfo | null {
  const [info, setInfo] = useState<AppInfo | null>(null);
  useEffect(() => {
    appInfo()
      .then(setInfo)
      .catch(() => setInfo(null));
  }, []);
  return info;
}
