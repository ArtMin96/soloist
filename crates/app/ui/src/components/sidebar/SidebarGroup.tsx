import type { ReactNode } from "react";
import { ChevronRight } from "lucide-react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";

interface SidebarGroupProps {
  label: string;
  count: number;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children: ReactNode;
}

// The collapsible shell every sidebar group (process-kind or document) wears: a small
// sentence-case label with a count over a disclosure chevron — deliberately not a tracked-uppercase
// eyebrow. Extracted from the process-kind group so a project's Todos and Scratchpads groups render
// through the same header rather than a copy of it.
export function SidebarGroup({ label, count, open, onOpenChange, children }: SidebarGroupProps) {
  return (
    <Collapsible open={open} onOpenChange={onOpenChange} className="select-none">
      <CollapsibleTrigger className="group/trigger flex w-full items-center gap-1.5 rounded-sm px-1 py-1 text-left outline-none hover:bg-sidebar-accent focus-visible:ring-2 focus-visible:ring-sidebar-ring">
        <ChevronRight
          aria-hidden
          className="size-3 text-muted-foreground transition-transform duration-[var(--dur-control)] ease-spring-settle group-data-[state=open]/trigger:rotate-90"
        />
        <span className="text-[0.6875rem] font-[550] tracking-[0.01em] text-muted-foreground">
          {label}
        </span>
        <span className="ml-auto pr-1 font-mono text-[0.6875rem] tabular-nums text-muted-foreground">
          {count}
        </span>
      </CollapsibleTrigger>
      <CollapsibleContent className="overflow-hidden data-[state=open]:animate-disclose-down data-[state=closed]:animate-disclose-up">
        {children}
      </CollapsibleContent>
    </Collapsible>
  );
}
