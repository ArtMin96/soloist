import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

// A read-only monospace block for setup snippets, endpoint lists, and other literal values —
// these are data, so they take the mono face on a muted inset well (DESIGN: mono means data;
// flat, hairline-bordered, no card chrome). Horizontally scrollable rather than wrapping so
// aligned text stays aligned.
export function CodeBlock({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <pre
      className={cn(
        "overflow-x-auto rounded-md border border-border bg-muted px-3 py-2 font-mono text-xs leading-relaxed text-foreground",
        className,
      )}
    >
      {children}
    </pre>
  );
}
