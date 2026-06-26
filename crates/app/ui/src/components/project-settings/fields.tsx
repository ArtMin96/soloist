import type { ReactNode } from "react";
import { Switch } from "@/components/ui/switch";

// A labelled vertical field: a caption (with an optional hint line) over its control. The shared
// building block for the command editor and the add-command modal, so their fields read identically.
export function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <div>
        <div className="text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
          {label}
        </div>
        {hint && <p className="mt-0.5 text-xs text-muted-foreground">{hint}</p>}
      </div>
      {children}
    </div>
  );
}

// A switch with its label on one line — the inline toggle the command forms use, distinct from the
// settings SettingRow that stacks a description under its label.
export function ToggleRow({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex items-center justify-between gap-4">
      <span className="text-[0.8125rem] text-foreground">{label}</span>
      <Switch checked={checked} onCheckedChange={onChange} aria-label={label} />
    </label>
  );
}
