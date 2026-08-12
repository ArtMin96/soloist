import * as React from "react"

import { cn } from "@/lib/utils"

function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "field-sizing-content min-h-16 w-full rounded-md border border-border bg-input px-2.5 py-1.5 text-base transition-[color,background-color,border-color,box-shadow] duration-[var(--dur-fast)] ease-spring outline-none placeholder:text-placeholder focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:cursor-not-allowed disabled:bg-muted disabled:opacity-50 aria-invalid:border-error aria-invalid:ring-2 aria-invalid:ring-error/30 md:text-sm",
        className
      )}
      {...props}
    />
  )
}

export { Textarea }
