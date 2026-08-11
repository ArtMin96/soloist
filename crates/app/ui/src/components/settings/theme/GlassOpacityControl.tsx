import { RotateCcw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { GLASS_OPACITY } from "@/theme/constraints";

export function GlassOpacityControl({
  value,
  onChange,
  onReset,
}: {
  value: number;
  onChange: (value: number) => void;
  onReset: () => void;
}) {
  return (
    <div className="flex items-center gap-3">
      <output className="w-9 text-right font-mono text-xs text-foreground" htmlFor="glass-opacity">
        {value}%
      </output>
      <Slider
        id="glass-opacity"
        value={[value]}
        min={GLASS_OPACITY.min}
        max={GLASS_OPACITY.max}
        step={GLASS_OPACITY.step}
        onValueChange={([next]) => {
          if (next !== undefined) onChange(next);
        }}
        aria-label="Glass opacity"
        className="w-36"
      />
      <Button
        variant="ghost"
        size="icon-sm"
        onClick={onReset}
        disabled={value === GLASS_OPACITY.default}
        aria-label="Reset glass opacity"
      >
        <RotateCcw />
      </Button>
    </div>
  );
}
