import { useEffect, useRef } from "react";
import { ChevronDown, ChevronUp, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";

interface FindBarProps {
  query: string;
  onChange: (query: string) => void;
  onFindNext: () => void;
  onFindPrevious: () => void;
  onClose: () => void;
}

// A floating find bar anchored to the top-right of the terminal area. Springs in from above
// on mount (translate-y animation), focuses the input immediately, and closes on Escape.
// Enter / Shift+Enter cycle matches; the toolbar buttons do the same without touching the
// PTY keystroke stream (the Ctrl+F chord is intercepted upstream by useTerminalHotkeys).
export function FindBar({ query, onChange, onFindNext, onFindPrevious, onClose }: FindBarProps) {
  const inputRef = useRef<HTMLInputElement>(null);

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
