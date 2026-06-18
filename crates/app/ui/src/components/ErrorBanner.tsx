import { X } from "lucide-react";
import { Button } from "@/components/ui/button";

// A dismissible banner surfacing the core's error verbatim. The UI renders core errors;
// it never invents its own — every command returns a string the user can read.
export function ErrorBanner({ message, onDismiss }: { message: string; onDismiss: () => void }) {
  return (
    <div
      role="alert"
      className="flex items-center gap-2 border-b border-destructive/30 bg-destructive/10 px-3 py-1.5 text-xs text-destructive"
    >
      <span className="min-w-0 flex-1 truncate">{message}</span>
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label="Dismiss"
        className="text-destructive hover:bg-destructive/15"
        onClick={onDismiss}
      >
        <X />
      </Button>
    </div>
  );
}
