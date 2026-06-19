import { useCallback, useState } from "react";

const STORAGE_KEY = "soloist.sidebar.collapsed";

type CollapseMap = Record<string, boolean>;

function load(): CollapseMap {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") as CollapseMap;
  } catch {
    return {};
  }
}

// Per-section collapse state for the sidebar, persisted across launches and keyed by an
// opaque string so the tree can collapse at either level — a project (`project:<id>`) or
// one of its kind subgroups (`kind:<id>:<Kind>`). A key absent from the map is expanded,
// so a freshly opened project shows its processes by default.
export function useCollapseState(): [CollapseMap, (key: string, collapsed: boolean) => void] {
  const [collapsed, setCollapsed] = useState<CollapseMap>(load);
  const set = useCallback((key: string, value: boolean) => {
    setCollapsed((prev) => {
      const next = { ...prev, [key]: value };
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
