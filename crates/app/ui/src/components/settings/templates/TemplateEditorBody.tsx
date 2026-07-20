import { useRef, useState } from "react";
import { Check, Copy } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { LazyRichTextEditor } from "@/components/editor/LazyRichTextEditor";
import { useAutosave } from "@/components/editor/useAutosave";

interface TemplateEditorBodyProps {
  /** The Markdown body to seed the editor with — read once (this component is remounted per template). */
  initialBody: string;
  /** The one-line description the template loaded with — read once, then controlled locally. */
  initialDescription: string;
  /** Persists the description + body revision-guarded; the panel routes it to the core. */
  onSave: (description: string, body: string) => Promise<void>;
  /** True while a revision conflict is unresolved: autosave pauses until the panel reloads. */
  paused: boolean;
}

// The editable half of a template: a one-line description field over the rich-text body, sharing the
// one autosave. Remounted per open template (a fresh React key), so its editor and undo history are
// per-template and a reload starts clean. Both fields feed one debounced save of the pair — editing
// either schedules it — flushing immediately on blur or Cmd/Ctrl+S; saved content is never echoed
// back into the editor, so the caret never jumps.
export function TemplateEditorBody({
  initialBody,
  initialDescription,
  onSave,
  paused,
}: TemplateEditorBodyProps) {
  const [description, setDescription] = useState(initialDescription);
  // The latest of each field, so the single save always writes the current pair regardless of which
  // field triggered it.
  const descriptionRef = useRef(initialDescription);
  const bodyRef = useRef(initialBody);
  const autosave = useAutosave({ onSave: (body) => onSave(descriptionRef.current, body), paused });

  const changeDescription = (value: string) => {
    setDescription(value);
    descriptionRef.current = value;
    autosave.push(bodyRef.current);
  };

  // The body verbatim — the template's source, markers and all. A heading naming the template used to
  // be prepended here, which corrupted every paste: a prompt pasted to an agent gained a title it
  // never declared, and a seedable template's copy gained an H1 that is not part of the document it
  // seeds. The filled-in prompt is a different artifact and has its own copy on the preview.
  const copyMarkdown = () => {
    void navigator.clipboard?.writeText(bodyRef.current);
  };

  const status = autosave.saving ? "Saving…" : autosave.dirty ? "Unsaved changes" : "Saved";

  return (
    <div className="flex flex-col gap-2">
      <Input
        value={description}
        onChange={(event) => changeDescription(event.target.value)}
        onBlur={autosave.flush}
        placeholder="Description (optional)"
        aria-label="Template description"
        className="h-8 text-[0.8125rem]"
      />

      <div className="h-[22rem]">
        <LazyRichTextEditor
          initialMarkdown={initialBody}
          ariaLabel="Template body"
          placeholder="Write the template — press / for commands"
          outline
          onChange={(markdown) => {
            bodyRef.current = markdown;
            autosave.push(markdown);
          }}
          onSaveShortcut={autosave.flush}
          onBlur={autosave.flush}
        />
      </div>

      <footer className="flex items-center gap-3">
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
          data-template-save
        >
          {autosave.saving ? "Saving…" : <Check aria-hidden />}
          {autosave.saving ? "" : "Save"}
        </Button>
      </footer>
    </div>
  );
}
