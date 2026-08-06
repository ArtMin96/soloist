import { CHANGE } from "@/lib/git";
import { cn } from "@/lib/utils";
import type { ChangeKind } from "@/domain";

/**
 * The letter version control prints for a change, in the tone that change owns. The letter is
 * the meaning and the colour only reinforces it, so the row still reads in grayscale or to an
 * eye that cannot separate the hues; the word itself is the element's accessible name.
 */
export function StatusLetter({ change, className }: { change: ChangeKind; className?: string }) {
  const display = CHANGE[change];
  return (
    <span
      role="img"
      aria-label={display.label}
      title={display.label}
      className={cn(
        // Mono at the data step: a column of single letters down the rail's edge is an aligned
        // value, which is the one place DESIGN.md reserves the monospace face for.
        "w-3.5 shrink-0 text-center font-mono text-[0.8125rem] leading-none font-medium",
        display.toneClass,
        className,
      )}
    >
      {display.letter}
    </span>
  );
}
