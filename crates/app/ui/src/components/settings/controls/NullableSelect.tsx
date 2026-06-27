import { SettingSelect } from "@/components/settings/controls/SettingSelect";
import type { Option } from "@/lib/appearance";

// Radix Select cannot use null/empty as an item value, so a nullable choice rides a private
// sentinel mapped to/from null here — the one place that mapping lives, instead of each panel
// (font family, default editor/terminal, summarizer model) re-inventing its own sentinel.
const NULL_SENTINEL = "__null__";

export function NullableSelect<T extends string>({
  value,
  options,
  onValueChange,
  ariaLabel,
  className,
}: {
  value: T | null;
  options: Option<T | null>[];
  onValueChange: (value: T | null) => void;
  ariaLabel: string;
  className?: string;
}) {
  return (
    <SettingSelect
      value={value ?? NULL_SENTINEL}
      options={options.map((option) => ({
        value: option.value ?? NULL_SENTINEL,
        label: option.label,
      }))}
      onValueChange={(next) => onValueChange(next === NULL_SENTINEL ? null : (next as T))}
      ariaLabel={ariaLabel}
      className={className}
    />
  );
}
