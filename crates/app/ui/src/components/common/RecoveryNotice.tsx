import { TriangleAlert } from "lucide-react";
import { Alert, AlertAction, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

interface RecoveryNoticeProps {
  /** What went wrong, in the reader's terms ("Diff view ran into a problem."). */
  message: string;
  /** Re-runs whatever failed; without it the notice states the fault and offers nothing. */
  onRetry?: () => void;
}

/**
 * The one way the app says a region failed: a contained destructive alert with an optional retry.
 * A caught render error and a read that would not resolve are the same thing to the person looking
 * at the pane, so they get the same notice rather than each surface inventing its own treatment.
 *
 * Centred and width-capped because it stands in for one region's content, not the whole app's.
 */
export function RecoveryNotice({ message, onRetry }: RecoveryNoticeProps) {
  return (
    <div className="flex w-full justify-center p-3">
      <Alert variant="destructive" className="max-w-sm">
        <TriangleAlert aria-hidden />
        <AlertDescription>{message}</AlertDescription>
        {onRetry != null && (
          <AlertAction className="top-1/2 -translate-y-1/2">
            <Button variant="outline" size="sm" onClick={onRetry}>
              Try again
            </Button>
          </AlertAction>
        )}
      </Alert>
    </div>
  );
}
