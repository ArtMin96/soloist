import { AutosaveStatus } from "@/components/editor/AutosaveStatus";
import { LazyRichTextEditor } from "@/components/editor/LazyRichTextEditor";
import { useAutosave } from "@/components/editor/useAutosave";
import { RevisionConflictNotice } from "@/components/RevisionConflictNotice";
import type { SaveOutcome } from "@/store/saveOutcome";
import type { ScratchpadConflict } from "@/store/useScratchpadEditor";

/**
 * Names the document being edited. Distinct from the detail pane's own handle, which says only that
 * a scratchpad is open: this one is present exactly while it is being written to.
 */
export const SCRATCHPAD_EDITOR_ATTRIBUTE = "data-scratchpad-editor";

/** The editor's accessible name, shared by the surface and whoever addresses it. */
export const SCRATCHPAD_BODY_LABEL = "Scratchpad body";

interface ScratchpadEditorProps {
  /** The scratchpad's raw name handle — the document this surface is writing to. */
  name: string;
  /** The Markdown to seed the editor with; read once, since the parent remounts per open and reload. */
  initialBody: string;
  /** A concurrent write moved the scratchpad past the opened revision, or null. */
  conflict: ScratchpadConflict | null;
  /** A non-conflict refusal of a save (an invalid document), or null. */
  error: string | null;
  /** Persists the Markdown body revision-guarded — routed to the core. */
  onSave: (markdown: string) => Promise<SaveOutcome>;
  /** Reload the scratchpad fresh, adopting the concurrent write and discarding local edits. */
  onReload: () => void;
  /** Every keystroke's Markdown, so the pane can export or copy text that is not saved yet. */
  onBodyChange: (markdown: string) => void;
}

/**
 * The edit surface for one scratchpad: the rich-text body plus its autosave. Edits are debounced by
 * `useAutosave` and flush on blur, Cmd/Ctrl+S, and unmount — never echoed back into the editor, so
 * the caret never jumps. There is deliberately no Save control: leaving edit mode unmounts this
 * surface and the unmount flush is what persists the last keystrokes.
 *
 * A stale save is refused by the core's revision guard, and the parent passes the `conflict` it
 * learned from the re-read; that pauses autosave so the rejected edit is never retried behind the
 * user's back, and offers the Reload that resolves it. Nothing was overwritten either way.
 */
export function ScratchpadEditor({
  name,
  initialBody,
  conflict,
  error,
  onSave,
  onReload,
  onBodyChange,
}: ScratchpadEditorProps) {
  const autosave = useAutosave({ onSave, paused: conflict != null });

  return (
    <div {...{ [SCRATCHPAD_EDITOR_ATTRIBUTE]: name }} className="flex flex-col gap-2">
      {conflict && (
        <RevisionConflictNotice
          subject="scratchpad"
          revision={conflict.actual}
          onReload={onReload}
        />
      )}

      {/* The conflict banner already says why the write did not land, so the two never stack. */}
      {error && !conflict && (
        <p className="type-body text-destructive" aria-live="polite">
          {error}
        </p>
      )}

      <LazyRichTextEditor
        initialMarkdown={initialBody}
        ariaLabel={SCRATCHPAD_BODY_LABEL}
        outline
        onChange={(markdown) => {
          onBodyChange(markdown);
          autosave.push(markdown);
        }}
        onSaveShortcut={autosave.flush}
        onBlur={autosave.flush}
      />

      <footer className="flex items-center">
        <AutosaveStatus saving={autosave.saving} dirty={autosave.dirty} />
      </footer>
    </div>
  );
}
