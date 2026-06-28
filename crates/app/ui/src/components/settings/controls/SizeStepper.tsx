import { Minus, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { FONT_SCALE_LABEL, FONT_SCALE_ORDER } from "@/lib/appearance";
import type { FontScale } from "@/domain";

// The A·A·A discrete size picker: − / current step / +, clamped at the ends. The current step
// name is announced politely so the change is legible without sight of the steppers.
export function SizeStepper({
  value,
  onChange,
  ariaLabel,
}: {
  value: FontScale;
  onChange: (value: FontScale) => void;
  ariaLabel: string;
}) {
  const index = FONT_SCALE_ORDER.indexOf(value);
  const atMin = index <= 0;
  const atMax = index >= FONT_SCALE_ORDER.length - 1;

  return (
    <div className="inline-flex items-center gap-1.5" role="group" aria-label={ariaLabel}>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={`Decrease ${ariaLabel}`}
        disabled={atMin}
        onClick={() => onChange(FONT_SCALE_ORDER[index - 1])}
      >
        <Minus />
      </Button>
      <span aria-live="polite" className="w-[5.5rem] text-center text-[0.8125rem] text-foreground">
        {FONT_SCALE_LABEL[value]}
      </span>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={`Increase ${ariaLabel}`}
        disabled={atMax}
        onClick={() => onChange(FONT_SCALE_ORDER[index + 1])}
      >
        <Plus />
      </Button>
    </div>
  );
}
