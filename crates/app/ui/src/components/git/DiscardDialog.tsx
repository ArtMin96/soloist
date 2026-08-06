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

/** What is about to be thrown away: a whole path's change, or one hunk of it. */
export interface Discardable {
  path: string;
  /** True when only one hunk goes, so the question names what it really costs. */
  hunk: boolean;
}

const TITLE = "Discard this change?";
const CONFIRM = "Discard";
const CANCEL = "Keep it";

/**
 * The question every discard asks first. A discard is the one action here that destroys work, so
 * it is never one click away — and the wording says exactly how far it reaches, which is as far
 * as the index and no further.
 */
export function DiscardDialog({
  discarding,
  onConfirm,
  onCancel,
}: {
  /** What is about to go, or null when nothing is being discarded. */
  discarding: Discardable | null;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const what = discarding?.hunk === true ? "This hunk of" : "The unstaged changes to";
  return (
    <AlertDialog open={discarding !== null} onOpenChange={(open) => !open && onCancel()}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{TITLE}</AlertDialogTitle>
          <AlertDialogDescription>
            {what} <span className="font-mono">{discarding?.path}</span> will go back to what the
            index holds. Nothing staged and nothing committed is affected, and this cannot be
            undone.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel onClick={onCancel}>{CANCEL}</AlertDialogCancel>
          <AlertDialogAction onClick={onConfirm}>{CONFIRM}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
