import { useCallback, useEffect, useState } from "react";
import {
  onDomainEvent,
  setDefaultTemplate,
  templateCreate,
  templateDefaults,
  templateDelete,
  templateRead,
  templates as listTemplates,
} from "@/api";
import { TEMPLATE_KINDS } from "@/lib/templates";
import { persistThenReconcile } from "@/store/persist";
import { uniqueCopyName } from "@/store/templateCopy";
import { useLatestRef } from "@/store/useLatestRef";
import type { TemplateDefaults, TemplateKind, TemplateSummary } from "@/domain";

type TemplateLists = Record<TemplateKind, TemplateSummary[]>;

const EMPTY_LISTS: TemplateLists = { prompt: [], scratchpad: [], todo: [] };
const NO_DEFAULTS: TemplateDefaults = { scratchpad: null, todo: null };

export interface TemplatesStore {
  /** Every template grouped by kind, refreshed live on `TemplateChanged`. */
  lists: TemplateLists;
  /** The selected default template per seedable kind (global-only). */
  defaults: TemplateDefaults;
  /** A load or default-selection failure, or null. */
  error: string | null;
  /** Create a template; rejects with the core's message (a taken name, a blank body) for the form. */
  create: (kind: TemplateKind, name: string, description: string, body: string) => Promise<void>;
  /** Delete a template; rejects on failure. The core clears a default that pointed at it. */
  remove: (kind: TemplateKind, name: string) => Promise<void>;
  /** Copy a template under a free `<name> copy` handle; rejects on failure. */
  duplicate: (kind: TemplateKind, name: string) => Promise<void>;
  /** Select (or clear, with null) the default template for a seedable kind — optimistic, reconciled. */
  setDefault: (kind: TemplateKind, id: number | null) => void;
}

// The Settings template-manager read model: the global library grouped by kind plus the default
// selection. Loads each kind once, then re-reads the changed kind — and the defaults, which a delete
// may have cleared in core — on every `TemplateChanged` event (no polling, no per-render IPC). Holds
// no business logic: kind grouping, name uniqueness, and clearing a deleted default all live in the
// core; this projects the result and routes writes to the one façade.
export function useTemplates(): TemplatesStore {
  const [lists, setLists] = useState<TemplateLists>(EMPTY_LISTS);
  const [defaults, setDefaults] = useState<TemplateDefaults>(NO_DEFAULTS);
  const [error, setError] = useState<string | null>(null);

  // The latest lists, read by `duplicate` for name uniqueness without making it re-created on every
  // refresh (which would not matter, but the ref keeps the action identity stable).
  const listsRef = useLatestRef(lists);

  const fail = useCallback((reason: unknown) => setError(String(reason)), []);

  const loadKind = useCallback(
    (kind: TemplateKind) => {
      listTemplates(kind)
        .then((rows) => setLists((prev) => ({ ...prev, [kind]: rows })))
        .catch(fail);
    },
    [fail],
  );

  const loadDefaults = useCallback(() => {
    templateDefaults().then(setDefaults).catch(fail);
  }, [fail]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    for (const kind of TEMPLATE_KINDS) loadKind(kind);
    loadDefaults();

    // Attach the listener so a create/edit/delete (from this manager or an MCP prompt-template
    // caller) re-reads the affected kind and the defaults.
    onDomainEvent((event) => {
      if (event.type === "TemplateChanged") {
        loadKind(event.kind);
        loadDefaults();
      }
    })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(fail);

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [loadKind, loadDefaults, fail]);

  const create = useCallback(
    async (kind: TemplateKind, name: string, description: string, body: string) => {
      await templateCreate(kind, name, description.trim() === "" ? null : description, body);
    },
    [],
  );

  const remove = useCallback(async (kind: TemplateKind, name: string) => {
    await templateDelete(kind, name);
  }, []);

  const duplicate = useCallback(
    async (kind: TemplateKind, name: string) => {
      const source = await templateRead(kind, name);
      const existing = listsRef.current[kind].map((template) => template.name);
      await templateCreate(
        kind,
        uniqueCopyName(source.name, existing),
        source.description,
        source.body,
      );
    },
    [listsRef],
  );

  const setDefault = useCallback((kind: TemplateKind, id: number | null) => {
    setDefaults((prev) => ({ ...prev, [kind]: id }));
    persistThenReconcile(setDefaultTemplate(kind, id), templateDefaults, setDefaults);
  }, []);

  return { lists, defaults, error, create, remove, duplicate, setDefault };
}
