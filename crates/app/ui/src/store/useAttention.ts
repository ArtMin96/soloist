import { useCallback, useEffect, useRef, useState } from "react";
import { attentionSnapshot, clearAllAttention, onDomainEvent } from "@/api";
import { NO_ATTENTION } from "@/lib/attention";
import type { AttentionSnapshot } from "@/domain";

export interface AttentionStore {
  /** Everything the core currently holds as unread — the one source every indicator renders from. */
  snapshot: AttentionSnapshot;
  /** Ask the core to drop every unread alert; the announcement it publishes empties the surface. */
  clearAll: () => void;
}

// Mirrors the core's unread registry. `AttentionChanged` carries no payload by design, so every
// change is answered by re-reading the whole snapshot rather than by folding a delta: there is no
// count kept here to drift from the core's, and the title bar, the sidebar and the dock badge all
// read the same one. A read that fails leaves the last snapshot standing — flashing to zero would
// claim nothing needs the user on the strength of a failed call.
export function useAttention(): AttentionStore {
  const [snapshot, setSnapshot] = useState<AttentionSnapshot>(NO_ATTENTION);
  // Reads can overlap when alerts arrive in a burst, and they can answer out of order; only an
  // answer at least as new as the newest already shown is allowed to land.
  const latest = useRef(0);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    const read = () => {
      const request = ++latest.current;
      attentionSnapshot()
        .then((next) => {
          if (cancelled || request !== latest.current) return;
          // A command the backend does not know resolves rather than rejects, so an answer is only
          // adopted once it is recognisably a snapshot; anything else would reach the derivations
          // every indicator renders from and throw into the render tree.
          if (next?.processes) setSnapshot(next);
        })
        .catch(() => {});
    };

    read();
    onDomainEvent((event) => {
      if (event.type === "AttentionChanged") read();
    })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const clearAll = useCallback(() => {
    void clearAllAttention().catch(() => {});
  }, []);

  return { snapshot, clearAll };
}
