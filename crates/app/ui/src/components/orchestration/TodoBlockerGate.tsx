import { ShieldAlert } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

/** The derived blocker relationship shown on todo cards and in todo headers. */
export const TODO_BLOCKERS_ATTRIBUTE = "data-todo-blockers";

interface TodoBlockerGateProps {
  blockerIds: number[];
  variant?: "compact" | "detail";
  className?: string;
}

/** The stable identity token shared by todo cards and detail headers. */
export function TodoIdentity({ id }: { id: number }) {
  return (
    <Badge
      data-todo-ref
      variant="outline"
      className="shrink-0 border-accent bg-accent font-mono tabular-nums text-accent-foreground"
    >
      Todo #{id}
    </Badge>
  );
}

/** A blocker relationship with warning-state treatment and addressable todo targets. */
export function TodoBlockerGate({
  blockerIds,
  variant = "compact",
  className,
}: TodoBlockerGateProps) {
  if (blockerIds.length === 0) return null;

  const [first, ...rest] = blockerIds;
  const targets =
    variant === "detail"
      ? blockerIds.map((id) => `#${id}`).join(" · ")
      : `#${first}${rest.length > 0 ? ` +${rest.length}` : ""}`;
  const full = `Blocked by ${blockerIds.map((id) => `Todo #${id}`).join(", ")}`;

  return (
    <span
      {...{ [TODO_BLOCKERS_ATTRIBUTE]: "" }}
      title={full}
      aria-label={full}
      className={cn(
        "type-label inline-flex min-w-0 items-center gap-1.5 rounded-md border border-warning bg-warning-surface px-2 py-1 text-warning-foreground",
        className,
      )}
    >
      <ShieldAlert aria-hidden className="size-3.5 shrink-0 text-warning" />
      <span className="shrink-0">Blocked by</span>
      <span className="min-w-0 truncate font-mono tabular-nums font-semibold">{targets}</span>
    </span>
  );
}
