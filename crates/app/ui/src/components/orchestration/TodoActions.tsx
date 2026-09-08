import { Check, Link2, Pencil } from "lucide-react";
import { DetailActions } from "@/components/common/DetailActions";
import { LABEL_FLOOR, SQUARE_FLOOR } from "@/components/common/DetailPane";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";

interface TodoActionsProps {
  /** A completed todo offers no Complete — the action is spent, not merely unavailable. */
  done: boolean;
  busy: boolean;
  onComplete: () => void;
  onCopyLink: () => void;
  onStartEdit: () => void;
}

/**
 * The todo detail header's action cluster: Edit and Copy link as the secondary set, with Complete
 * as the primary that finishes the todo.
 */
export function TodoActions({ done, busy, onComplete, onCopyLink, onStartEdit }: TodoActionsProps) {
  return (
    <DetailActions
      actions={[
        { icon: Pencil, label: "Edit", menuLabel: "Edit todo", onSelect: onStartEdit },
        {
          icon: Link2,
          label: "Copy link to todo",
          menuLabel: "Copy link to todo",
          onSelect: onCopyLink,
        },
      ]}
      menuTooltip="More todo actions"
      // `undefined`, never `false`: the cluster draws its separator for any primary that is not
      // nullish, and on a done todo that rule would point at nothing.
      primary={
        done ? undefined : (
          <Button
            size="sm"
            onClick={onComplete}
            disabled={busy}
            aria-busy={busy}
            className={SQUARE_FLOOR}
          >
            {/* `aria-hidden` suppresses the spinner's built-in "Loading" so the state is announced
                once, by `aria-busy`. The text swap remains the carrier when reduced motion freezes
                the spin. */}
            {busy ? (
              <Spinner aria-hidden data-icon="inline-start" />
            ) : (
              <Check aria-hidden data-icon="inline-start" />
            )}
            <span className={LABEL_FLOOR}>{busy ? "Completing…" : "Complete"}</span>
          </Button>
        )
      }
    />
  );
}
