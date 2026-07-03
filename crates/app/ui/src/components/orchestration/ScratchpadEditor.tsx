import { useId } from "react";
import { Link2 } from "lucide-react";
import { FieldList } from "@/components/orchestration/FieldList";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import type { ScratchpadForm } from "@/store/scratchpadForm";
import type { ScratchpadConflict } from "@/store/useScratchpadEditor";

interface ScratchpadEditorProps {
  name: string;
  form: ScratchpadForm;
  revision: number | null;
  saving: boolean;
  conflict: ScratchpadConflict | null;
  error: string | null;
  onChange: (form: ScratchpadForm) => void;
  onSave: () => void;
  onReload: () => void;
  onCopyLink: () => void;
}

// The structured editor over a scratchpad's disciplined document — a field per section, not a free
// Markdown textarea, so every scratchpad records the same shape. Presentational: the form and every
// callback arrive as props; the parent owns the read/write and revision guard. A stale save surfaces
// the conflict banner (the core already refused it, nothing was clobbered) with a reload to the
// other edit; validity is the core's call, surfaced as the error line.
export function ScratchpadEditor({
  name,
  form,
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
  const set = <K extends keyof ScratchpadForm>(key: K, value: ScratchpadForm[K]) =>
    onChange({ ...form, [key]: value });

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

      <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-auto p-3">
        <label htmlFor={`${fieldId}-objective`} className="flex flex-col gap-1.5">
          <span className="text-[0.6875rem] font-[550] text-muted-foreground">Objective</span>
          <Input
            id={`${fieldId}-objective`}
            value={form.objective}
            placeholder="What this scratchpad is for"
            onChange={(event) => set("objective", event.target.value)}
            className="h-7 text-[0.8125rem]"
          />
        </label>

        <label htmlFor={`${fieldId}-context`} className="flex flex-col gap-1.5">
          <span className="text-[0.6875rem] font-[550] text-muted-foreground">Context</span>
          <Textarea
            id={`${fieldId}-context`}
            value={form.context}
            placeholder="The background a reader needs"
            onChange={(event) => set("context", event.target.value)}
            className="min-h-16 text-[0.8125rem]"
          />
        </label>

        <FieldList
          label="Plan"
          items={form.plan}
          placeholder="A step"
          onChange={(items) => set("plan", items)}
        />
        <FieldList
          label="Acceptance criteria"
          items={form.acceptance_criteria}
          placeholder="A condition for done"
          onChange={(items) => set("acceptance_criteria", items)}
        />
        <FieldList
          label="Risks"
          items={form.risks}
          placeholder='A risk (or "none identified")'
          onChange={(items) => set("risks", items)}
        />

        <label htmlFor={`${fieldId}-status`} className="flex flex-col gap-1.5">
          <span className="text-[0.6875rem] font-[550] text-muted-foreground">Status</span>
          <Input
            id={`${fieldId}-status`}
            value={form.status}
            placeholder="e.g. in progress"
            onChange={(event) => set("status", event.target.value)}
            className="h-7 text-[0.8125rem]"
          />
        </label>

        <label htmlFor={`${fieldId}-notes`} className="flex flex-col gap-1.5">
          <span className="text-[0.6875rem] font-[550] text-muted-foreground">
            Notes (optional)
          </span>
          <Textarea
            id={`${fieldId}-notes`}
            value={form.notes}
            placeholder="Anything else, in Markdown"
            onChange={(event) => set("notes", event.target.value)}
            className="min-h-16 text-[0.8125rem]"
          />
        </label>
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
