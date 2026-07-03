import { Plus, X } from "lucide-react";
import { Input } from "@/components/ui/input";
import { appendRow, removeItem, setItem, type Row } from "@/store/scratchpadForm";

interface FieldListProps {
  label: string;
  items: Row[];
  placeholder: string;
  onChange: (items: Row[]) => void;
}

// A labelled list of single-line entries (the disciplined document's plan / acceptance-criteria /
// risks): one input per item, an inline remove, and an add row. Presentational — the array and its
// updates flow through props using the pure list helpers, so the editor stays free of mutation logic.
export function FieldList({ label, items, placeholder, onChange }: FieldListProps) {
  return (
    <fieldset className="flex flex-col gap-1.5">
      <legend className="text-[0.6875rem] font-[550] text-muted-foreground">{label}</legend>
      <div className="flex flex-col gap-1">
        {items.map((item, index) => (
          // Keyed by the row's stable id, not its index, so removing a middle row keeps each input
          // bound to its own DOM node (focus/caret don't jump to the wrong row).
          <div key={item.id} className="flex items-center gap-1">
            <Input
              value={item.value}
              placeholder={placeholder}
              aria-label={`${label} ${index + 1}`}
              onChange={(event) => onChange(setItem(items, index, event.target.value))}
              className="h-7 text-[0.8125rem]"
            />
            <button
              type="button"
              aria-label={`Remove ${label} ${index + 1}`}
              onClick={() => onChange(removeItem(items, index))}
              disabled={items.length === 1}
              className="flex size-7 shrink-0 items-center justify-center rounded-md text-muted-foreground outline-none hover:bg-sidebar-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-sidebar-ring disabled:pointer-events-none disabled:opacity-40"
            >
              <X aria-hidden className="size-3.5" />
            </button>
          </div>
        ))}
      </div>
      <button
        type="button"
        onClick={() => onChange(appendRow(items))}
        className="flex w-fit items-center gap-1 rounded-md px-1.5 py-1 text-[0.6875rem] text-muted-foreground outline-none hover:bg-sidebar-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-sidebar-ring"
      >
        <Plus aria-hidden className="size-3" /> Add
      </button>
    </fieldset>
  );
}
