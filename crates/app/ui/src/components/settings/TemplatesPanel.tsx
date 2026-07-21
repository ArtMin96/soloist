import { useEffect, useState } from "react";
import { TemplateCreateForm } from "@/components/settings/templates/TemplateCreateForm";
import { TemplateEditor } from "@/components/settings/templates/TemplateEditor";
import {
  type SeedDefault,
  TemplateKindSection,
} from "@/components/settings/templates/TemplateKindSection";
import type { SettingsPanelProps } from "@/components/settings/tabs";
import { TEMPLATE_KINDS, TEMPLATE_SCOPES } from "@/lib/templates";
import { useTemplateEditor } from "@/store/useTemplateEditor";
import { useTemplateRender } from "@/store/useTemplateRender";
import { useTemplates } from "@/store/useTemplates";
import type { TemplateKind, TemplateScope } from "@/domain";

// What a "New template" affordance opened: which kind, and which library it will land in.
interface Creating {
  kind: TemplateKind;
  scope: TemplateScope;
}

// The Templates tab: a manager for both template libraries — the one shared across every project and
// the open project's own — grouped by kind and, within a kind, by scope. The split is the point: MCP
// writes default to the project scope, so a panel that only ever read the global library showed a
// user none of what their agents authored. It drills in — a browse view (the grouped lists plus a
// default-template selector for the seedable kinds) opens into a full editor for one template, or a
// create form — rather than a cramped master-detail inside the narrow settings column. Every
// read/write routes through the two hooks to the one façade; kind grouping, scope isolation, name
// uniqueness, the revision guard, and clearing a deleted default live in the core. This surface also
// delivers the reserved prompt-templates view (the Prompt kind).
export function TemplatesPanel({ project, onWideChange }: SettingsPanelProps) {
  const { lists, defaults, error, create, remove, duplicate, setDefault } = useTemplates(project);
  const editor = useTemplateEditor(project);
  const preview = useTemplateRender({
    kind: editor.kind,
    scope: editor.scope,
    name: editor.name,
    project,
    revision: editor.baseRevision,
  });
  const [creating, setCreating] = useState<Creating | null>(null);
  // Mirrors the two conditions below that decide what to render: a create form or an opened editor
  // both need the full-width builder layout in place of the standard settings column.
  const wide =
    creating != null || (editor.kind != null && editor.scope != null && editor.name != null);
  useEffect(() => {
    onWideChange?.(wide);
  }, [wide, onWideChange]);
  // Delete and duplicate reject with the core's reason (a name past the length cap, a template already
  // removed elsewhere). The hook reports only read failures, so this surface holds the reason for the
  // writes it starts — the same way the create form keeps its own rejection on screen.
  const [writeError, setWriteError] = useState<string | null>(null);

  // Without a project open there is no "this project" library to show, so the kind sections carry the
  // global group alone rather than a group that could never hold anything.
  const scopes =
    project == null ? TEMPLATE_SCOPES.filter((scope) => scope !== "project") : TEMPLATE_SCOPES;

  const closeEditor = () => {
    setWriteError(null);
    editor.close();
  };

  const open = (kind: TemplateKind) => (scope: TemplateScope, name: string) => {
    setCreating(null);
    setWriteError(null);
    editor.open(kind, scope, name);
  };

  const seedDefaultFor = (kind: TemplateKind): SeedDefault | undefined => {
    switch (kind) {
      case "scratchpad":
        return { id: defaults.scratchpad, onChange: (id) => setDefault("scratchpad", id) };
      case "todo":
        return { id: defaults.todo, onChange: (id) => setDefault("todo", id) };
      case "prompt":
        return undefined;
    }
  };

  if (editor.kind != null && editor.scope != null && editor.name != null) {
    const { kind, scope, name } = editor;
    return editor.initialBody == null ? (
      <p className="py-3 text-[0.8125rem] text-muted-foreground">
        {editor.loading ? "Loading…" : (editor.error ?? "Not found.")}
      </p>
    ) : (
      <TemplateEditor
        kind={kind}
        scope={scope}
        name={name}
        initialBody={editor.initialBody}
        initialDescription={editor.initialDescription}
        revision={editor.baseRevision}
        mountKey={editor.mountKey}
        conflict={editor.conflict}
        error={writeError ?? editor.error}
        preview={
          preview.renderable
            ? {
                placeholders: editor.placeholders,
                values: preview.values,
                rendered: preview.rendered,
                error: preview.error,
                onValueChange: preview.setValue,
              }
            : null
        }
        onSave={editor.save}
        onReload={editor.reload}
        onBack={closeEditor}
        onDelete={() => {
          setWriteError(null);
          // Only a resolved delete closes the editor — a refused one keeps the template on screen
          // with the reason, so the user can retry or go back deliberately.
          remove(kind, scope, name)
            .then(closeEditor)
            .catch((reason) => setWriteError(String(reason)));
        }}
      />
    );
  }

  if (creating != null) {
    const { kind, scope } = creating;
    return (
      <TemplateCreateForm
        kind={kind}
        scope={scope}
        onCancel={() => setCreating(null)}
        onCreate={(name, description, body) =>
          create(kind, scope, name, description, body).then(() => setCreating(null))
        }
      />
    );
  }

  return (
    <div className="flex flex-col">
      {(writeError ?? error) && (
        <p className="mb-3 text-[0.8125rem] text-destructive" aria-live="polite">
          {writeError ?? error}
        </p>
      )}
      {TEMPLATE_KINDS.map((kind) => (
        <TemplateKindSection
          key={kind}
          kind={kind}
          templates={lists[kind]}
          scopes={scopes}
          seedDefault={seedDefaultFor(kind)}
          onOpen={open(kind)}
          onDuplicate={(scope, name) => {
            setWriteError(null);
            duplicate(kind, scope, name).catch((reason) => setWriteError(String(reason)));
          }}
          onNew={(scope) => setCreating({ kind, scope })}
        />
      ))}
    </div>
  );
}
