import { useEffect, useState, type KeyboardEvent } from "react";
import { Check, CircleCheck, TriangleAlert } from "lucide-react";
import { DiagramCanvas } from "@/components/mermaid/DiagramCanvas";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { MERMAID_RENDER_DEBOUNCE_MS } from "@/lib/mermaid";
import { cn } from "@/lib/utils";

interface DiagramBodyProps {
  /** The controlled draft source — the editor is authoritative, so the persisted value is never echoed here. */
  source: string;
  /** Records an edit: the panel updates the draft and schedules an autosave. */
  onChange: (next: string) => void;
  /** True while a save is in flight. */
  saving: boolean;
  /** True when the latest edit has not been persisted. */
  dirty: boolean;
  /** Persist the pending edit now — Cmd/Ctrl+S, blur, or the Save button. */
  onFlush: () => void;
}

// The editing surface for a diagram: a monospace Mermaid source editor beside a live preview. A diagram
// is pure Mermaid source, so the left half is a plain textarea (autosaved by the parent) rather than a
// rich-text editor; the right half renders the debounced draft through the shared pan-zoom canvas and
// reports validity. Side-by-side when the panel is wide, stacked when it is narrow. The textarea is
// controlled by the draft the parent owns, so a theme override applied from the header appears here at
// once; the persisted value is never pushed back, so the caret never jumps.
export function DiagramBody({ source, onChange, saving, dirty, onFlush }: DiagramBodyProps) {
  // The preview renders a debounced copy of the draft, so a burst of keystrokes coalesces into one
  // re-render rather than re-parsing on every character.
  const [previewSource, setPreviewSource] = useState(source);
  const [valid, setValid] = useState<boolean | null>(null);

  useEffect(() => {
    const id = setTimeout(() => setPreviewSource(source), MERMAID_RENDER_DEBOUNCE_MS);
    return () => clearTimeout(id);
  }, [source]);

  function onKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
      event.preventDefault();
      onFlush();
    }
  }

  const status = saving ? "Saving…" : dirty ? "Unsaved changes" : "Saved";

  return (
    <div className="@container/diagram flex min-h-0 flex-1 flex-col gap-2 p-3">
      <div className="flex min-h-0 flex-1 flex-col gap-2 @2xl/diagram:flex-row">
        <Textarea
          value={source}
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={onKeyDown}
          onBlur={onFlush}
          spellCheck={false}
          aria-label="Diagram source"
          className="field-sizing-fixed min-h-40 flex-1 resize-none font-mono text-[0.8125rem] leading-relaxed @2xl/diagram:min-h-0"
        />
        <DiagramCanvas
          source={previewSource}
          onParse={setValid}
          className="min-h-40 flex-1 @2xl/diagram:min-h-0"
        />
      </div>

      <footer className="flex shrink-0 items-center gap-3">
        <Validity valid={valid} />
        <span
          className="text-[0.6875rem] text-muted-foreground"
          aria-live="polite"
          data-autosave-status
        >
          {status}
        </span>
        <div className="flex-1" />
        <Button size="sm" onClick={onFlush} disabled={saving || !dirty} data-diagram-save>
          {saving ? "Saving…" : <Check aria-hidden />}
          {saving ? "" : "Save"}
        </Button>
      </footer>
    </div>
  );
}

// The live parse state of the drafted source — icon plus label, never colour alone, so it survives a
// grayscale screenshot and colour blindness. Nothing shows until the first render settles.
function Validity({ valid }: { valid: boolean | null }) {
  if (valid === null) return null;
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 text-[0.6875rem]",
        valid ? "text-muted-foreground" : "text-destructive",
      )}
      aria-live="polite"
    >
      {valid ? (
        <CircleCheck className="size-3" aria-hidden />
      ) : (
        <TriangleAlert className="size-3" aria-hidden />
      )}
      {valid ? "Valid" : "Invalid"}
    </span>
  );
}
