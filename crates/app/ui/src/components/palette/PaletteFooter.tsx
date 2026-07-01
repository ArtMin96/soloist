import type { ReactNode } from "react";
import { Kbd } from "@/components/ui/kbd";

/** One key hint in a palette footer: the chord glyphs and what pressing it does. */
export interface PaletteHintData {
  keys: string;
  label: string;
}

// A single key hint — a key cap (or chord) followed by its action label.
export function PaletteHint({ keys, label }: PaletteHintData) {
  return (
    <span className="flex items-center gap-1.5">
      <Kbd>{keys}</Kbd>
      {label}
    </span>
  );
}

// The footer bar shared by every command palette: key hints on the left and, optionally, the
// scoped target the palette acts within on the right (e.g. the active project). One source so the
// hint row never drifts between palettes.
export function PaletteFooter({ hints, target }: { hints: PaletteHintData[]; target?: ReactNode }) {
  return (
    <div className="flex items-center gap-3 border-t px-3 py-2 text-xs text-muted-foreground">
      {hints.map((hint) => (
        <PaletteHint key={hint.label} {...hint} />
      ))}
      {target && <span className="ml-auto min-w-0 truncate">▸ {target}</span>}
    </div>
  );
}
