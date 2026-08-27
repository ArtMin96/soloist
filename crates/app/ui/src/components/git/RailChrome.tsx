import type { ReactNode } from "react";
import { FoldVerticalIcon, UnfoldVerticalIcon } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { Empty, EmptyDescription, EmptyHeader } from "@/components/ui/empty";

const EXPAND_FOLDERS_LABEL = "Expand all folders";
const COLLAPSE_FOLDERS_LABEL = "Collapse all folders";

/** What the last refused change said, stated where the change was asked for. */
export function RailError({ message }: { message: string }) {
  return (
    <p
      role="alert"
      className="type-body shrink-0 border-t border-sidebar-border px-3 py-2 text-destructive"
    >
      {message}
    </p>
  );
}

/** A compact tree control for either tab; it never changes the version-control rail. */
export function TreeExpansionButton({
  expanded,
  onClick,
}: {
  expanded: boolean;
  onClick: () => void;
}) {
  const label = expanded ? COLLAPSE_FOLDERS_LABEL : EXPAND_FOLDERS_LABEL;
  return (
    <IconButton
      label={label}
      icon={expanded ? <FoldVerticalIcon /> : <UnfoldVerticalIcon />}
      onClick={onClick}
    />
  );
}

export function RailMessage({ children }: { children: ReactNode }) {
  return <p className="type-body text-muted-foreground">{children}</p>;
}

/** The quiet line a tab shows when it has nothing in it. */
export function RailEmpty({ children }: { children: ReactNode }) {
  return (
    <Empty className="h-full">
      <EmptyHeader>
        <EmptyDescription>{children}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}
