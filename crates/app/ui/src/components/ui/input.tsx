import * as React from "react"

import { cn } from "@/lib/utils"

function Input({ className, type, ...props }: React.ComponentProps<"input">) {
  return (
    <input
      type={type}
      data-slot="input"
      className={cn(
        "h-8 w-full min-w-0 rounded-md border border-border bg-input px-2.5 py-1 text-base transition-[color,background-color,border-color,box-shadow] duration-[var(--dur-fast)] ease-spring outline-none file:inline-flex file:h-6 file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-foreground placeholder:text-placeholder focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:cursor-not-allowed disabled:bg-muted disabled:opacity-50 aria-invalid:border-error aria-invalid:ring-2 aria-invalid:ring-error/30 md:text-sm",
        className
      )}
      {...props}
    />
  )
}

export { Input }
