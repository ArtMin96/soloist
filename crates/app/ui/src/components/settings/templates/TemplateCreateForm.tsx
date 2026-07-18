import { useRef, useState } from "react";
import { ArrowLeft } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { LazyRichTextEditor } from "@/components/editor/LazyRichTextEditor";
import { TEMPLATE_KIND_LABEL } from "@/lib/templates";
import type { TemplateKind } from "@/domain";

interface TemplateCreateFormProps {
  kind: TemplateKind;
  /** Authors the template — the panel routes it to the core and closes the form on success. */
  onCreate: (name: string, description: string, body: string) => Promise<void>;
  /** Dismiss the form without creating. */
  onCancel: () => void;
}

// The drill-in surface for a new template of one kind: a name and optional description over the rich
// body editor, posted explicitly on Create — creation has no prior revision to guard, so unlike
// editing it is a single deliberate write, not autosave. A template's name and body are both required
// (the core refuses either blank), so Create stays disabled until both are present. A rejection (a
// taken name) stays on the form with the reason so the draft survives.
export function TemplateCreateForm({ kind, onCreate, onCancel }: TemplateCreateFormProps) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const bodyRef = useRef("");
  const [hasBody, setHasBody] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canCreate = name.trim() !== "" && hasBody && !busy;

  const create = () => {
    if (!canCreate) return;
    setBusy(true);
    setError(null);
    onCreate(name, description, bodyRef.current)
      .catch((reason) => setError(String(reason)))
      .finally(() => setBusy(false));
  };

  return (
    <div className="flex flex-col gap-3">
      <header className="flex items-center gap-2">
        <Button variant="ghost" size="sm" onClick={onCancel}>
          <ArrowLeft aria-hidden /> Templates
        </Button>
        <div className="flex-1" />
      </header>

      <div>
        <p className="text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
          New {TEMPLATE_KIND_LABEL[kind].toLowerCase()} template
        </p>
      </div>

      {error && (
        <p className="text-[0.8125rem] text-destructive" aria-live="polite">
          {error}
        </p>
      )}

      <Input
        value={name}
        onChange={(event) => setName(event.target.value)}
        placeholder="Name"
        aria-label="Template name"
        className="h-8 font-[550]"
      />
      <Input
        value={description}
        onChange={(event) => setDescription(event.target.value)}
        placeholder="Description (optional)"
        aria-label="Template description"
        className="h-8 text-[0.8125rem]"
      />

      <div className="h-[22rem]">
        <LazyRichTextEditor
          initialMarkdown=""
          ariaLabel="Template body"
          placeholder="Write the template — press / for commands"
          outline
          onChange={(markdown) => {
            bodyRef.current = markdown;
            setHasBody(markdown.trim() !== "");
          }}
          onSaveShortcut={create}
        />
      </div>

      <footer className="flex items-center justify-end gap-2">
        <Button variant="ghost" size="sm" onClick={onCancel}>
          Cancel
        </Button>
        <Button size="sm" onClick={create} disabled={!canCreate}>
          {busy ? "Creating…" : "Create template"}
        </Button>
      </footer>
    </div>
  );
}
