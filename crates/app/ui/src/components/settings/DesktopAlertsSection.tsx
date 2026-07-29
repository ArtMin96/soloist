import { useCallback, useEffect, useRef, useState } from "react";
import { sendTestNotification } from "@/api";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { Button } from "@/components/ui/button";
import { advertisedSupport, NOTIFIER_STATUS } from "@/lib/notifications";
import { cn } from "@/lib/utils";
import { useNotifierStatus } from "@/store/useNotifierStatus";

// How long the sample alert's confirmation stays before the row goes quiet again — long enough to
// read after looking away at the desktop, short enough not to read as a lasting state.
const SENT_CONFIRMATION_MS = 4000;

// Whether alerts can reach the desktop, and a way to try one.
//
// It sits apart from the preferences above it because it is not a preference: it reports what this
// machine is doing, which no switch here can change. Keeping them apart is what stops a dead
// notification service reading as "Soloist is off" — the master switch and the sound still work,
// because an in-app toast never touches the desktop at all.
//
// Nothing here ever claims an alert arrived. Showing a desktop notification is fire-and-forget: the
// result is discarded, so delivery is unobservable, and this row exists precisely because it is.
export function DesktopAlertsSection() {
  const { status, recheck } = useNotifierStatus();
  const [sent, setSent] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => () => clearTimeout(timer.current), []);

  const sendTest = useCallback(() => {
    void sendTestNotification().catch(() => {});
    // "Sent", never "Delivered" — the app handed the alert over and that is the whole of what it
    // knows. Confirming more than that would be inventing a fact.
    setSent(true);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setSent(false), SENT_CONFIRMATION_MS);
  }, []);

  const display = NOTIFIER_STATUS[status.type];

  return (
    <SettingsSection
      title="Desktop alerts"
      description="Soloist hands each alert to your desktop's notification service, which decides how it looks and how long it stays. That handover is one-way — nothing reports back whether you saw it — so this shows whether the service is there to receive them."
    >
      <div className="flex items-center justify-between gap-6 py-3">
        <div className="flex min-w-0 items-start gap-2">
          <span aria-hidden className={cn("text-[0.8125rem] leading-5", display.toneClass)}>
            {display.glyph}
          </span>
          <div className="min-w-0">
            <div className="text-[0.8125rem] text-foreground">{display.label}</div>
            <p className="mt-0.5 max-w-[42ch] text-xs text-muted-foreground">{display.detail}</p>
            {display.check && (
              <p className="mt-1 max-w-[42ch] text-xs text-muted-foreground">{display.check}</p>
            )}
            {status.type === "available" && (
              <>
                <p className="mt-1 max-w-[42ch] text-xs text-muted-foreground">
                  {advertisedSupport(status.capabilities)}
                </p>
                <p className="mt-1 truncate font-mono text-[0.6875rem] text-muted-foreground">
                  {status.server} {status.version}
                </p>
              </>
            )}
          </div>
        </div>
        <Button variant="ghost" size="sm" className="shrink-0" onClick={recheck}>
          Check again
        </Button>
      </div>

      <div className="flex items-center justify-between gap-6 py-3">
        <div className="min-w-0">
          <div className="text-[0.8125rem] text-foreground">Sample alert</div>
          <p className="mt-0.5 max-w-[42ch] text-xs text-muted-foreground">
            Sends one alert that looks and sounds like a real one, so you can hear the sound you
            picked before something goes wrong.
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <span role="status" className="text-xs text-muted-foreground">
            {sent ? "Sent" : ""}
          </span>
          <Button variant="outline" size="sm" onClick={sendTest}>
            Send test
          </Button>
        </div>
      </div>
    </SettingsSection>
  );
}
