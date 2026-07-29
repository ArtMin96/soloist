import type { CSSProperties } from "react";
import { Toaster as Sonner, type ToasterProps } from "sonner";
import { VISIBLE_TOASTS } from "@/lib/notifications";

// Clears the toolbar, which the stack would otherwise sit on top of.
const OFFSET = { top: "3.5rem", right: "0.75rem" } as const;
const WIDTH = "22rem";
// One step above the app's floating chrome, so an alert is never lost behind a dialog. Replaces
// sonner's own five-figure default, which would sit above everything the OS draws too.
const LAYER = 60;

// The alert stack: top-right, newest in front, capped so a burst of alerts can never grow an
// unbounded column down the window.
//
// Every toast here renders its own card, so sonner's built-in one stays off and only the stack's
// geometry belongs in this file. Its 400 ms slide is replaced by the app's own spring and duration,
// which the rest of the floating chrome already moves on; reduced motion is sonner's own (it drops
// both the transition and the animation).
export function Toaster(props: ToasterProps) {
  return (
    <Sonner
      position="top-right"
      visibleToasts={VISIBLE_TOASTS}
      offset={OFFSET}
      mobileOffset={OFFSET}
      style={{ zIndex: LAYER, "--width": WIDTH } as CSSProperties}
      toastOptions={{ classNames: { toast: "duration-[var(--dur-control)]! ease-spring!" } }}
      {...props}
    />
  );
}
