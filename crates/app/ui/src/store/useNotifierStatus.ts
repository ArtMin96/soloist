import { useCallback, useState } from "react";
import { notifierStatus } from "@/api";
import { useLoadOnce } from "@/store/useLoadOnce";
import type { NotifierStatus } from "@/domain";

// Whether the desktop notification channel has anything listening. Probed once when the surface
// opens and again only when the user asks: answering it is a blocking round trip to the desktop, and
// a backend can start or stop while the app runs — so it is re-read on demand rather than polled or
// remembered, either of which would go stale without saying so.
//
// A failed probe reads as unavailable rather than as an error state of its own. The question is
// "can an alert reach the desktop", and a probe that could not complete has answered it.
export function useNotifierStatus(): { status: NotifierStatus; recheck: () => void } {
  const [status, setStatus] = useState<NotifierStatus>({ type: "unavailable" });

  const probe = useCallback(
    () =>
      notifierStatus().catch<NotifierStatus>(() => ({
        type: "unavailable",
      })),
    [],
  );

  useLoadOnce(probe, setStatus);

  const recheck = useCallback(() => {
    void probe().then(setStatus);
  }, [probe]);

  return { status, recheck };
}
