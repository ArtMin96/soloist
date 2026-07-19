import { useState } from "react";
import { TemplateCreateForm } from "@/components/settings/templates/TemplateCreateForm";
import { TemplateEditor } from "@/components/settings/templates/TemplateEditor";
import {
  type SeedDefault,
  TemplateKindSection,
} from "@/components/settings/templates/TemplateKindSection";
import { TEMPLATE_KINDS } from "@/lib/templates";
import { useTemplateEditor } from "@/store/useTemplateEditor";
import { useTemplates } from "@/store/useTemplates";
import type { TemplateKind } from "@/domain";

// The Templates tab: a manager for the global template library grouped by kind. It drills in — a
// browse view (the grouped lists plus a default-template selector for the seedable kinds) opens into
// a full editor for one template, or a create form — rather than a cramped master-detail inside the
// narrow settings column. Every read/write routes through the two hooks to the one façade; kind
// grouping, name uniqueness, the revision guard, and clearing a deleted default live in the core.
// This surface also delivers the reserved prompt-templates view (the Prompt kind).
export function TemplatesPanel() {
  const { lists, defaults, error, create, remove, duplicate, setDefault } = useTemplates();
  const editor = useTemplateEditor();
  const [creating, setCreating] = useState<TemplateKind | null>(null);

  const open = (kind: TemplateKind) => (name: string) => {
    setCreating(null);
    editor.open(kind, name);
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

  if (editor.kind != null && editor.name != null) {
    return editor.initialBody == null ? (
      <p className="py-3 text-[0.8125rem] text-muted-foreground">
        {editor.loading ? "Loading…" : (editor.error ?? "Not found.")}
      </p>
    ) : (
      <TemplateEditor
        kind={editor.kind}
        name={editor.name}
        initialBody={editor.initialBody}
        initialDescription={editor.initialDescription}
        revision={editor.baseRevision}
        mountKey={editor.mountKey}
        conflict={editor.conflict}
        error={editor.error}
        onSave={editor.save}
        onReload={editor.reload}
        onBack={editor.close}
        onDelete={() => {
          const { kind, name } = editor;
          if (kind != null && name != null) void remove(kind, name).then(editor.close);
        }}
      />
    );
  }

  if (creating != null) {
    return (
      <TemplateCreateForm
        kind={creating}
        onCancel={() => setCreating(null)}
        onCreate={(name, description, body) =>
          create(creating, name, description, body).then(() => setCreating(null))
        }
      />
    );
  }

  return (
    <div className="flex flex-col">
      {error && (
        <p className="mb-3 text-[0.8125rem] text-destructive" aria-live="polite">
          {error}
        </p>
      )}
      {TEMPLATE_KINDS.map((kind) => (
        <TemplateKindSection
          key={kind}
          kind={kind}
          templates={lists[kind]}
          seedDefault={seedDefaultFor(kind)}
          onOpen={open(kind)}
          onDuplicate={(name) => void duplicate(kind, name)}
          onNew={() => setCreating(kind)}
        />
      ))}
    </div>
  );
}
