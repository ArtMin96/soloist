import { useState } from "react";
import { Check, Copy, RotateCw } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { ProjectSettingsPage } from "@/domain";

// The project's directory, configuration validity, and live process counts — the read-only
// orientation tab. Opening the folder, a terminal, or the editor from here are I9 follow-ups and
// are intentionally absent; this tab only reports.
export function OverviewSection({
  page,
  onReload,
}: {
  page: ProjectSettingsPage;
  onReload: () => void;
}) {
  const [copied, setCopied] = useState(false);

  const copyPath = () => {
    void navigator.clipboard
      .writeText(page.root)
      .then(() => {
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1500);
      })
      .catch(() => {});
  };

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col gap-1.5">
        <Caption>Directory</Caption>
        <div className="flex items-center gap-2">
          <code
            title={page.root}
            className="min-w-0 flex-1 truncate rounded-md border border-border bg-muted px-2.5 py-1.5 font-mono text-xs text-foreground"
          >
            {page.root}
          </code>
          <Button variant="outline" size="sm" onClick={copyPath} aria-label="Copy path">
            {copied ? <Check /> : <Copy />}
            {copied ? "Copied" : "Copy path"}
          </Button>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-x-8 gap-y-3">
        <div className="flex items-center gap-2.5">
          <Caption>Configuration</Caption>
          {page.config.valid ? (
            <Badge variant="outline" className="gap-1 border-status-running/40 text-status-running">
              <Check />
              Valid
            </Badge>
          ) : (
            <Badge variant="destructive">Invalid</Badge>
          )}
          <Button variant="ghost" size="sm" onClick={onReload} aria-label="Refresh">
            <RotateCw />
            Refresh
          </Button>
        </div>

        <div className="flex items-center gap-2.5">
          <Caption>Processes</Caption>
          <span className="font-mono text-xs tabular-nums text-muted-foreground">
            {page.running} running &middot; {page.total} total
          </span>
        </div>
      </div>

      {!page.config.valid && page.config.error && (
        <p className="max-w-[60ch] text-xs leading-relaxed text-destructive">{page.config.error}</p>
      )}
    </div>
  );
}

// The DESIGN label treatment for the inline field captions on this tab.
function Caption({ children }: { children: string }) {
  return (
    <span className="text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
      {children}
    </span>
  );
}
