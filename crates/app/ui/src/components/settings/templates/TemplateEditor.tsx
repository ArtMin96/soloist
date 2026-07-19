import { useState } from "react";
import { ArrowLeft, Trash2 } from "lucide-react";
import { AdvisoryNotice } from "@/components/AdvisoryNotice";
import { Button } from "@/components/ui/button";
import { TemplateEditorBody } from "@/components/settings/templates/TemplateEditorBody";
import { templateScopeHeading } from "@/lib/templates";
import { TemplatePreview } from "@/components/settings/templates/TemplatePreview";
import type { TemplateConflict } from "@/store/useTemplateEditor";
import type { RenderedPrompt, TemplateKind, TemplateScope } from "@/domain";

// Everything the preview needs: the declared placeholders, the values typed against them, and the
// core's latest render of the pair.
export interface TemplatePreviewState {
  placeholders: string[];
  values: Record<string, string>;
  rendered: RenderedPrompt | null;
  error: string | null;
  onValueChange: (placeholder: string, value: string) => void;
}

interface TemplateEditorProps {
  kind: TemplateKind;
  /** Which library the open template lives in — the caption states it, since a name can exist in both. */
  scope: TemplateScope;
  name: string;
  initialBody: string;
  initialDescription: string;
  revision: number | null;
  /** Bumped on open/reload so the editor body remounts with fresh content and undo history. */
  mountKey: number;
  conflict: TemplateConflict | null;
  error: string | null;
  /** The live preview for a renderable kind, or null when this kind is never rendered. */
  preview: TemplatePreviewState | null;
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
  scope,
  name,
  initialBody,
  initialDescription,
  revision,
  mountKey,
  conflict,
  error,
  preview,
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
          {templateScopeHeading(kind, scope)}
        </p>
        <h2 className="truncate text-[0.9375rem] font-[550] tracking-[-0.005em]">{name}</h2>
      </div>

      {conflict && (
        <AdvisoryNotice
          action={
            <Button variant="outline" size="sm" onClick={onReload}>
              Reload
            </Button>
          }
        >
          This template changed elsewhere (now at revision {conflict.actual}). Your edits were not
          saved and nothing was overwritten.
        </AdvisoryNotice>
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
        onSave={onSave}
        paused={conflict != null}
      />

      {preview && (
        <TemplatePreview
          placeholders={preview.placeholders}
          values={preview.values}
          onValueChange={preview.onValueChange}
          rendered={preview.rendered}
          error={preview.error}
        />
      )}
    </div>
  );
}
