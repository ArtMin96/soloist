import { useCallback, useRef, useState } from "react";
import { todoCreate, todoUpdate } from "@/api";
import type { TodoDoc, TodoView } from "@/domain";

export type TodoEditorMode = "create" | "edit";

export interface TodoEditorStore {
  /** Whether a create form or an edit surface is open, or null when neither is. */
  mode: TodoEditorMode | null;
  /** The todo being edited (edit mode); null in create mode and when closed. */
  editingId: number | null;
  /**
   * The document the fields mount with, frozen at open/reload. The body editor is uncontrolled:
   * this seeds it, and a change of `mountKey` remounts it with fresh content and undo history — the
   * document is never pushed back mid-edit, so a save never moves the caret.
   */
  initial: TodoDoc | null;
  /** The revision the edited todo was opened at — the guard the next update carries. Null in create. */
  baseRevision: number | null;
  /** Bumped on every open and reload so the fields can key off it and remount with fresh content. */
  mountKey: number;
  /** A non-conflict failure (an invalid document, or the blocked→done gate), or null. */
  error: string | null;
  /** Open the create form with an empty draft. */
  startCreate: () => void;
  /** Open the edit surface for `todo`, seeded from its current document and revision. */
  editTodo: (todo: TodoView) => void;
  /** Close whichever surface is open, discarding its unsaved edits. */
  close: () => void;
  /**
   * Persists `doc`: create → {@link todoCreate} then close; edit → {@link todoUpdate} guarded by the
   * base revision, bumping it on success. Resolves once the outcome is applied; a rejection sets
   * `error` and keeps the surface open so the caller's edits survive.
   */
  save: (doc: TodoDoc) => Promise<void>;
  /**
   * Re-seed the edit from `todo` (the live snapshot's copy) — the conflict resolution: it discards
   * local edits and adopts the concurrent writer's document and revision.
   */
  reload: (todo: TodoView) => void;
}

const DRAFT: TodoDoc = { title: "", body: "", status: "open" };

// Drives the to-do board's create/edit lifecycle against the shared uncontrolled editor. Creating
// posts a whole document; editing writes the whole document revision-guarded, so a stale write is
// refused by the core (never clobbering a concurrent edit) — the board watches the live revision to
// raise the conflict banner and calls `reload` to resolve it. The base revision and edited id are
// held in refs as well as state so `save` reads the current guard without being re-created on each
// bump (which would restart the caller's autosave loop). The `project` is the local-UI scope.
export function useTodoEditor(project: number): TodoEditorStore {
  const [mode, setMode] = useState<TodoEditorMode | null>(null);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [initial, setInitial] = useState<TodoDoc | null>(null);
  const [baseRevision, setBaseRevision] = useState<number | null>(null);
  const [mountKey, setMountKey] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const editingIdRef = useRef<number | null>(null);
  const baseRevisionRef = useRef<number | null>(null);

  const startCreate = useCallback(() => {
    setMode("create");
    setEditingId(null);
    editingIdRef.current = null;
    setInitial(DRAFT);
    setBaseRevision(null);
    baseRevisionRef.current = null;
    setError(null);
    setMountKey((key) => key + 1);
  }, []);

  const seedEdit = useCallback((todo: TodoView) => {
    setMode("edit");
    setEditingId(todo.id);
    editingIdRef.current = todo.id;
    setInitial(todo.doc);
    setBaseRevision(todo.revision);
    baseRevisionRef.current = todo.revision;
    setError(null);
    setMountKey((key) => key + 1);
  }, []);

  const close = useCallback(() => {
    setMode(null);
    setEditingId(null);
    editingIdRef.current = null;
    setInitial(null);
    setBaseRevision(null);
    baseRevisionRef.current = null;
    setError(null);
  }, []);

  const save = useCallback(
    async (doc: TodoDoc) => {
      setError(null);
      const id = editingIdRef.current;
      try {
        if (id == null) {
          await todoCreate(project, doc);
          close();
        } else {
          const view = await todoUpdate(project, id, doc, baseRevisionRef.current ?? 0);
          setBaseRevision(view.revision);
          baseRevisionRef.current = view.revision;
        }
      } catch (reason) {
        // The write was refused. Surface the core's message verbatim (an invalid document or the
        // blocked→done gate) and keep the surface open — the uncontrolled editor still holds the
        // edits. A revision conflict is the board's call from the live revision, not re-decided here.
        setError(String(reason));
      }
    },
    [project, close],
  );

  return {
    mode,
    editingId,
    initial,
    baseRevision,
    mountKey,
    error,
    startCreate,
    editTodo: seedEdit,
    close,
    save,
    reload: seedEdit,
  };
}
