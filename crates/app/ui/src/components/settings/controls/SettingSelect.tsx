import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { Option } from "@/lib/appearance";

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
  return (
    <Select value={value} onValueChange={onValueChange}>
      <SelectTrigger size="sm" aria-label={ariaLabel} className={className}>
        <SelectValue />
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
