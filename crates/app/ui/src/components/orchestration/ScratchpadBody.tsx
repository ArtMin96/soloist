import { useRef } from "react";
import { Check, Copy } from "lucide-react";
import { Button } from "@/components/ui/button";
import { LazyRichTextEditor } from "@/components/editor/LazyRichTextEditor";
import { useAutosave } from "@/components/editor/useAutosave";

interface ScratchpadBodyProps {
  /** The Markdown to seed the editor with — this component is remounted per document, so it is read once. */
  initialBody: string;
  /** The document's name, used to shape the copied Markdown (`# name` + body). */
  name: string;
  /** Persists the Markdown body revision-guarded; the panel routes it to the core. */
  onSave: (markdown: string) => Promise<void>;
  /** True while a revision conflict is unresolved: autosave pauses until the panel reloads. */
  paused: boolean;
}

// The editable half of a scratchpad: the rich-text editor plus its autosave. It is remounted for each
// open document (a fresh React key), so its editor and undo history are per-document and a reload
// starts clean. Edits stream out of the editor as Markdown, are debounced by `useAutosave`, and flush
// immediately on blur or Cmd/Ctrl+S — never echoed back into the editor, so the caret never jumps.
export function ScratchpadBody({ initialBody, name, onSave, paused }: ScratchpadBodyProps) {
  // The editor's latest Markdown, tracked for "Copy Markdown" without making the editor controlled.
  const latestMarkdown = useRef(initialBody);
  const autosave = useAutosave({ onSave, paused });

  const copyMarkdown = () => {
    void navigator.clipboard?.writeText(`# ${name}\n\n${latestMarkdown.current}`);
  };

  const status = autosave.saving ? "Saving…" : autosave.dirty ? "Unsaved changes" : "Saved";

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 p-3">
      <LazyRichTextEditor
        initialMarkdown={initialBody}
        ariaLabel="Scratchpad body"
        outline
        onChange={(markdown) => {
          latestMarkdown.current = markdown;
          autosave.push(markdown);
        }}
        onSaveShortcut={autosave.flush}
        onBlur={autosave.flush}
      />

      <footer className="flex shrink-0 items-center gap-3">
        <span
          className="text-[0.6875rem] text-muted-foreground"
          aria-live="polite"
          data-autosave-status
        >
          {status}
        </span>
        <div className="flex-1" />
        <Button variant="ghost" size="sm" onClick={copyMarkdown}>
          <Copy aria-hidden /> Copy Markdown
        </Button>
        <Button
          size="sm"
          onClick={autosave.flush}
          disabled={autosave.saving || !autosave.dirty}
          data-scratchpad-save
        >
          {autosave.saving ? "Saving…" : <Check aria-hidden />}
          {autosave.saving ? "" : "Save"}
        </Button>
      </footer>
    </div>
  );
}
