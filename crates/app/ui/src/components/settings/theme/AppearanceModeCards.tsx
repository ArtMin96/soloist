import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import type { Theme } from "@/domain";
import { SchemePreview, type ThemePreviewPalette } from "./ThemePreview";

const MODES: ReadonlyArray<{ value: Theme; label: string }> = [
  { value: "system", label: "System" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

export function AppearanceModeCards({
  value,
  light,
  dark,
  onChange,
}: {
  value: Theme;
  light: ThemePreviewPalette;
  dark: ThemePreviewPalette;
  onChange: (value: Theme) => void;
}) {
  return (
    <ToggleGroup
      type="single"
      value={value}
      onValueChange={(next) => {
        if (next) onChange(next as Theme);
      }}
      variant="outline"
      spacing={2}
      role="radiogroup"
      aria-label="Color scheme"
      className="grid w-full grid-cols-3 items-stretch"
    >
      {MODES.map((mode) => (
        <ToggleGroupItem
          key={mode.value}
          value={mode.value}
          role="radio"
          aria-checked={value === mode.value}
          aria-label={mode.label}
          className="h-auto min-w-0 flex-col gap-2 p-2 text-xs data-[state=on]:border-ring data-[state=on]:bg-accent"
        >
          <span className="h-24 w-full">
            <SchemePreview light={light} dark={dark} mode={mode.value} />
          </span>
          <span>{mode.label}</span>
        </ToggleGroupItem>
      ))}
    </ToggleGroup>
  );
}
