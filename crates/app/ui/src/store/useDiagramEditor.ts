import { useCallback, useRef, useState } from "react";
import { diagramRead, diagramRename, diagramWrite } from "@/api";

// A revision conflict surfaced to the panel: a write was refused because the diagram moved on since it
// was opened. `actual` is the revision it now sits at, so the banner can name it.
export interface DiagramConflict {
  actual: number;
}

export interface DiagramEditorStore {
  /** The open diagram's name, or null when none is open. */
  name: string | null;
  /**
   * The Mermaid source the editor mounts with, or null while none is open or it is still loading. The
   * source editor is seeded from this and a change of `mountKey` re-seeds it — the persisted source is
   * never pushed back mid-edit.
   */
  initialSource: string | null;
  /** The revision the open source was loaded at — the guard the next write carries. */
  baseRevision: number | null;
  /** Bumped on every open and reload so the editor can key off it and re-seed with fresh content. */
  mountKey: number;
  loading: boolean;
  /** A stale-write conflict to surface, or null. The core refused the write, so nothing was clobbered. */
  conflict: DiagramConflict | null;
  /** A non-conflict failure (e.g. an invalid document), or null. */
  error: string | null;
  open: (name: string) => void;
  close: () => void;
  /** Saves the Mermaid source revision-guarded; resolves once the outcome (success/conflict/error) is set. */
  save: (source: string) => Promise<void>;
  /** Reload the open diagram fresh, discarding local edits — the conflict resolution. */
  reload: () => void;
  /**
   * Rename the open diagram, re-pointing the editor at the new handle. Rejects with the core's refusal
   * (a taken name, an invalid one) so the caller can keep the user's text and show why.
   */
  rename: (to: string) => Promise<void>;
}

// Drives the diagram panel's edit lifecycle: open one (read its Mermaid source to seed the editor),
// edit it (the source editor autosaves through `save`), and save revision-guarded. A stale write is
// refused by the core — this hook then re-reads to learn whether the revision moved (a real conflict,
// surfaced for the user to reload) or the write failed for another reason (surfaced as an error). It
// never re-decides validity or clobbers a concurrent edit; the core is the single source of truth. The
// base revision is held in a ref as well as state so `save` reads the current guard without being
// re-created on every bump (which would restart the autosave loop). The `project` is the local-UI scope
// (the trusted surface). Live snapshot refresh lives in the parent's `useOrchestration`. Mirrors
// `useScratchpadEditor`, minus the `solo://` copy-link (a diagram has no link surface).
export function useDiagramEditor(project: number): DiagramEditorStore {
  const [name, setName] = useState<string | null>(null);
  const [initialSource, setInitialSource] = useState<string | null>(null);
  const [baseRevision, setBaseRevision] = useState<number | null>(null);
  const [mountKey, setMountKey] = useState(0);
  const [loading, setLoading] = useState(false);
  const [conflict, setConflict] = useState<DiagramConflict | null>(null);
  const [error, setError] = useState<string | null>(null);
  const baseRevisionRef = useRef<number | null>(null);

  const load = useCallback(
    (target: string) => {
      setLoading(true);
      setConflict(null);
      setError(null);
      diagramRead(project, target)
        .then((view) => {
          setInitialSource(view.source);
          setBaseRevision(view.revision);
          baseRevisionRef.current = view.revision;
          // Re-seed the editor so it re-mounts with the fresh source and a clean history.
          setMountKey((key) => key + 1);
        })
        .catch((reason) => {
          setInitialSource(null);
          setError(String(reason));
        })
        .finally(() => setLoading(false));
    },
    [project],
  );

  const open = useCallback(
    (target: string) => {
      setName(target);
      setInitialSource(null);
      setBaseRevision(null);
      baseRevisionRef.current = null;
      load(target);
    },
    [load],
  );

  const close = useCallback(() => {
    setName(null);
    setInitialSource(null);
    setBaseRevision(null);
    baseRevisionRef.current = null;
    setConflict(null);
    setError(null);
  }, []);

  const reload = useCallback(() => {
    if (name != null) load(name);
  }, [name, load]);

  const save = useCallback(
    async (source: string) => {
      if (name == null) return;
      setError(null);
      try {
        const view = await diagramWrite(project, name, source, baseRevisionRef.current);
        setBaseRevision(view.revision);
        baseRevisionRef.current = view.revision;
      } catch (reason) {
        // The write was refused. Re-read to tell a stale revision (a concurrent edit landed — surface a
        // conflict and leave the user's edits intact) from any other rejection (e.g. an invalid
        // document), which we surface verbatim from the core rather than guessing a reason.
        try {
          const fresh = await diagramRead(project, name);
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

  // A rename keeps the document's durable id, source, and revision, so the open editor only has to
  // follow the new handle — no remount, no re-read, no interruption to an edit in progress. The refusal
  // is rethrown rather than parked in `error`, so the header's field keeps the typed name.
  const rename = useCallback(
    async (to: string) => {
      if (name == null) return;
      const view = await diagramRename(project, name, to);
      setName(view.name);
    },
    [project, name],
  );

  return {
    name,
    initialSource,
    baseRevision,
    mountKey,
    loading,
    conflict,
    error,
    open,
    close,
    save,
    reload,
    rename,
  };
}
