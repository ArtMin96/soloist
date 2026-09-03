import { useId, useState } from "react";
import { Input } from "@/components/ui/input";
import { normalizeHexColor } from "@/theme/derive";

export function ThemeColorInput({
  label,
  value,
  onChange,
  onHighlight,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  onHighlight?: (active: boolean) => void;
}) {
  const id = useId();
  const [draft, setDraft] = useState(value);
  // A commit that fails to validate leaves `draft` untouched, so `value` only ever moves out from
  // under it on a genuine external change -- tracked here, during render, rather than in an effect,
  // so that reset lands the same render `value` does.
  const [syncedValue, setSyncedValue] = useState(value);
  if (syncedValue !== value) {
    setSyncedValue(value);
    setDraft(value);
  }
  const valid = normalizeHexColor(draft) !== null;

  const commit = () => {
    const normalized = normalizeHexColor(draft);
    if (normalized) onChange(normalized);
    else setDraft(value);
  };

  return (
    <div
      className="flex items-center gap-2"
      onPointerEnter={() => onHighlight?.(true)}
      onPointerLeave={() => onHighlight?.(false)}
    >
      <label htmlFor={id} className="min-w-0 flex-1 truncate text-xs text-muted-foreground">
        {label}
      </label>
      <label className="relative size-7 shrink-0 cursor-pointer overflow-hidden rounded-full border border-border">
        <span className="sr-only">Choose {label}</span>
        <input
          type="color"
          value={value.slice(0, 7)}
          onChange={(event) => onChange(event.target.value)}
          className="absolute -inset-2 size-12 cursor-pointer border-0 p-0"
          aria-label={`Choose ${label}`}
        />
      </label>
      <Input
        id={id}
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") commit();
          if (event.key === "Escape") setDraft(value);
        }}
        aria-invalid={!valid}
        className="h-7 w-24 font-mono text-xs"
        spellCheck={false}
      />
    </div>
  );
}
