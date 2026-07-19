import { useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { TodoDocFields } from "@/components/orchestration/TodoDocFields";
import type { ScratchpadSummary, TodoDoc, TodoStatus } from "@/domain";

interface TodoCreateFormProps {
  /** Posts the new document and its association — the board routes it to the core. */
  onCreate: (doc: TodoDoc, scratchpad: number | null) => Promise<void>;
  /** The project's scratchpads, offered as the documents this todo may derive from. */
  scratchpads: ScratchpadSummary[];
  /** Dismiss the form without creating. */
  onCancel: () => void;
  /** The core's rejection (e.g. a blank title), or null. */
  error: string | null;
}

// The inline new-todo form at the top of the board (a progressive affordance, not a modal). It
// authors a whole document with the shared fields and posts it explicitly on Create — creation has
// no prior revision to guard, so unlike editing it is a single deliberate write, not autosave. The
// title is required (the core refuses a blank one); the body is optional and may be seeded from the
// default todo template server-side; the scratchpad starts at None and is the author's to opt into. On success the board closes the form; a rejection stays open
// with the reason so the draft survives.
export function TodoCreateForm({ onCreate, scratchpads, onCancel, error }: TodoCreateFormProps) {
  const [title, setTitle] = useState("");
  const [status, setStatus] = useState<TodoStatus>("open");
  const [scratchpad, setScratchpad] = useState<number | null>(null);
  const bodyRef = useRef("");
  const [busy, setBusy] = useState(false);

  const canCreate = title.trim() !== "" && !busy;

  const create = () => {
    if (!canCreate) return;
    setBusy(true);
    void onCreate({ title, body: bodyRef.current, status }, scratchpad).finally(() =>
      setBusy(false),
    );
  };

  return (
    <div className="flex flex-col gap-2 border-b bg-sidebar-accent/40 p-3">
      {error && (
        <p className="text-[0.8125rem] text-destructive" aria-live="polite">
          {error}
        </p>
      )}

      <TodoDocFields
        title={title}
        status={status}
        initialBody=""
        titleId="todo-create-title"
        scratchpads={scratchpads}
        scratchpad={scratchpad}
        onTitleChange={setTitle}
        onStatusChange={setStatus}
        onScratchpadChange={setScratchpad}
        onBodyChange={(markdown) => {
          bodyRef.current = markdown;
        }}
        onSaveShortcut={create}
      />

      <footer className="flex items-center justify-end gap-2">
        <Button variant="ghost" size="sm" onClick={onCancel}>
          Cancel
        </Button>
        <Button size="sm" onClick={create} disabled={!canCreate}>
          {busy ? "Creating…" : "Create todo"}
        </Button>
      </footer>
    </div>
  );
}
