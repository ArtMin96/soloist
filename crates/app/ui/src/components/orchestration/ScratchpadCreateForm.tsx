import { useRef, useState } from "react";
import { LazyRichTextEditor } from "@/components/editor/LazyRichTextEditor";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { SaveOutcome } from "@/store/saveOutcome";

/** The name field's accessible name and its placeholder — the same words, said once. */
const NAME_FIELD_LABEL = "New scratchpad name";
/** The draft body's accessible name, distinct from the open document's so the two never collide. */
const BODY_FIELD_LABEL = "New scratchpad body";

interface ScratchpadCreateFormProps {
  /** Posts the new document — the board routes it to the core. */
  onCreate: (name: string, body: string) => Promise<SaveOutcome>;
  /** Dismiss the form without creating. */
  onCancel: () => void;
  /** The core's refusal (a taken or invalid name), or null. */
  error: string | null;
}

/**
 * The inline new-scratchpad form at the top of the board (a progressive affordance, not a modal). It
 * posts explicitly on Create: a document that does not exist yet has no revision to guard, so unlike
 * editing this is a single deliberate write rather than autosave. A name already taken is the core's
 * refusal to make, not this form's to guess, so a rejection leaves the draft exactly as it was with
 * the reason above it.
 */
export function ScratchpadCreateForm({ onCreate, onCancel, error }: ScratchpadCreateFormProps) {
  const [name, setName] = useState("");
  // The body is uncontrolled — the editor owns its own text and reports it, so a keystroke never
  // round-trips through this component and moves the caret.
  const bodyRef = useRef("");
  const [busy, setBusy] = useState(false);

  const canCreate = name.trim() !== "" && !busy;

  const create = () => {
    if (!canCreate) return;
    setBusy(true);
    void onCreate(name, bodyRef.current).finally(() => setBusy(false));
  };

  return (
    <div className="flex flex-col gap-2 border-b bg-sidebar-accent/40 p-3">
      {error && (
        <p className="type-body text-destructive" aria-live="polite">
          {error}
        </p>
      )}

      <Input
        value={name}
        aria-label={NAME_FIELD_LABEL}
        placeholder={NAME_FIELD_LABEL}
        onChange={(event) => setName(event.target.value)}
      />

      <LazyRichTextEditor
        initialMarkdown=""
        ariaLabel={BODY_FIELD_LABEL}
        outline
        onChange={(markdown) => {
          bodyRef.current = markdown;
        }}
        onSaveShortcut={create}
      />

      <footer className="flex items-center justify-end gap-2">
        <Button variant="ghost" size="sm" onClick={onCancel}>
          Cancel
        </Button>
        <Button size="sm" onClick={create} disabled={!canCreate}>
          {busy ? "Creating…" : "Create scratchpad"}
        </Button>
      </footer>
    </div>
  );
}
