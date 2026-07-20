import { useRef, useState } from "react";
import { Check } from "lucide-react";
import { RevisionConflictNotice } from "@/components/RevisionConflictNotice";
import { Button } from "@/components/ui/button";
import { TodoDocFields } from "@/components/orchestration/TodoDocFields";
import { useAutosave } from "@/components/editor/useAutosave";
import type { ScratchpadSummary, TodoDoc, TodoStatus } from "@/domain";

export interface TodoConflict {
  actual: number;
}

interface TodoEditorProps {
  /** The document to seed the fields with. The parent remounts this component per open/reload. */
  initial: TodoDoc;
  /** The scratchpad the todo derives from at open, or null. */
  initialScratchpad: number | null;
  /** The project's scratchpads, offered as the documents this todo may derive from. */
  scratchpads: ScratchpadSummary[];
  /** A concurrent write moved the todo past the opened revision, or null. */
  conflict: TodoConflict | null;
  /** A non-conflict failure (invalid document, blocked→done gate), or null. */
  error: string | null;
  /** Persists the whole document and its association revision-guarded — routed to the core. */
  onSave: (doc: TodoDoc, scratchpad: number | null) => Promise<void>;
  /** Reload the todo fresh, adopting the concurrent write and discarding local edits. */
  onReload: () => void;
  /** Leave edit mode (edits already autosaved). */
  onDone: () => void;
}

// The inline edit surface for one todo: the title/body/status fields plus their autosave. Edits
// stream out as the whole document and are debounced by `useAutosave`, flushing on blur, Cmd/Ctrl+S,
// Done, or unmount — never echoed back into the editor, so the caret never jumps. A stale save is
// refused by the core's revision guard; the board detects that from the live revision and passes a
// `conflict`, which pauses autosave and offers a Reload (nothing was overwritten). The parent keys
// this component by the editor's mount key, so a reload remounts it with fresh content and a clean
// undo history.
export function TodoEditor({
  initial,
  initialScratchpad,
  scratchpads,
  conflict,
  error,
  onSave,
  onReload,
  onDone,
}: TodoEditorProps) {
  const [title, setTitle] = useState(initial.title);
  const [status, setStatus] = useState<TodoStatus>(initial.status);
  const [scratchpad, setScratchpad] = useState<number | null>(initialScratchpad);
  const bodyRef = useRef(initial.body);
  const autosave = useAutosave({
    onSave: (body: string) => onSave({ title, body, status }, scratchpad),
    paused: conflict != null,
  });

  const done = () => {
    autosave.flush();
    onDone();
  };

  const statusLabel = autosave.saving ? "Saving…" : autosave.dirty ? "Unsaved changes" : "Saved";

  return (
    <div className="flex flex-col gap-2">
      {conflict && (
        <RevisionConflictNotice subject="todo" revision={conflict.actual} onReload={onReload} />
      )}

      {error && !conflict && (
        <p className="text-[0.8125rem] text-destructive" aria-live="polite">
          {error}
        </p>
      )}

      <TodoDocFields
        title={title}
        status={status}
        initialBody={initial.body}
        titleId="todo-edit-title"
        scratchpads={scratchpads}
        scratchpad={scratchpad}
        onTitleChange={(value) => {
          setTitle(value);
          autosave.push(bodyRef.current);
        }}
        onStatusChange={(value) => {
          setStatus(value);
          autosave.push(bodyRef.current);
        }}
        onScratchpadChange={(value) => {
          setScratchpad(value);
          autosave.push(bodyRef.current);
        }}
        onBodyChange={(markdown) => {
          bodyRef.current = markdown;
          autosave.push(markdown);
        }}
        onSaveShortcut={autosave.flush}
        onBlur={autosave.flush}
      />

      <footer className="flex items-center gap-3">
        <span
          className="text-[0.6875rem] text-muted-foreground"
          aria-live="polite"
          data-todo-autosave-status
        >
          {statusLabel}
        </span>
        <div className="flex-1" />
        <Button size="sm" onClick={done} data-todo-done>
          <Check aria-hidden /> Done
        </Button>
      </footer>
    </div>
  );
}
