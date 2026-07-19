import { useState } from "react";
import { ArrowLeft, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { TemplateEditorBody } from "@/components/settings/templates/TemplateEditorBody";
import { TEMPLATE_KIND_LABEL } from "@/lib/templates";
import type { TemplateConflict } from "@/store/useTemplateEditor";
import type { TemplateKind } from "@/domain";

interface TemplateEditorProps {
  kind: TemplateKind;
  name: string;
  initialBody: string;
  initialDescription: string;
  revision: number | null;
  /** Bumped on open/reload so the editor body remounts with fresh content and undo history. */
  mountKey: number;
  conflict: TemplateConflict | null;
  error: string | null;
  onSave: (description: string, body: string) => Promise<void>;
  onReload: () => void;
  onDelete: () => void;
  onBack: () => void;
}

// The drill-in editing surface for one template: a persistent header (back, kind + name, revision,
// delete), the conflict banner, and the remounting editor body. Presentational — the body, the
// revision guard, and every callback arrive as props; the panel owns the read/write. A stale save
// surfaces the conflict banner (the core refused it, so nothing was clobbered) with a Reload; while
// it shows, autosave is paused so the rejected edit is never retried behind the user's back. Delete
// is a deliberate two-step so an authored template is never lost to one mis-click.
export function TemplateEditor({
  kind,
  name,
  initialBody,
  initialDescription,
  revision,
  mountKey,
  conflict,
  error,
  onSave,
  onReload,
  onDelete,
  onBack,
}: TemplateEditorProps) {
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  return (
    <div className="flex flex-col gap-3">
      <header className="flex items-center gap-2">
        <Button variant="ghost" size="sm" onClick={onBack}>
          <ArrowLeft aria-hidden /> Templates
        </Button>
        <div className="flex-1" />
        {revision != null && (
          <span className="font-mono text-[0.6875rem] text-muted-foreground">
            revision {revision}
          </span>
        )}
        {confirmingDelete ? (
          <>
            <Button variant="ghost" size="sm" onClick={() => setConfirmingDelete(false)}>
              Cancel
            </Button>
            <Button variant="destructive" size="sm" onClick={onDelete}>
              Confirm delete
            </Button>
          </>
        ) : (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setConfirmingDelete(true)}
            aria-label="Delete template"
          >
            <Trash2 aria-hidden /> Delete
          </Button>
        )}
      </header>

      <div>
        <p className="text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
          {TEMPLATE_KIND_LABEL[kind]} template
        </p>
        <h2 className="truncate text-[0.9375rem] font-[550] tracking-[-0.005em]">{name}</h2>
      </div>

      {conflict && (
        <div
          role="alert"
          className="flex items-center gap-3 rounded-md border border-status-transition/40 bg-status-transition/10 px-3 py-2 text-[0.8125rem]"
        >
          <span className="min-w-0 flex-1">
            This template changed elsewhere (now at revision {conflict.actual}). Your edits were not
            saved and nothing was overwritten.
          </span>
          <Button variant="outline" size="sm" onClick={onReload}>
            Reload
          </Button>
        </div>
      )}

      {error && (
        <p className="text-[0.8125rem] text-destructive" aria-live="polite">
          {error}
        </p>
      )}

      <TemplateEditorBody
        key={`${kind}:${name}:${mountKey}`}
        initialBody={initialBody}
        initialDescription={initialDescription}
        name={name}
        onSave={onSave}
        paused={conflict != null}
      />
    </div>
  );
}
