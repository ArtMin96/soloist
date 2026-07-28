import { useEffect, useRef } from "react";
import { ChevronDown, ChevronUp, X } from "lucide-react";
import { NO_ACTIVE_MATCH, type SearchMatches } from "@/components/terminal/terminalSearch";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";

interface FindBarProps {
  query: string;
  matches: SearchMatches;
  onChange: (query: string) => void;
  onFindNext: () => void;
  onFindPrevious: () => void;
  onClose: () => void;
}

// What the tally reads, or null when there is nothing worth saying. An empty query has not asked a
// question yet, so it gets no answer rather than a zero. Past the highlight ceiling the emulator
// stops tracking which match is current, so the total is reported without a position instead of
// claiming one — the honest reading, and the one that explains why stepping stops renumbering.
function matchSummary(query: string, matches: SearchMatches): string | null {
  if (!query) return null;
  if (matches.count === 0) return "No results";
  if (matches.index === NO_ACTIVE_MATCH) return `${matches.count} matches`;
  return `${matches.index + 1} of ${matches.count}`;
}

// A floating find bar anchored to the top-right of the terminal area. Springs in from above
// on mount (translate-y animation), focuses the input immediately, and closes on Escape.
// Enter / Shift+Enter cycle matches; the toolbar buttons do the same without touching the
// PTY keystroke stream (the Ctrl+F chord is intercepted upstream by useTerminalHotkeys).
export function FindBar({
  query,
  matches,
  onChange,
  onFindNext,
  onFindPrevious,
  onClose,
}: FindBarProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const summary = matchSummary(query, matches);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  function handleKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (event.shiftKey) onFindPrevious();
      else onFindNext();
    }
  }

  return (
    <search
      className={cn(
        "absolute top-0 right-3 z-10",
        "flex items-center gap-0.5 rounded-b-md border border-t-0 border-border/60",
        "bg-sidebar px-2 py-1 shadow-[var(--shadow-overlay)]",
        "animate-in slide-in-from-top-2 duration-[var(--dur-select)] ease-out-quint",
      )}
      aria-label="Find in terminal"
    >
      <Input
        ref={inputRef}
        type="text"
        value={query}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="Find…"
        aria-label="Search query"
        className="h-6 w-40 border-0 bg-transparent px-0 text-[0.8125rem] shadow-none focus-visible:ring-0"
      />
      {/* Always mounted, so a screen reader is watching the region before the first count lands in
          it; an element that appears together with its text is announced unreliably. Tabular
          figures and a floor on the width keep the toolbar from shifting as the tally changes. */}
      <span
        role="status"
        aria-live="polite"
        className={cn(
          "min-w-14 shrink-0 pl-1.5 text-right tabular-nums",
          "text-[0.6875rem] font-[550] tracking-[0.01em] text-muted-foreground",
        )}
      >
        {summary}
      </span>
      <Separator orientation="vertical" className="mx-1.5 h-3.5 bg-border/60" />
      <Button variant="ghost" size="icon-xs" onClick={onFindPrevious} aria-label="Previous match">
        <ChevronUp />
      </Button>
      <Button variant="ghost" size="icon-xs" onClick={onFindNext} aria-label="Next match">
        <ChevronDown />
      </Button>
      <Separator orientation="vertical" className="mx-0.5 h-3.5 bg-border/60" />
      <Button variant="ghost" size="icon-xs" onClick={onClose} aria-label="Close find">
        <X />
      </Button>
    </search>
  );
}
