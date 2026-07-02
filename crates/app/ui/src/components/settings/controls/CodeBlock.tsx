import { useState, type ReactNode } from "react";
import { Check, Copy } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

// A read-only monospace block for setup snippets, endpoint lists, and other literal values —
// these are data, so they take the mono face on a muted inset well (DESIGN: mono means data;
// flat, hairline-bordered, no card chrome). Horizontally scrollable rather than wrapping so
// aligned text stays aligned. Pass `copy` to overlay a copy-to-clipboard button for content
// meant to be pasted elsewhere (a config snippet); omit it for purely informational blocks.
export function CodeBlock({
  children,
  className,
  copy,
}: {
  children: ReactNode;
  className?: string;
  copy?: string;
}) {
  const [copied, setCopied] = useState(false);

  const copyText = () => {
    if (copy === undefined) return;
    void navigator.clipboard
      .writeText(copy)
      .then(() => {
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1500);
      })
      .catch(() => {});
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
        aria-label="Copy"
        className="absolute right-1.5 top-1.5 size-7 text-muted-foreground hover:text-foreground"
      >
        {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
      </Button>
    </div>
  );
}
