import { useCallback, useRef, useState } from "react";
import { scratchpadLink, scratchpadRead, scratchpadWrite } from "@/api";

// A revision conflict surfaced to the panel: a write was refused because the scratchpad moved on
// since it was opened. `actual` is the revision it now sits at, so the banner can name it.
export interface ScratchpadConflict {
  actual: number;
}

export interface ScratchpadEditorStore {
  /** The open scratchpad's name, or null when none is open. */
  name: string | null;
  /**
   * The Markdown the editor mounts with, or null while none is open or it is still loading. The
   * editor is uncontrolled: this seeds it, and a change of `mountKey` remounts it with a fresh body
   * and a fresh undo history — the body is never pushed back in mid-edit.
   */
  initialBody: string | null;
  /** The revision the open body was loaded at — the guard the next write carries. */
  baseRevision: number | null;
  /** Bumped on every open and reload so the editor can key off it and remount with fresh content. */
  mountKey: number;
  loading: boolean;
  /** A stale-write conflict to surface, or null. The core refused the write, so nothing was clobbered. */
  conflict: ScratchpadConflict | null;
  /** A non-conflict failure (e.g. an invalid document), or null. */
  error: string | null;
  open: (name: string) => void;
  close: () => void;
  /** Saves the Markdown body revision-guarded; resolves once the outcome (success/conflict/error) is set. */
  save: (markdown: string) => Promise<void>;
  /** Reload the open scratchpad fresh, discarding local edits — the conflict resolution. */
  reload: () => void;
  /** Copy the scratchpad's `solo://` link to the clipboard, by its durable id. */
  copyLink: (id: number) => void;
}

// Drives the scratchpad panel's edit lifecycle against an uncontrolled rich-text editor: open one
// (read its Markdown body to seed the editor), edit it (the editor emits Markdown; the panel autosaves
// through `save`), and save revision-guarded. A stale write is refused by the core — this hook then
// re-reads to learn whether the revision moved (a real conflict, surfaced for the user to reload) or
// the write failed for another reason (an invalid document, surfaced as an error). It never re-decides
// validity or clobbers a concurrent edit; the core is the single source of truth. The base revision is
// held in a ref as well as state so `save` reads the current guard without being re-created on every
// bump (which would restart the autosave loop). The `project` is the local-UI scope (the trusted
// surface). Live snapshot refresh lives in the parent's `useOrchestration`.
export function useScratchpadEditor(project: number): ScratchpadEditorStore {
  const [name, setName] = useState<string | null>(null);
  const [initialBody, setInitialBody] = useState<string | null>(null);
  const [baseRevision, setBaseRevision] = useState<number | null>(null);
  const [mountKey, setMountKey] = useState(0);
  const [loading, setLoading] = useState(false);
  const [conflict, setConflict] = useState<ScratchpadConflict | null>(null);
  const [error, setError] = useState<string | null>(null);
  const baseRevisionRef = useRef<number | null>(null);

  const load = useCallback(
    (target: string) => {
      setLoading(true);
      setConflict(null);
      setError(null);
      scratchpadRead(project, target)
        .then((view) => {
          setInitialBody(view.body);
          setBaseRevision(view.revision);
          baseRevisionRef.current = view.revision;
          // Remount the editor so it re-seeds with the fresh body and starts a clean undo history.
          setMountKey((key) => key + 1);
        })
        .catch((reason) => {
          setInitialBody(null);
          setError(String(reason));
        })
        .finally(() => setLoading(false));
    },
    [project],
  );

  const open = useCallback(
    (target: string) => {
      setName(target);
      setInitialBody(null);
      setBaseRevision(null);
      baseRevisionRef.current = null;
      load(target);
    },
    [load],
  );

  const close = useCallback(() => {
    setName(null);
    setInitialBody(null);
    setBaseRevision(null);
    baseRevisionRef.current = null;
    setConflict(null);
    setError(null);
  }, []);

  const reload = useCallback(() => {
    if (name != null) load(name);
  }, [name, load]);

  const save = useCallback(
    async (markdown: string) => {
      if (name == null) return;
      setError(null);
      try {
        const view = await scratchpadWrite(project, name, markdown, baseRevisionRef.current);
        setBaseRevision(view.revision);
        baseRevisionRef.current = view.revision;
      } catch (reason) {
        // The write was refused. Re-read to tell a stale revision (a concurrent edit landed — surface
        // a conflict and leave the user's edits intact) from any other rejection (e.g. an invalid
        // document), which we surface verbatim from the core rather than guessing a reason.
        try {
          const fresh = await scratchpadRead(project, name);
          if (baseRevisionRef.current != null && fresh.revision !== baseRevisionRef.current) {
            setConflict({ actual: fresh.revision });
          } else {
            setError(String(reason));
          }
        } catch (readReason) {
          setError(String(readReason));
        }
      }
    },
    [project, name],
  );

  const copyLink = useCallback(
    (id: number) => {
      scratchpadLink(project, id)
        .then((link) => navigator.clipboard?.writeText(link))
        .catch((reason) => setError(String(reason)));
    },
    [project],
  );

  return {
    name,
    initialBody,
    baseRevision,
    mountKey,
    loading,
    conflict,
    error,
    open,
    close,
    save,
    reload,
    copyLink,
  };
}
