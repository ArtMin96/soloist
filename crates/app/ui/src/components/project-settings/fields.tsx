import { useId, type ReactNode } from "react";
import { Field, FieldLabel } from "@/components/ui/field";
import { Switch } from "@/components/ui/switch";

// A labelled vertical field: a caption (with an optional hint line) over its control. The shared
// building block for the command editor and the add-command modal, so their fields read identically.
// Named for the forms it dresses rather than `Field`, which is the shadcn primitive underneath it.
//
// The caption stays a plain element rather than a `FieldLabel`: these fields wrap arbitrary controls
// whose ids this component does not know, and a `<label>` pointing at nothing is a worse promise to
// a screen reader than a caption that never claimed the association.
export function CommandField({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <Field className="gap-1.5">
      <div>
        <div className="text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
          {label}
        </div>
        {hint && <p className="mt-0.5 text-xs text-muted-foreground">{hint}</p>}
      </div>
      {children}
    </Field>
  );
}

// A switch with its label on one line — the inline toggle the command forms use, distinct from the
// settings SettingRow that stacks a description under its label. Here the control is this field's
// own, so the label is a real one bound to it by id.
export function ToggleRow({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  const id = useId();
  return (
    <Field orientation="horizontal" className="justify-between gap-4">
      <FieldLabel htmlFor={id} className="text-[0.8125rem] font-normal text-foreground">
        {label}
      </FieldLabel>
      <Switch id={id} checked={checked} onCheckedChange={onChange} />
    </Field>
  );
}
