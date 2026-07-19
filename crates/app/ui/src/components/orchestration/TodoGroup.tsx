import type { ReactNode } from "react";
import { ChevronRight } from "lucide-react";
import { Collapsible } from "radix-ui";

interface TodoGroupProps {
  label: string;
  count: number;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children: ReactNode;
}

// One scratchpad's section of the to-do board. The header is the same small sentence-case label
// with a monospace count the sidebar's process groups wear — deliberately not a tracked-uppercase
// eyebrow, and deliberately the same vocabulary, so a grouped list reads identically wherever it
// appears in the app. Every group looks alike, including the unlinked one: a todo that derives from
// no scratchpad is ordinary, so its section carries no warning colour and no nudge to fill it in.
//
// Purely a wrapper: the rows inside are unchanged, which is what keeps the collapsed row the same
// element the board, the keyboard, and the end-to-end walks already address.
export function TodoGroup({ label, count, open, onOpenChange, children }: TodoGroupProps) {
  return (
    <Collapsible.Root open={open} onOpenChange={onOpenChange} className="select-none">
      <Collapsible.Trigger
        data-todo-group
        className="group/todogroup flex w-full items-center gap-1.5 rounded-sm px-2 py-1 text-left outline-none hover:bg-sidebar-accent focus-visible:ring-2 focus-visible:ring-sidebar-ring"
      >
        <ChevronRight
          aria-hidden
          className="size-3 text-muted-foreground transition-transform duration-[var(--dur-control)] ease-spring-settle group-data-[state=open]/todogroup:rotate-90"
        />
        <span className="min-w-0 truncate text-[0.6875rem] font-[550] tracking-[0.01em] text-muted-foreground">
          {label}
        </span>
        <span className="ml-auto pr-1 font-mono text-[0.6875rem] tabular-nums text-muted-foreground/70">
          {count}
        </span>
      </Collapsible.Trigger>
      <Collapsible.Content className="overflow-hidden data-[state=open]:animate-disclose-down data-[state=closed]:animate-disclose-up">
        {children}
      </Collapsible.Content>
    </Collapsible.Root>
  );
}
