import { useCallback, useState } from "react";
import { scratchpadLink, scratchpadRead, scratchpadWrite } from "@/api";

// A revision conflict surfaced to the panel: a write was refused because the scratchpad moved on
// since it was opened. `actual` is the revision it now sits at, so the banner can name it.
export interface ScratchpadConflict {
  actual: number;
}

export interface ScratchpadEditorStore {
  /** The open scratchpad's name, or null when none is open. */
  name: string | null;
  /** The editable Markdown body, or null while none is open or it is still loading. */
  body: string | null;
  /** The revision the open body was loaded at — the guard the next write carries. */
  baseRevision: number | null;
  loading: boolean;
  saving: boolean;
  /** A stale-write conflict to surface, or null. The core refused the write, so nothing was clobbered. */
  conflict: ScratchpadConflict | null;
  /** A non-conflict failure (e.g. an invalid document), or null. */
  error: string | null;
  open: (name: string) => void;
  close: () => void;
  setBody: (body: string) => void;
  save: () => void;
  /** Reload the open scratchpad fresh, discarding local edits — the conflict resolution. */
  reload: () => void;
  /** Copy the scratchpad's `solo://` link to the clipboard, by its durable id. */
  copyLink: (id: number) => void;
}

// Drives the scratchpad panel's edit lifecycle: open one (read its Markdown body), edit it, and save
// revision-guarded. A stale write is refused by the core — this hook then re-reads to learn whether
// the revision moved (a real conflict, surfaced for the user to reload) or the write failed for
// another reason (an invalid document, surfaced as an error). It never re-decides validity or
// clobbers a concurrent edit; the core is the single source of truth. The `project` is the local-UI
// scope (the trusted surface). Live snapshot refresh lives in the parent's `useOrchestration`.
export function useScratchpadEditor(project: number): ScratchpadEditorStore {
  const [name, setName] = useState<string | null>(null);
  const [body, setBody] = useState<string | null>(null);
  const [baseRevision, setBaseRevision] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [conflict, setConflict] = useState<ScratchpadConflict | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(
    (target: string) => {
      setLoading(true);
      setConflict(null);
      setError(null);
      scratchpadRead(project, target)
        .then((view) => {
          setBody(view.body);
          setBaseRevision(view.revision);
        })
        .catch((reason) => setError(String(reason)))
        .finally(() => setLoading(false));
    },
    [project],
  );

  const open = useCallback(
    (target: string) => {
      setName(target);
      setBody(null);
      setBaseRevision(null);
      load(target);
    },
    [load],
  );

  const close = useCallback(() => {
    setName(null);
    setBody(null);
    setBaseRevision(null);
    setConflict(null);
    setError(null);
  }, []);

  const reload = useCallback(() => {
    if (name != null) load(name);
  }, [name, load]);

  const save = useCallback(() => {
    if (name == null || body == null) return;
    setSaving(true);
    setConflict(null);
    setError(null);
    scratchpadWrite(project, name, body, baseRevision)
      .then((view) => {
        setBody(view.body);
        setBaseRevision(view.revision);
      })
      .catch((reason) => {
        // The write was refused. Re-read to tell a stale revision (a concurrent edit landed — surface
        // a conflict and leave the user's edits intact) from any other rejection (e.g. an invalid
        // document), which we surface verbatim from the core rather than guessing a reason.
        scratchpadRead(project, name)
          .then((fresh) => {
            if (baseRevision != null && fresh.revision !== baseRevision) {
              setConflict({ actual: fresh.revision });
            } else {
              setError(String(reason));
            }
          })
          .catch((readReason) => setError(String(readReason)));
      })
      .finally(() => setSaving(false));
  }, [project, name, body, baseRevision]);

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
    body,
    baseRevision,
    loading,
    saving,
    conflict,
    error,
    open,
    close,
    setBody,
    save,
    reload,
    copyLink,
  };
}
