import { useCallback, useRef, useState } from "react";
import { templateRead, templateUpdate } from "@/api";
import type { TemplateKind } from "@/domain";

// A revision conflict surfaced to the editor: an edit was refused because the template moved on since
// it was opened. `actual` is the revision it now sits at, so the banner can name it.
export interface TemplateConflict {
  actual: number;
}

export interface TemplateEditorStore {
  /** The open template's kind, or null when none is open. */
  kind: TemplateKind | null;
  /** The open template's name, or null when none is open. */
  name: string | null;
  /** The Markdown body the editor mounts with, or null while none is open or it is still loading. */
  initialBody: string | null;
  /** The one-line description the open template loaded with. */
  initialDescription: string;
  /** The revision the open template was loaded at — the guard the next write carries. */
  baseRevision: number | null;
  /** Bumped on every open and reload so the editor remounts with fresh content and undo history. */
  mountKey: number;
  loading: boolean;
  /** A stale-write conflict to surface, or null. The core refused the write, so nothing was clobbered. */
  conflict: TemplateConflict | null;
  /** A non-conflict failure (e.g. an invalid document), or null. */
  error: string | null;
  open: (kind: TemplateKind, name: string) => void;
  close: () => void;
  /** Saves the description + body revision-guarded; resolves once the outcome is set. */
  save: (description: string, body: string) => Promise<void>;
  /** Reload the open template fresh, discarding local edits — the conflict resolution. */
  reload: () => void;
}

// Drives the template editor's edit lifecycle against the uncontrolled rich-text editor: open one
// (read its Markdown body to seed the editor), edit its description + body (autosaved through `save`),
// and save revision-guarded. A stale write is refused by the core — this hook re-reads to tell a real
// conflict (the revision moved; surface a reload) from any other rejection (an invalid document;
// surface the message). It never re-decides validity or clobbers a concurrent edit; the core is the
// single source of truth. Mirrors `useScratchpadEditor`, extended for a template's separate
// description field.
export function useTemplateEditor(): TemplateEditorStore {
  const [kind, setKind] = useState<TemplateKind | null>(null);
  const [name, setName] = useState<string | null>(null);
  const [initialBody, setInitialBody] = useState<string | null>(null);
  const [initialDescription, setInitialDescription] = useState("");
  const [baseRevision, setBaseRevision] = useState<number | null>(null);
  const [mountKey, setMountKey] = useState(0);
  const [loading, setLoading] = useState(false);
  const [conflict, setConflict] = useState<TemplateConflict | null>(null);
  const [error, setError] = useState<string | null>(null);
  const baseRevisionRef = useRef<number | null>(null);

  const load = useCallback((target: TemplateKind, targetName: string) => {
    setLoading(true);
    setConflict(null);
    setError(null);
    templateRead(target, targetName)
      .then((view) => {
        setInitialBody(view.body);
        setInitialDescription(view.description ?? "");
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
  }, []);

  const open = useCallback(
    (target: TemplateKind, targetName: string) => {
      setKind(target);
      setName(targetName);
      setInitialBody(null);
      setInitialDescription("");
      setBaseRevision(null);
      baseRevisionRef.current = null;
      load(target, targetName);
    },
    [load],
  );

  const close = useCallback(() => {
    setKind(null);
    setName(null);
    setInitialBody(null);
    setInitialDescription("");
    setBaseRevision(null);
    baseRevisionRef.current = null;
    setConflict(null);
    setError(null);
  }, []);

  const reload = useCallback(() => {
    if (kind != null && name != null) load(kind, name);
  }, [kind, name, load]);

  const save = useCallback(
    async (description: string, body: string) => {
      if (kind == null || name == null || baseRevisionRef.current == null) return;
      setError(null);
      try {
        const view = await templateUpdate(
          kind,
          name,
          description.trim() === "" ? null : description,
          body,
          baseRevisionRef.current,
        );
        setBaseRevision(view.revision);
        baseRevisionRef.current = view.revision;
      } catch (reason) {
        // The write was refused. Re-read to tell a stale revision (a concurrent edit landed — surface
        // a conflict and keep the user's edits) from any other rejection (e.g. an invalid document),
        // surfaced verbatim from the core.
        try {
          const fresh = await templateRead(kind, name);
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
    [kind, name],
  );

  return {
    kind,
    name,
    initialBody,
    initialDescription,
    baseRevision,
    mountKey,
    loading,
    conflict,
    error,
    open,
    close,
    save,
    reload,
  };
}
