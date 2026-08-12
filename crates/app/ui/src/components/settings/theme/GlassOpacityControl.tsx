import { useState } from "react";
import { RotateCcw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { GLASS_OPACITY } from "@/theme/constraints";

// The thumb and the readout follow the pointer from local state, so dragging never waits on the
// durable write. Only a committed value — pointer release, or a keyboard step — reaches the core.
export function GlassOpacityControl({
  value,
  onChange,
  onReset,
}: {
  value: number;
  onChange: (value: number) => Promise<void>;
  onReset: () => void;
}) {
  const [persisted, setPersisted] = useState(value);
  const [live, setLive] = useState(value);
  if (persisted !== value) {
    setPersisted(value);
    setLive(value);
  }

  return (
    <div className="flex items-center gap-3">
      <output className="w-9 text-right font-mono text-xs text-foreground" htmlFor="glass-opacity">
        {live}%
      </output>
      <Slider
        id="glass-opacity"
        value={[live]}
        min={GLASS_OPACITY.min}
        max={GLASS_OPACITY.max}
        step={GLASS_OPACITY.step}
        onValueChange={([next]) => {
          if (next !== undefined) setLive(next);
        }}
        onValueCommit={([next]) => {
          if (next !== undefined) void onChange(next).catch(() => setLive(value));
        }}
        aria-label="Glass opacity"
        className="w-36"
      />
      <Button
        variant="ghost"
        size="icon-sm"
        onClick={onReset}
        disabled={live === GLASS_OPACITY.default}
        aria-label="Reset glass opacity"
      >
        <RotateCcw />
      </Button>
    </div>
  );
}
