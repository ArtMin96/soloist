import { useEffect, useState } from "react";
import { toast } from "sonner";
import { onDomainEvent } from "@/api";
import { ProcessToast } from "@/components/ProcessToast";
import { Toaster } from "@/components/ui/sonner";
import { playBell } from "@/lib/bell";
import { TOAST, TOAST_LIFETIME_MS } from "@/lib/notifications";
import { createToastDismissal, type ToastId } from "@/lib/toastDismissal";
import { useAppearance } from "@/store/appearanceContext";
import { useLatestRef } from "@/store/useLatestRef";
import type { ProcessView } from "@/domain";

export interface NotificationToastsProps {
  /** The live stack, so a toast for a process that has since gone leads nowhere. */
  processes: ProcessView[];
  onSelectProcess: (id: number) => void;
}

// The in-app alert surface: the one place a raised notification becomes something on screen.
//
// It decides nothing about whether to alert. The core has already applied the master switch, the
// notification level and where the user is looking, and it arrives having written the words — so
// this renders what it is handed. What is left is the toast's own behaviour: which kinds stay until
// they are acted on, when the countdown runs, and where a click goes.
export function NotificationToasts({ processes, onSelectProcess }: NotificationToastsProps) {
  const { dark } = useAppearance();
  const live = useLatestRef(processes);
  const select = useLatestRef(onSelectProcess);
  const [dismissal] = useState(() => createToastDismissal((id) => toast.dismiss(id)));

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    // Acting on an alert both takes the user to the process and starts the toast's countdown —
    // which is the only countdown a toast that stays until acted on ever gets. A process that has
    // left the stack has nowhere to go, so the click does nothing rather than selecting a dead id.
    const open = (process: number, id: ToastId) => {
      if (!live.current.some((candidate) => candidate.id === process)) return;
      select.current(process);
      dismissal.schedule(id, TOAST_LIFETIME_MS);
    };

    const dismiss = (id: ToastId) => {
      dismissal.forget(id);
      toast.dismiss(id);
    };

    onDomainEvent((event) => {
      if (event.type !== "NotificationRaised") return;
      const id = toast.custom(
        (toastId) => (
          <ProcessToast
            kind={event.kind}
            title={event.title}
            body={event.body}
            onOpen={() => open(event.process, toastId)}
            onDismiss={() => dismiss(toastId)}
          />
        ),
        // The countdown is the app's own, so sonner is given a toast that never expires by itself.
        {
          duration: Number.POSITIVE_INFINITY,
          onDismiss: (dismissed) => dismissal.forget(dismissed.id),
        },
      );
      const { dismissAfterMs } = TOAST[event.kind];
      if (dismissAfterMs !== null) dismissal.schedule(id, dismissAfterMs);
      // The sound rides on the alert: the core asked for it, or there is none to play.
      if (event.sound !== null) playBell();
    })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      unlisten?.();
      dismissal.cancel();
    };
  }, [dismissal, live, select]);

  return (
    // Hovering the stack freezes every countdown; leaving it starts each one again from the top.
    <div onPointerEnter={dismissal.pause} onPointerLeave={dismissal.resume}>
      <Toaster theme={dark ? "dark" : "light"} />
    </div>
  );
}
