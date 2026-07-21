import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

// The padding a settings panel sits in whichever width it takes — one value, so the standard column
// and a drill-in builder never drift apart.
const PANEL_PADDING = "px-6 py-6";

// The standard settings panel: a centered, reading-width column. Every panel that does not manage
// its own width renders inside one.
export function SettingsColumn({ children }: { children: ReactNode }) {
  return <div className={cn("mx-auto max-w-2xl", PANEL_PADDING)}>{children}</div>;
}

// The full-width variant a panel drills into for a builder surface. `h-full min-h-0` is the
// load-bearing part: it gives the flex chain below a bounded height, which is what lets a rich
// editor and its preview fill the panel and scroll independently instead of growing it past the
// window.
export function SettingsBuilderColumn({ children }: { children: ReactNode }) {
  return <div className={cn("flex h-full min-h-0 flex-col", PANEL_PADDING)}>{children}</div>;
}
