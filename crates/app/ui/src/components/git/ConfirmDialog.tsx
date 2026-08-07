import type { ReactNode } from "react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

/**
 * The question an action that destroys work asks first.
 *
 * One shape for all of them, because what varies between a discarded change, a deleted branch and an
 * abandoned merge is only the wording — and wording is exactly what each of them has to get right:
 * the description says how far the action reaches, so nobody has to guess.
 */
export function ConfirmDialog({
  open,
  title,
  confirm,
  cancel,
  onConfirm,
  onCancel,
  children,
}: {
  open: boolean;
  title: string;
  /** The verb on the button that goes through with it. */
  confirm: string;
  /** The verb on the button that does not — named for the outcome, never "Cancel". */
  cancel: string;
  onConfirm: () => void;
  onCancel: () => void;
  /** What the action costs, in the product's own words. */
  children: ReactNode;
}) {
  return (
    <AlertDialog open={open} onOpenChange={(next) => !next && onCancel()}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          <AlertDialogDescription>{children}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel onClick={onCancel}>{cancel}</AlertDialogCancel>
          <AlertDialogAction onClick={onConfirm}>{confirm}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
