import { useRef, useState, type KeyboardEvent } from "react";
import { Input } from "@/components/ui/input";
import { humanizeName } from "@/lib/humanize";

interface ScratchpadTitleProps {
  /** The document's raw name handle — what a rename edits, and what the title is humanized from. */
  name: string;
  /**
   * Commits a rename. Resolves once the core accepted it; rejects with the refusal (a taken name,
   * an invalid one) so the field can stay open showing the user's text.
   */
  onRename: (to: string) => Promise<void>;
}

// The open scratchpad's title, renamed in place: it rests as the humanized title and becomes a text
// field on click — Enter commits, Escape cancels, and clicking away commits, the Finder idiom. The
// field seeds with the raw handle rather than the humanized title, because the handle is what a
// rename writes and what MCP calls address; committing it unchanged is a no-op that never touches
// the core. A refusal keeps the field open with the typed text and names the reason beneath it, so
// nothing the user wrote is lost to an error. View state only — the rename itself is the parent's.
export function ScratchpadTitle({ name, onRename }: ScratchpadTitleProps) {
  const [draft, setDraft] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Guards the commit against running twice for one intent (Enter, then the blur that follows it),
  // and against a second commit while one is in flight. The field is deliberately never disabled
  // while saving: a rename is a local round trip, and disabling it would blur the field the user
  // needs to be in if the core refuses the name.
  const committing = useRef(false);

  const title = humanizeName(name);

  function cancel() {
    setDraft(null);
    setError(null);
  }

  async function commit() {
    if (draft === null || committing.current) return;
    const next = draft.trim();
    if (next === "" || next === name) {
      cancel();
      return;
    }
    committing.current = true;
    setError(null);
    try {
      await onRename(next);
      setDraft(null);
    } catch (reason) {
      setError(String(reason));
    } finally {
      committing.current = false;
    }
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Enter") {
      event.preventDefault();
      void commit();
    } else if (event.key === "Escape") {
      event.preventDefault();
      cancel();
    }
  }

  if (draft === null) {
    return (
      // The negative inset cancels the button's own padding, so the resting title sits on the
      // header's text edge and only its hover fill extends past it.
      <h2 className="-ml-1.5 min-w-0 flex-1">
        <button
          type="button"
          onClick={() => setDraft(name)}
          title="Rename"
          aria-label={`Rename scratchpad ${title}`}
          className="max-w-full truncate rounded-md px-1.5 py-0.5 text-[0.9375rem] leading-5 font-[550] tracking-[-0.005em] transition-colors duration-[var(--dur-fast)] ease-out-quint hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none"
        >
          {title}
        </button>
      </h2>
    );
  }

  return (
    <div className="relative min-w-0 flex-1">
      <Input
        // The field replaces the title the user just clicked, so focus has to follow it there —
        // without this the rename could be opened but not typed.
        autoFocus
        value={draft}
        aria-label="Scratchpad name"
        aria-invalid={error != null}
        aria-describedby={error != null ? "scratchpad-rename-error" : undefined}
        onChange={(event) => setDraft(event.target.value)}
        onKeyDown={onKeyDown}
        onBlur={() => void commit()}
        onFocus={(event) => event.target.select()}
        className="h-7 text-[0.9375rem] leading-5 font-[550] tracking-[-0.005em]"
      />
      {/* Floated under the field so a refusal never grows the fixed-height header and shoves the
          editor down; it is transient, above the body, so it takes the one overlay shadow. */}
      {error && (
        <p
          id="scratchpad-rename-error"
          role="alert"
          className="absolute top-full right-0 left-0 z-10 mt-1 rounded-md border bg-popover px-2 py-1 text-[0.6875rem] text-destructive shadow-[var(--shadow-overlay)]"
        >
          {error}
        </p>
      )}
    </div>
  );
}
