import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { Option } from "@/lib/appearance";

// How a stored value the option set no longer offers reads. Radix has no item to render for it and
// no placeholder to fall back on (a placeholder only shows for an empty value), so the trigger comes
// out blank — indistinguishable from nothing being selected. Naming the value keeps the two apart:
// the choice is still on record, it just cannot be served right now.
function unavailable(value: string): string {
  return `${value} (unavailable)`;
}

// A labeled dropdown over a fixed option set, for the discrete pickers that read better as a
// list than a segmented row (font family, weights, line height, letter spacing). Values are the
// option strings; the caller maps any non-string domain value to/from a string at the edge.
export function SettingSelect({
  value,
  options,
  onValueChange,
  ariaLabel,
  className,
}: {
  value: string;
  options: Option<string>[];
  onValueChange: (value: string) => void;
  ariaLabel: string;
  className?: string;
}) {
  const offered = options.some((option) => option.value === value);

  return (
    <Select value={value} onValueChange={onValueChange}>
      <SelectTrigger size="sm" aria-label={ariaLabel} className={className}>
        {/* Children only when there is no item to speak for the value: given any, Radix stops
            portaling the selected item's own text in here. */}
        <SelectValue>{offered ? undefined : unavailable(value)}</SelectValue>
      </SelectTrigger>
      <SelectContent>
        <SelectGroup>
          {options.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectGroup>
      </SelectContent>
    </Select>
  );
}
