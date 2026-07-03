import { useEffect, useRef } from "react";
import { ChevronDown, ChevronUp, X } from "lucide-react";
import { Button } from "@/components/ui/button";
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
      <input
        ref={inputRef}
        type="text"
        value={query}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="Find…"
        aria-label="Search query"
        className={cn(
          "w-40 bg-transparent text-[0.8125rem] outline-none",
          "placeholder:text-muted-foreground",
        )}
      />
      <div className="mx-1.5 h-3.5 w-px bg-border/60" aria-hidden />
      <Button
        variant="ghost"
        size="icon-sm"
        onClick={onFindPrevious}
        aria-label="Previous match"
        className="size-6"
      >
        <ChevronUp className="size-3.5" />
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        onClick={onFindNext}
        aria-label="Next match"
        className="size-6"
      >
        <ChevronDown className="size-3.5" />
      </Button>
      <div className="mx-0.5 h-3.5 w-px bg-border/60" aria-hidden />
      <Button
        variant="ghost"
        size="icon-sm"
        onClick={onClose}
        aria-label="Close find"
        className="size-6"
      >
        <X className="size-3.5" />
      </Button>
    </search>
  );
}
