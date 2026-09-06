import { useCallback, useRef, useState } from "react";
import { scratchpadLink, scratchpadRead, scratchpadRename, scratchpadWrite } from "@/api";
import { failed, loading, LoadStatus, ready, type Loadable } from "@/store/loadable";
import type { SaveOutcome } from "@/store/saveOutcome";
import type { ScratchpadView } from "@/domain";

// A revision conflict surfaced to the panel: a write was refused because the scratchpad moved on
// since it was opened. `actual` is the revision it now sits at, so the banner can name it.
export interface ScratchpadConflict {
  actual: number;
}

export interface ScratchpadEditorStore {
  /** The open scratchpad's name, or null when none is open. */
  name: string | null;
  /**
   * The open scratchpad's body read: loading from `open` until the read answers, ready after, and
   * failed when a read with nothing to fall back on is refused. A `reload` keeps the held document
   * on screen until the fresh read lands, and a save that goes through replaces it with the view
   * the write returned. Loading while nothing is open — the surface renders no body then.
   */
  document: Loadable<ScratchpadView>;
  /** The revision the open body was loaded at — the guard the next write carries. */
  baseRevision: number | null;
  /** Bumped on every open and reload so the editor can key off it and remount with fresh content. */
  mountKey: number;
  /** A stale-write conflict to surface, or null. The core refused the write, so nothing was clobbered. */
  conflict: ScratchpadConflict | null;
  /** A non-conflict failure beside a document that stays on screen (an invalid write, a refused
   *  re-read, a refused link), or null. */
  error: string | null;
  open: (name: string) => void;
  close: () => void;
  /**
   * Saves the Markdown body revision-guarded, resolving to whether it went through. A refusal
   * (conflict or error) never rejects — it is surfaced through `conflict`/`error` state, and the
   * resolved `"refused"` is the caller's signal that nothing was persisted.
   */
  save: (markdown: string) => Promise<SaveOutcome>;
  /** Reload the open scratchpad fresh, discarding local edits — the conflict resolution. */
  reload: () => void;
  /**
   * Rename the open scratchpad, re-pointing the editor at the new handle. Rejects with the core's
   * refusal (a taken name, an invalid one) so the caller can keep the user's text and show why.
   */
  rename: (to: string) => Promise<void>;
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
  const [document, setDocumentPhase] = useState<Loadable<ScratchpadView>>(loading());
  const [baseRevision, setBaseRevision] = useState<number | null>(null);
  const [mountKey, setMountKey] = useState(0);
  const [conflict, setConflict] = useState<ScratchpadConflict | null>(null);
  const [error, setError] = useState<string | null>(null);
  const baseRevisionRef = useRef<number | null>(null);
  const loadRequestRef = useRef(0);
  // The phase is mirrored in a ref as well as state so a read's own callbacks can tell a re-read
  // beside a document already on screen from one with nothing to fall back on, without `load`
  // being re-created — and restarting the autosave loop — on every phase change.
  const documentRef = useRef<Loadable<ScratchpadView>>(loading());

  const setDocument = useCallback((next: Loadable<ScratchpadView>) => {
    documentRef.current = next;
    setDocumentPhase(next);
  }, []);

  const load = useCallback(
    (target: string) => {
      const request = ++loadRequestRef.current;
      // A re-read keeps what is already on screen until the fresh body lands, so a revision the
      // roster noticed never blanks the document the user is reading; with nothing held, the read
      // is the only thing standing between the surface and its content.
      if (documentRef.current.status !== LoadStatus.Ready) setDocument(loading());
      setConflict(null);
      setError(null);
      scratchpadRead(project, target)
        .then((view) => {
          if (request !== loadRequestRef.current) return;
          setDocument(ready(view));
          setBaseRevision(view.revision);
          baseRevisionRef.current = view.revision;
          // Remount the editor so it re-seeds with the fresh body and starts a clean undo history.
          setMountKey((key) => key + 1);
        })
        .catch((reason) => {
          if (request !== loadRequestRef.current) return;
          // Where the refusal belongs depends on whether there is a body to keep: beside a held
          // document it is news about a re-read, and with nothing held it is the phase itself.
          if (documentRef.current.status === LoadStatus.Ready) setError(String(reason));
          else setDocument(failed(String(reason)));
        });
    },
    [project, setDocument],
  );

  const open = useCallback(
    (target: string) => {
      setName(target);
      setDocument(loading());
      setBaseRevision(null);
      baseRevisionRef.current = null;
      load(target);
    },
    [load, setDocument],
  );

  const close = useCallback(() => {
    loadRequestRef.current += 1;
    setName(null);
    setDocument(loading());
    setBaseRevision(null);
    baseRevisionRef.current = null;
    setConflict(null);
    setError(null);
  }, [setDocument]);

  const reload = useCallback(() => {
    if (name != null) load(name);
  }, [name, load]);

  const save = useCallback(
    async (markdown: string): Promise<SaveOutcome> => {
      if (name == null) return "refused";
      setError(null);
      try {
        const view = await scratchpadWrite(project, name, markdown, baseRevisionRef.current);
        // The write answers with the document it stored, so what was just saved becomes what is
        // read without a round trip — and without bumping `mountKey`, which would remount the
        // editor and drop the caret mid-edit.
        setDocument(ready(view));
        setBaseRevision(view.revision);
        baseRevisionRef.current = view.revision;
        return "saved";
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
        return "refused";
      }
    },
    [project, name, setDocument],
  );

  // A rename keeps the document's durable id, body, and revision, so the open editor only has to
  // follow the new handle — no remount, no re-read, no interruption to an edit in progress. The
  // refusal is rethrown rather than parked in `error`, so the header's field keeps the typed name.
  const rename = useCallback(
    async (to: string) => {
      if (name == null) return;
      const view = await scratchpadRename(project, name, to);
      setName(view.name);
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
    document,
    baseRevision,
    mountKey,
    conflict,
    error,
    open,
    close,
    save,
    reload,
    rename,
    copyLink,
  };
}
