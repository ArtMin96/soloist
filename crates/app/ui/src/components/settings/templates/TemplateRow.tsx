import { Copy } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { TemplateSummary } from "@/domain";

// One template in a scope group's list: its name and description open the editor, with a duplicate
// action trailing. Pure presentation — every action is the panel's.
export function TemplateRow({
  template,
  onOpen,
  onDuplicate,
}: {
  template: TemplateSummary;
  onOpen: () => void;
  onDuplicate: () => void;
}) {
  return (
    <div className="flex items-center justify-between gap-3 py-2.5">
      <button
        type="button"
        onClick={onOpen}
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
        onClick={onDuplicate}
        aria-label={`Duplicate ${template.name}`}
      >
        <Copy aria-hidden />
      </Button>
    </div>
  );
}
