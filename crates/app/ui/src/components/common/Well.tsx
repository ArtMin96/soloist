import type { ComponentProps } from "react";
import { cn } from "@/lib/utils";

/**
 * The inset frame a region draws its content in, as classes — for callers that need the frame on an
 * element the component form cannot be, such as a `<dl>` of label/value pairs or a `<ul>` of rows.
 *
 * The stroke is the load-bearing part. Across the built-in themes `muted` is worth only 1.05–1.21:1
 * against the pane it sits on, so a fill-only step reads as one unbroken field on half of them.
 * `muted` rather than `card` because `card` does not step at all in three of the seven themes —
 * `surfaceRaised` equals `surface` there — which leaves the frame resting on its stroke alone.
 */
export const WELL = "rounded-lg border bg-muted";

/**
 * The inset frame as an element, for regions that own their own container. Merges `className`, so a
 * caller adds layout (`overflow-hidden`, `divide-y`, padding) without restating the frame.
 */
export function Well({ className, ...props }: ComponentProps<"div">) {
  return <div className={cn(WELL, className)} {...props} />;
}
