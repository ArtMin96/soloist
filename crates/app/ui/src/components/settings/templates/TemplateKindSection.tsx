import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { NullableSelect } from "@/components/settings/controls/NullableSelect";
import { TemplateScopeGroup } from "@/components/settings/templates/TemplateScopeGroup";
import { TEMPLATE_KIND_DESCRIPTION, TEMPLATE_KIND_LABEL } from "@/lib/templates";
import type { TemplateScopeLists } from "@/store/useTemplates";
import type { TemplateKind, TemplateScope } from "@/domain";

// The default-template selection for a seedable kind: the selected id (or null) and the setter. Absent
// for prompts, which have no seed default.
export interface SeedDefault {
  id: number | null;
  onChange: (id: number | null) => void;
}

interface TemplateKindSectionProps {
  kind: TemplateKind;
  templates: TemplateScopeLists;
  /** The scopes to show, in order — the project half is absent while no project is open. */
  scopes: readonly TemplateScope[];
  /** Present only for seedable kinds — renders the "Default template" selector row. */
  seedDefault?: SeedDefault;
  onOpen: (scope: TemplateScope, name: string) => void;
  onDuplicate: (scope: TemplateScope, name: string) => void;
  onNew: (scope: TemplateScope) => void;
}

// One kind's group in the template manager: its explanation, an optional default-template selector
// (seedable kinds only), and one list per scope — the global library and, while a project is open,
// that project's. Pure presentation; grouping, the default, and every write live in the panel and the
// core.
export function TemplateKindSection({
  kind,
  templates,
  scopes,
  seedDefault,
  onOpen,
  onDuplicate,
  onNew,
}: TemplateKindSectionProps) {
  // A seed default is global-only, so it is chosen from — and only offered alongside — the global
  // library. A default that points at a since-deleted template resolves to nothing; show it as unset
  // rather than a dangling value the select can't render.
  const selected =
    seedDefault && templates.global.some((template) => template.id === seedDefault.id)
      ? String(seedDefault.id)
      : null;

  return (
    <SettingsSection
      title={TEMPLATE_KIND_LABEL[kind]}
      description={TEMPLATE_KIND_DESCRIPTION[kind]}
    >
      {seedDefault && templates.global.length > 0 && (
        <SettingRow
          label="Default template"
          description="New documents created empty are seeded from this template. Global templates only."
        >
          <NullableSelect
            value={selected}
            options={[
              { value: null, label: "None" },
              ...templates.global.map((template) => ({
                value: String(template.id),
                label: template.name,
              })),
            ]}
            onValueChange={(value) => seedDefault.onChange(value == null ? null : Number(value))}
            ariaLabel={`Default ${TEMPLATE_KIND_LABEL[kind].toLowerCase()} template`}
            className="w-48"
          />
        </SettingRow>
      )}

      {scopes.map((scope) => (
        <TemplateScopeGroup
          key={scope}
          kind={kind}
          scope={scope}
          templates={templates[scope]}
          onOpen={(name) => onOpen(scope, name)}
          onDuplicate={(name) => onDuplicate(scope, name)}
          onNew={() => onNew(scope)}
        />
      ))}
    </SettingsSection>
  );
}
