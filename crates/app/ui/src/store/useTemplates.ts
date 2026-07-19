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
import { TEMPLATE_KINDS, TEMPLATE_SCOPES } from "@/lib/templates";
import { persistThenReconcile } from "@/store/persist";
import { uniqueCopyName } from "@/store/templateCopy";
import { useLatestRef } from "@/store/useLatestRef";
import type { TemplateDefaults, TemplateKind, TemplateScope, TemplateSummary } from "@/domain";

/** One kind's templates, split by the scope they live in — the two are separate libraries. */
export type TemplateScopeLists = Record<TemplateScope, TemplateSummary[]>;

type TemplateLists = Record<TemplateKind, TemplateScopeLists>;

const EMPTY_SCOPES: TemplateScopeLists = { global: [], project: [] };
const EMPTY_LISTS: TemplateLists = {
  prompt: EMPTY_SCOPES,
  scratchpad: EMPTY_SCOPES,
  todo: EMPTY_SCOPES,
};
const NO_DEFAULTS: TemplateDefaults = { scratchpad: null, todo: null };

export interface TemplatesStore {
  /** Every template grouped by kind and scope, refreshed live on `TemplateChanged`. */
  lists: TemplateLists;
  /** The selected default template per seedable kind (global-only). */
  defaults: TemplateDefaults;
  /** A load or default-selection failure, or null. */
  error: string | null;
  /** Create a template in `scope`; rejects with the core's message (a taken name, a blank body). */
  create: (
    kind: TemplateKind,
    scope: TemplateScope,
    name: string,
    description: string,
    body: string,
  ) => Promise<void>;
  /** Delete a template from `scope`; rejects on failure. The core clears a default that pointed at it. */
  remove: (kind: TemplateKind, scope: TemplateScope, name: string) => Promise<void>;
  /** Copy a template within its scope under a free `<name> copy` handle; rejects on failure. */
  duplicate: (kind: TemplateKind, scope: TemplateScope, name: string) => Promise<void>;
  /** Select (or clear, with null) the default template for a seedable kind — optimistic, reconciled. */
  setDefault: (kind: TemplateKind, id: number | null) => void;
}

// The Settings template-manager read model: both template libraries — the global one and the open
// project's — grouped by kind and scope, plus the default selection. Loads every (kind, scope) once,
// then re-reads the one a `TemplateChanged` names, and the defaults, which a delete may have cleared
// in core. A change in some *other* project is ignored: it belongs to a library this panel is not
// showing. Holds no business logic; kind grouping, name uniqueness, and clearing a deleted default
// all live in the core, and this projects the result and routes writes to the one façade.
export function useTemplates(project: number | null): TemplatesStore {
  const [lists, setLists] = useState<TemplateLists>(EMPTY_LISTS);
  const [defaults, setDefaults] = useState<TemplateDefaults>(NO_DEFAULTS);
  const [error, setError] = useState<string | null>(null);

  // The latest lists, read by `duplicate` for name uniqueness without making it re-created on every
  // refresh (which would not matter, but the ref keeps the action identity stable).
  const listsRef = useLatestRef(lists);

  const fail = useCallback((reason: unknown) => setError(String(reason)), []);

  // The project id a scope addresses: the global library has none, and the project scope is only
  // readable while a project is open.
  const idOf = useCallback(
    (scope: TemplateScope) => (scope === "global" ? null : project),
    [project],
  );

  const loadKind = useCallback(
    (kind: TemplateKind, scope: TemplateScope) => {
      if (scope === "project" && project == null) {
        setLists((prev) => ({ ...prev, [kind]: { ...prev[kind], project: [] } }));
        return;
      }
      listTemplates(kind, idOf(scope))
        .then((rows) => setLists((prev) => ({ ...prev, [kind]: { ...prev[kind], [scope]: rows } })))
        .catch(fail);
    },
    [fail, idOf, project],
  );

  const loadDefaults = useCallback(() => {
    templateDefaults().then(setDefaults).catch(fail);
  }, [fail]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    for (const kind of TEMPLATE_KINDS) {
      for (const scope of TEMPLATE_SCOPES) loadKind(kind, scope);
    }
    loadDefaults();

    // Attach the listener so a create/edit/delete (from this manager or an MCP prompt-template
    // caller) re-reads the affected kind in the scope it changed, plus the defaults.
    onDomainEvent((event) => {
      if (event.type !== "TemplateChanged") return;
      if (event.project == null) loadKind(event.kind, "global");
      else if (event.project === project) loadKind(event.kind, "project");
      else return;
      loadDefaults();
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
  }, [loadKind, loadDefaults, fail, project]);

  const create = useCallback(
    async (
      kind: TemplateKind,
      scope: TemplateScope,
      name: string,
      description: string,
      body: string,
    ) => {
      await templateCreate(
        kind,
        idOf(scope),
        name,
        description.trim() === "" ? null : description,
        body,
      );
    },
    [idOf],
  );

  const remove = useCallback(
    async (kind: TemplateKind, scope: TemplateScope, name: string) => {
      await templateDelete(kind, idOf(scope), name);
    },
    [idOf],
  );

  const duplicate = useCallback(
    async (kind: TemplateKind, scope: TemplateScope, name: string) => {
      const id = idOf(scope);
      const source = await templateRead(kind, id, name);
      // A copy lands beside its source, so uniqueness is decided against that scope's names alone.
      const existing = listsRef.current[kind][scope].map((template) => template.name);
      await templateCreate(
        kind,
        id,
        uniqueCopyName(source.name, existing),
        source.description,
        source.body,
      );
    },
    [idOf, listsRef],
  );

  const setDefault = useCallback((kind: TemplateKind, id: number | null) => {
    setDefaults((prev) => ({ ...prev, [kind]: id }));
    persistThenReconcile(setDefaultTemplate(kind, id), templateDefaults, setDefaults);
  }, []);

  return { lists, defaults, error, create, remove, duplicate, setDefault };
}
