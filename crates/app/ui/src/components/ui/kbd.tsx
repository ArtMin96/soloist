import * as React from "react"

import { cn } from "@/lib/utils"

// A single key cap for a chord token. Mono (the key is data), on a muted inset with a hairline —
// the same treatment as the picker's key hints, factored into one place.
function Kbd({ className, ...props }: React.ComponentProps<"kbd">) {
  return (
    <kbd
      data-slot="kbd"
      className={cn(
        "inline-flex h-5 min-w-5 items-center justify-center rounded border border-border bg-muted px-1.5 font-mono text-[0.6875rem] font-medium text-muted-foreground",
        className
      )}
      {...props}
    />
  )
}

export { Kbd }
