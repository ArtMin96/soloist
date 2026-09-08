import { useRef, useState } from "react";
import { CreatePane } from "@/components/common/CreatePane";
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
 * The new-scratchpad form fills the board's detail pane and posts explicitly on Create. A document
 * that does not exist yet has no revision to guard, so unlike
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
    <CreatePane subject="scratchpad" destination="scratchpads" error={error} onBack={onCancel}>
      <form
        className="flex min-h-0 flex-1 flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault();
          create();
        }}
      >
        <div className="flex flex-col gap-1.5">
          <label htmlFor="scratchpad-create-name" className="type-label text-muted-foreground">
            Name
          </label>
          <Input
            id="scratchpad-create-name"
            name="name"
            value={name}
            aria-label={NAME_FIELD_LABEL}
            placeholder="release-plan"
            onChange={(event) => setName(event.target.value)}
          />
        </div>

        <LazyRichTextEditor
          initialMarkdown=""
          ariaLabel={BODY_FIELD_LABEL}
          outline
          onChange={(markdown) => {
            bodyRef.current = markdown;
          }}
          onSaveShortcut={create}
        />

        <footer className="flex items-center justify-end gap-2 border-t pt-3">
          <Button type="button" variant="ghost" size="sm" onClick={onCancel}>
            Cancel
          </Button>
          <Button type="submit" size="sm" disabled={!canCreate}>
            {busy ? "Creating…" : "Create scratchpad"}
          </Button>
        </footer>
      </form>
    </CreatePane>
  );
}
