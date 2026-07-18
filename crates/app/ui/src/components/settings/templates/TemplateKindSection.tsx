import { Copy, Plus } from "lucide-react";
import { NullableSelect } from "@/components/settings/controls/NullableSelect";
import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { Button } from "@/components/ui/button";
import { TEMPLATE_KIND_DESCRIPTION, TEMPLATE_KIND_LABEL } from "@/lib/templates";
import type { TemplateKind, TemplateSummary } from "@/domain";

// The default-template selection for a seedable kind: the selected id (or null) and the setter. Absent
// for prompts, which have no seed default.
export interface SeedDefault {
  id: number | null;
  onChange: (id: number | null) => void;
}

interface TemplateKindSectionProps {
  kind: TemplateKind;
  templates: TemplateSummary[];
  /** Present only for seedable kinds — renders the "Default template" selector row. */
  seedDefault?: SeedDefault;
  onOpen: (name: string) => void;
  onDuplicate: (name: string) => void;
  onNew: () => void;
}

// One kind's group in the template manager: its explanation, an optional default-template selector
// (seedable kinds only), the list of templates as openable rows with a duplicate action, and a "New
// template" affordance. Pure presentation — grouping, the default, and every write live in the panel
// and the core; this renders the projection.
export function TemplateKindSection({
  kind,
  templates,
  seedDefault,
  onOpen,
  onDuplicate,
  onNew,
}: TemplateKindSectionProps) {
  // A default that points at a since-deleted template resolves to nothing — show it as unset rather
  // than a dangling value the select can't render.
  const selected =
    seedDefault && templates.some((template) => template.id === seedDefault.id)
      ? String(seedDefault.id)
      : null;

  return (
    <SettingsSection
      title={TEMPLATE_KIND_LABEL[kind]}
      description={TEMPLATE_KIND_DESCRIPTION[kind]}
    >
      {seedDefault && templates.length > 0 && (
        <SettingRow
          label="Default template"
          description="New documents created empty are seeded from this template."
        >
          <NullableSelect
            value={selected}
            options={[
              { value: null, label: "None" },
              ...templates.map((template) => ({
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

      {templates.length === 0 ? (
        <p className="py-3 text-xs text-muted-foreground">
          No {TEMPLATE_KIND_LABEL[kind].toLowerCase()} templates yet.
        </p>
      ) : (
        templates.map((template) => (
          <div key={template.id} className="flex items-center justify-between gap-3 py-2.5">
            <button
              type="button"
              onClick={() => onOpen(template.name)}
              className="min-w-0 flex-1 rounded-md text-left focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
            >
              <div className="truncate text-[0.8125rem] text-foreground">{template.name}</div>
              {template.description && (
                <p className="truncate text-xs text-muted-foreground">{template.description}</p>
              )}
            </button>
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={() => onDuplicate(template.name)}
              aria-label={`Duplicate ${template.name}`}
            >
              <Copy aria-hidden />
            </Button>
          </div>
        ))
      )}

      <div className="flex py-2">
        <Button variant="ghost" size="sm" onClick={onNew}>
          <Plus aria-hidden /> New template
        </Button>
      </div>
    </SettingsSection>
  );
}
