import { Plus } from "lucide-react";
import { TemplateRow } from "@/components/settings/templates/TemplateRow";
import { Button } from "@/components/ui/button";
import { TEMPLATE_SCOPE_EMPTY, TEMPLATE_SCOPE_LABEL, templateGroupLabel } from "@/lib/templates";
import type { TemplateKind, TemplateScope, TemplateSummary } from "@/domain";

interface TemplateScopeGroupProps {
  kind: TemplateKind;
  scope: TemplateScope;
  templates: TemplateSummary[];
  onOpen: (name: string) => void;
  onDuplicate: (name: string) => void;
  onNew: () => void;
}

// One scope's half of a kind section: a quiet sentence-case label, that library's templates as
// openable rows, and the affordance that adds to *this* scope. The global library and the open
// project's are separate lists — a name can exist in both — so each group states which it is and
// keeps its own empty state rather than the two sharing one "nothing here". Pure presentation.
export function TemplateScopeGroup({
  kind,
  scope,
  templates,
  onOpen,
  onDuplicate,
  onNew,
}: TemplateScopeGroupProps) {
  return (
    <div role="group" aria-label={templateGroupLabel(kind, scope)}>
      <div className="pt-3 pb-1 text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
        {TEMPLATE_SCOPE_LABEL[scope]}
      </div>
      <div className="divide-y divide-border">
        {templates.length === 0 ? (
          <p className="py-2.5 text-xs text-muted-foreground">{TEMPLATE_SCOPE_EMPTY[scope]}</p>
        ) : (
          templates.map((template) => (
            <TemplateRow
              key={template.id}
              template={template}
              onOpen={() => onOpen(template.name)}
              onDuplicate={() => onDuplicate(template.name)}
            />
          ))
        )}
        <div className="flex py-2">
          <Button variant="ghost" size="sm" onClick={onNew}>
            <Plus aria-hidden /> New template
          </Button>
        </div>
      </div>
    </div>
  );
}
