import { useCallback, useRef, useState } from "react";
import { templateRead, templateUpdate } from "@/api";
import type { TemplateKind, TemplateScope } from "@/domain";

// A revision conflict surfaced to the editor: an edit was refused because the template moved on since
// it was opened. `actual` is the revision it now sits at, so the banner can name it.
export interface TemplateConflict {
  actual: number;
}

// The placeholder list of a template that declares none, and of no open template — one shared empty
// array, so a consumer that keys off its identity is not woken by every unrelated state change.
const NO_PLACEHOLDERS: string[] = [];

export interface TemplateEditorStore {
  /** The open template's kind, or null when none is open. */
  kind: TemplateKind | null;
  /** The scope the open template lives in, or null when none is open. */
  scope: TemplateScope | null;
  /** The open template's name, or null when none is open. */
  name: string | null;
  /** The Markdown body the editor mounts with, or null while none is open or it is still loading. */
  initialBody: string | null;
  /** The one-line description the open template loaded with. */
  initialDescription: string;
  /**
   * The {{placeholders}} the open template's body declares, in first-appearance order, as the core
   * derived them. Re-read from every load and every accepted save, so editing a marker into or out
   * of the body moves this list once the edit lands. Never derived here — the core's scan is the one
   * definition of what a placeholder is.
   */
  placeholders: string[];
  /** The revision the open template was loaded at — the guard the next write carries. */
  baseRevision: number | null;
  /** Bumped on every open and reload so the editor remounts with fresh content and undo history. */
  mountKey: number;
  loading: boolean;
  /** A stale-write conflict to surface, or null. The core refused the write, so nothing was clobbered. */
  conflict: TemplateConflict | null;
  /** A non-conflict failure (e.g. an invalid document), or null. */
  error: string | null;
  open: (kind: TemplateKind, scope: TemplateScope, name: string) => void;
  close: () => void;
  /**
   * Saves the description + body revision-guarded; resolves once the outcome is set. The
   * description goes to the core verbatim: it reads a blank one as "clear it" and only an omitted
   * one as "keep the stored one", so a blank must not be mapped to null on the way out.
   */
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
export function useTemplateEditor(project: number | null): TemplateEditorStore {
  const [kind, setKind] = useState<TemplateKind | null>(null);
  const [scope, setScope] = useState<TemplateScope | null>(null);
  const [name, setName] = useState<string | null>(null);
  const [initialBody, setInitialBody] = useState<string | null>(null);
  const [initialDescription, setInitialDescription] = useState("");
  const [placeholders, setPlaceholders] = useState<string[]>(NO_PLACEHOLDERS);
  const [baseRevision, setBaseRevision] = useState<number | null>(null);
  const [mountKey, setMountKey] = useState(0);
  const [loading, setLoading] = useState(false);
  const [conflict, setConflict] = useState<TemplateConflict | null>(null);
  const [error, setError] = useState<string | null>(null);
  const baseRevisionRef = useRef<number | null>(null);

  // The project id a scope addresses, so every read and write in this hook stays inside the library
  // the template was opened from.
  const idOf = useCallback(
    (target: TemplateScope) => (target === "global" ? null : project),
    [project],
  );

  const load = useCallback(
    (target: TemplateKind, targetScope: TemplateScope, targetName: string) => {
      setLoading(true);
      setConflict(null);
      setError(null);
      templateRead(target, idOf(targetScope), targetName)
        .then((view) => {
          setInitialBody(view.body);
          setInitialDescription(view.description ?? "");
          setPlaceholders(view.placeholders);
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
    [idOf],
  );

  const open = useCallback(
    (target: TemplateKind, targetScope: TemplateScope, targetName: string) => {
      setKind(target);
      setScope(targetScope);
      setName(targetName);
      setInitialBody(null);
      setInitialDescription("");
      setPlaceholders(NO_PLACEHOLDERS);
      setBaseRevision(null);
      // Disarm the guard for the load window: until the read resolves, a save must not carry the
      // previously open template's revision into a write against this one.
      baseRevisionRef.current = null;
      load(target, targetScope, targetName);
    },
    [load],
  );

  const close = useCallback(() => {
    setKind(null);
    setScope(null);
    setName(null);
    setInitialBody(null);
    setInitialDescription("");
    setPlaceholders(NO_PLACEHOLDERS);
    setBaseRevision(null);
    setConflict(null);
    setError(null);
  }, []);

  const reload = useCallback(() => {
    if (kind != null && scope != null && name != null) load(kind, scope, name);
  }, [kind, scope, name, load]);

  const save = useCallback(
    async (description: string, body: string) => {
      if (kind == null || scope == null || name == null || baseRevisionRef.current == null) return;
      setError(null);
      try {
        const view = await templateUpdate(
          kind,
          idOf(scope),
          name,
          description,
          body,
          baseRevisionRef.current,
        );
        setPlaceholders(view.placeholders);
        setBaseRevision(view.revision);
        baseRevisionRef.current = view.revision;
      } catch (reason) {
        // The write was refused. Re-read to tell a stale revision (a concurrent edit landed — surface
        // a conflict and keep the user's edits) from any other rejection (e.g. an invalid document),
        // surfaced verbatim from the core.
        try {
          const fresh = await templateRead(kind, idOf(scope), name);
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
    [kind, scope, name, idOf],
  );

  return {
    kind,
    scope,
    name,
    initialBody,
    initialDescription,
    placeholders,
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
