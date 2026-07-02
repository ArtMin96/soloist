import { useCallback, useState } from "react";

export interface ToggleSet {
  has: (id: number) => boolean;
  toggle: (id: number) => void;
}

// In-session ephemeral membership state over ids — the collapse state for trees keyed by per-run
// process ids, which must never persist (a stored id would point at a different process next
// launch). Shared by the orchestration tree and the sidebar's lineage nesting.
export function useToggleSet(): ToggleSet {
  const [members, setMembers] = useState<ReadonlySet<number>>(() => new Set());
  const toggle = useCallback((id: number) => {
    setMembers((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);
  const has = useCallback((id: number) => members.has(id), [members]);
  return { has, toggle };
}
