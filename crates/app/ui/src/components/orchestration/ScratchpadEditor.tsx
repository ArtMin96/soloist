import { useId } from "react";
import { Link2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import type { ScratchpadConflict } from "@/store/useScratchpadEditor";

interface ScratchpadEditorProps {
  name: string;
  body: string;
  revision: number | null;
  saving: boolean;
  conflict: ScratchpadConflict | null;
  error: string | null;
  onChange: (body: string) => void;
  onSave: () => void;
  onReload: () => void;
  onCopyLink: () => void;
}

// The scratchpad's free-form Markdown body in a single editable field. Presentational: the body and
// every callback arrive as props; the parent owns the read/write and revision guard. A stale save
// surfaces the conflict banner (the core already refused it, nothing was clobbered) with a reload to
// the other edit; validity is the core's call, surfaced as the error line.
export function ScratchpadEditor({
  name,
  body,
  revision,
  saving,
  conflict,
  error,
  onChange,
  onSave,
  onReload,
  onCopyLink,
}: ScratchpadEditorProps) {
  const fieldId = useId();

  return (
    <div className="flex h-full min-w-0 flex-col">
      <header className="flex h-9 shrink-0 items-center gap-2 border-b px-3">
        <h2 className="min-w-0 flex-1 truncate text-[0.9375rem] font-[550] tracking-[-0.005em]">
          {name}
        </h2>
        {revision != null && (
          <span className="shrink-0 font-mono text-[0.6875rem] text-muted-foreground/70">
            revision {revision}
          </span>
        )}
        <Button variant="ghost" size="sm" onClick={onCopyLink}>
          <Link2 aria-hidden /> Copy link
        </Button>
      </header>

      {conflict && (
        <div
          role="alert"
          className="mx-3 mt-3 flex items-center gap-3 rounded-md border border-status-transition/40 bg-status-transition/10 px-3 py-2 text-[0.8125rem]"
        >
          <span className="min-w-0 flex-1">
            This scratchpad changed elsewhere (now at revision {conflict.actual}). Your edits were
            not saved and nothing was overwritten.
          </span>
          <Button variant="outline" size="sm" onClick={onReload}>
            Reload
          </Button>
        </div>
      )}

      <div className="flex min-h-0 flex-1 flex-col p-3">
        <label htmlFor={`${fieldId}-body`} className="sr-only">
          Scratchpad body
        </label>
        <Textarea
          id={`${fieldId}-body`}
          value={body}
          placeholder="Write in Markdown…"
          onChange={(event) => onChange(event.target.value)}
          className="min-h-0 flex-1 resize-none font-mono text-[0.8125rem] leading-relaxed"
        />
      </div>

      <footer className="flex h-11 shrink-0 items-center justify-end gap-3 border-t px-3">
        {error && (
          <span className="min-w-0 flex-1 truncate text-[0.8125rem] text-destructive">{error}</span>
        )}
        <Button size="sm" onClick={onSave} disabled={saving}>
          {saving ? "Saving…" : "Save"}
        </Button>
      </footer>
    </div>
  );
}
