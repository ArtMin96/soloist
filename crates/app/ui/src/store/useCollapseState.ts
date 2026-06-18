import { useCallback, useState } from "react";
import type { ProcessKind } from "@/domain";

const STORAGE_KEY = "soloist.sidebar.collapsed";

type CollapseMap = Partial<Record<ProcessKind, boolean>>;

function load(): CollapseMap {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") as CollapseMap;
  } catch {
    return {};
  }
}

// Per-group collapse state for the sidebar, persisted across launches. A kind absent from
// the map defaults to expanded.
export function useCollapseState(): [CollapseMap, (kind: ProcessKind, collapsed: boolean) => void] {
  const [collapsed, setCollapsed] = useState<CollapseMap>(load);
  const set = useCallback((kind: ProcessKind, value: boolean) => {
    setCollapsed((prev) => {
      const next = { ...prev, [kind]: value };
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
      } catch {
        /* storage unavailable; keep the choice in memory for this session */
      }
      return next;
    });
  }, []);
  return [collapsed, set];
}
