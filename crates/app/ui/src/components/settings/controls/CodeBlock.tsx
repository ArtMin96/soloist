import { useEffect, useRef, useState, type ReactNode } from "react";
import { Check, Copy, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

// How long the copied/failed confirmation shows before the button returns to rest.
const COPY_FLASH_MS = 1500;

// The copy button's honest states: a failed write must be as visible as a successful one,
// or the user pastes whatever was already on the clipboard.
type CopyState = "idle" | "copied" | "failed";

const COPY_LABELS: Record<CopyState, string> = {
  idle: "Copy",
  copied: "Copied",
  failed: "Copy failed",
};

// A read-only monospace block for setup snippets, endpoint lists, and other literal values —
// these are data, so they take the mono face on a muted inset well (DESIGN: mono means data;
// flat, hairline-bordered, no card chrome). Horizontally scrollable rather than wrapping so
// aligned text stays aligned. Pass `copy` to overlay a copy-to-clipboard button for content
// meant to be pasted elsewhere (a config snippet); omit it for purely informational blocks.
// The button confirms the outcome either way: a check for a copied snippet, a destructive X
// when the clipboard write failed or the async Clipboard API is unavailable in the webview.
export function CodeBlock({
  children,
  className,
  copy,
}: {
  children: ReactNode;
  className?: string;
  copy?: string;
}) {
  const [copyState, setCopyState] = useState<CopyState>("idle");
  const resetTimer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(resetTimer.current), []);

  const flash = (state: CopyState) => {
    setCopyState(state);
    window.clearTimeout(resetTimer.current);
    resetTimer.current = window.setTimeout(() => setCopyState("idle"), COPY_FLASH_MS);
  };

  const copyText = () => {
    if (copy === undefined) return;
    const clipboard: Clipboard | undefined = navigator.clipboard;
    if (clipboard === undefined) {
      flash("failed");
      return;
    }
    clipboard.writeText(copy).then(
      () => flash("copied"),
      () => flash("failed"),
    );
  };

  const block = (
    <pre
      className={cn(
        "overflow-x-auto rounded-md border border-border bg-muted px-3 py-2 font-mono text-xs leading-relaxed text-foreground",
        className,
      )}
    >
      {children}
    </pre>
  );

  if (copy === undefined) return block;
  return (
    <div className="relative">
      {block}
      <Button
        variant="ghost"
        size="icon"
        onClick={copyText}
        aria-label={COPY_LABELS[copyState]}
        className={cn(
          "absolute right-1.5 top-1.5 size-7",
          copyState === "failed"
            ? "text-destructive hover:text-destructive"
            : "text-muted-foreground hover:text-foreground",
        )}
      >
        {copyState === "copied" ? (
          <Check className="size-3.5" />
        ) : copyState === "failed" ? (
          <X className="size-3.5" />
        ) : (
          <Copy className="size-3.5" />
        )}
      </Button>
      <span className="sr-only" role="status">
        {copyState === "idle" ? "" : COPY_LABELS[copyState]}
      </span>
    </div>
  );
}
