import { cn } from "@/lib/utils";

/** The handle an autosave footer carries — read by the end-to-end walks, stated once here. */
export const AUTOSAVE_STATUS_ATTRIBUTE = "data-autosave-status";

/**
 * What an autosaving surface has done with the keystrokes it was given: a write in flight, edits
 * held for the next one, or nothing outstanding. Announced politely rather than assertively, since
 * it accompanies typing the reader is already doing.
 */
export function AutosaveStatus({
  saving,
  dirty,
  className,
}: {
  saving: boolean;
  dirty: boolean;
  className?: string;
}) {
  return (
    <span
      {...{ [AUTOSAVE_STATUS_ATTRIBUTE]: "" }}
      aria-live="polite"
      className={cn("type-label text-muted-foreground", className)}
    >
      {saving ? "Saving…" : dirty ? "Unsaved changes" : "Saved"}
    </span>
  );
}
