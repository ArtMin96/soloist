import { useState } from "react";
import { Archive, ArchiveRestore } from "lucide-react";
import { RevisionConflictNotice } from "@/components/RevisionConflictNotice";
import { Button } from "@/components/ui/button";
import { DiagramBody } from "@/components/orchestration/DiagramBody";
import { DiagramTitle } from "@/components/orchestration/DiagramTitle";
import { DiagramToolbar } from "@/components/orchestration/DiagramToolbar";
import { useAutosave } from "@/components/editor/useAutosave";
import { humanizeName } from "@/lib/humanize";
import type { DiagramConflict } from "@/store/useDiagramEditor";

interface DiagramEditorProps {
  name: string;
  initialSource: string;
  revision: number | null;
  conflict: DiagramConflict | null;
  error: string | null;
  /** Whether the open diagram is archived — flips the header control between Archive and Restore. */
  archived: boolean;
  onSave: (source: string) => Promise<void>;
  onReload: () => void;
  /** Archives the open diagram, or restores it when already archived. */
  onArchive: () => void;
  /** Renames the open diagram; rejects with the core's refusal so the field can surface it. */
  onRename: (to: string) => Promise<void>;
}

// The diagram's editing surface: a persistent header (the renamable title, the raw handle when it reads
// differently, revision, the theme/export/fullscreen toolbox, Archive⇄Restore), the conflict banner,
// and the split source-editor/preview body. This component is remounted per open/reload (the panel keys
// it by `mountKey`), so its draft and autosave are per-load and a reload starts clean; a rename does not
// change `mountKey`, so it keeps the draft and undo history intact. The draft is owned here — the header
// toolbox (a theme override) and the body both read and write it — and the persisted value is never
// pushed back, so autosave never jumps the caret. A stale save surfaces the conflict banner (the core
// already refused it, so nothing was clobbered); while it shows, autosave pauses so the rejected edit is
// never retried behind the user's back.
export function DiagramEditor({
  name,
  initialSource,
  revision,
  conflict,
  error,
  archived,
  onSave,
  onReload,
  onArchive,
  onRename,
}: DiagramEditorProps) {
  const [draft, setDraft] = useState(initialSource);
  const [toolboxError, setToolboxError] = useState<string | null>(null);
  const autosave = useAutosave({ onSave, paused: conflict != null });

  const change = (next: string) => {
    setDraft(next);
    autosave.push(next);
  };

  // The handle earns its place only when the title no longer reads as it — a name the user already
  // wrote is its own handle, and printing it twice would be noise.
  const handle = humanizeName(name) === name ? null : name;

  return (
    <div className="flex h-full min-w-0 flex-col">
      <header className="flex h-9 shrink-0 items-center gap-2 border-b px-3">
        <DiagramTitle name={name} onRename={onRename} />
        {handle && (
          <span
            className="type-label max-w-[12rem] shrink-0 truncate font-mono text-muted-foreground"
            title={`Handle: ${handle}`}
          >
            {handle}
          </span>
        )}
        {revision != null && (
          <span className="type-label shrink-0 font-mono tabular-nums text-muted-foreground">
            revision {revision}
          </span>
        )}
        <DiagramToolbar
          name={name}
          source={draft}
          onSourceChange={change}
          onError={setToolboxError}
        />
        <Button variant="ghost" size="sm" onClick={onArchive}>
          {archived ? (
            <>
              <ArchiveRestore aria-hidden /> Restore
            </>
          ) : (
            <>
              <Archive aria-hidden /> Archive
            </>
          )}
        </Button>
      </header>

      {conflict && (
        <RevisionConflictNotice
          className="mx-3 mt-3"
          subject="diagram"
          revision={conflict.actual}
          onReload={onReload}
        />
      )}

      {(error ?? toolboxError) && (
        <p className="mx-3 mt-3 text-[0.8125rem] text-destructive" aria-live="polite">
          {error ?? toolboxError}
        </p>
      )}

      <DiagramBody
        source={draft}
        onChange={change}
        saving={autosave.saving}
        dirty={autosave.dirty}
        onFlush={autosave.flush}
      />
    </div>
  );
}
