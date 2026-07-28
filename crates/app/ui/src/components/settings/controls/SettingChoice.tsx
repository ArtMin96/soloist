import { useId } from "react";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { cn } from "@/lib/utils";

// One option: what it stores, what it is called, and one line saying exactly what choosing it
// means. The description is part of the option, not a footnote about the group, because the whole
// point of this control is that the names alone do not separate the choices.
export interface Choice<T extends string> {
  value: T;
  label: string;
  description: string;
}

// A vertical list of mutually exclusive choices, each carrying its own description. The affordance
// for a setting whose options cannot be told apart by name — every description stays on screen, so
// the choices are compared side by side instead of one at a time behind a dropdown. The current
// choice is marked by a full border and a tinted fill, never a side stripe, and the whole row is
// the label, so the description selects it too.
//
// Each radio is named by its label alone and described by its own line, so a screen reader
// announces "All, radio" and then what All admits, rather than one run-on string.
export function SettingChoice<T extends string>({
  value,
  choices,
  onChange,
  ariaLabel,
}: {
  value: T;
  choices: Choice<T>[];
  onChange: (value: T) => void;
  ariaLabel: string;
}) {
  const group = useId();
  return (
    <RadioGroup value={value} onValueChange={(next) => onChange(next as T)} aria-label={ariaLabel}>
      {choices.map((choice) => {
        const id = `${group}-${choice.value}`;
        return (
          <label
            key={choice.value}
            htmlFor={id}
            className={cn(
              "flex cursor-pointer items-start gap-2.5 rounded-md border px-3 py-2.5",
              "transition-colors duration-[var(--dur-fast)]",
              value === choice.value
                ? "border-primary bg-muted/50"
                : "border-border hover:bg-muted/40",
            )}
          >
            <RadioGroupItem
              id={id}
              value={choice.value}
              aria-labelledby={`${id}-label`}
              aria-describedby={`${id}-description`}
              className="mt-0.5"
            />
            <span className="flex flex-col gap-0.5">
              <span id={`${id}-label`} className="text-[0.8125rem] font-medium text-foreground">
                {choice.label}
              </span>
              <span id={`${id}-description`} className="text-xs text-muted-foreground">
                {choice.description}
              </span>
            </span>
          </label>
        );
      })}
    </RadioGroup>
  );
}
