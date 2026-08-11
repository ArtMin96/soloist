import { useRef, useState, type DragEvent } from "react";
import { FileJson, Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/textarea";
import type { ThemeFile } from "@/domain";
import type { ThemeImportConflictPolicy } from "@/store/appearanceContext";
import { ThemeImportConflictError } from "@/theme/io";
import { cn } from "@/lib/utils";

export function ThemeImportDialog({
  open,
  onOpenChange,
  onImport,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onImport: (json: string, policy?: ThemeImportConflictPolicy) => Promise<ThemeFile>;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [json, setJson] = useState("");
  const [dragging, setDragging] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [conflict, setConflict] = useState<ThemeImportConflictError | null>(null);
  const immutableConflict = conflict?.existing.source === "built_in";

  const close = () => {
    setJson("");
    setError(null);
    setConflict(null);
    setDragging(false);
    onOpenChange(false);
  };

  const readFile = async (file: File | undefined) => {
    if (!file) return;
    setJson(await file.text());
    setError(null);
    setConflict(null);
  };

  const importTheme = async (policy: ThemeImportConflictPolicy = "error") => {
    setBusy(true);
    setError(null);
    try {
      await onImport(json, policy);
      close();
    } catch (caught) {
      if (caught instanceof ThemeImportConflictError) setConflict(caught);
      else setError(caught instanceof Error ? caught.message : "The theme could not be imported.");
    } finally {
      setBusy(false);
    }
  };

  const drop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    setDragging(false);
    void readFile(event.dataTransfer.files[0]);
  };

  return (
    <Dialog open={open} onOpenChange={(next) => (next ? onOpenChange(true) : close())}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>Import theme</DialogTitle>
          <DialogDescription>
            Add a Soloist or T3-compatible version 1 theme from a JSON file or pasted text.
          </DialogDescription>
        </DialogHeader>

        {conflict ? (
          <div className="flex flex-col gap-3">
            <div className="rounded-lg border border-border bg-muted/50 p-3">
              <div className="text-sm font-medium">A theme with this ID already exists</div>
              <p className="mt-1 text-xs text-muted-foreground">
                “{conflict.incoming.name}” conflicts with “{conflict.existing.name}”. Import a
                separate copy with a new ID
                {!immutableConflict && " or update the existing custom theme"}.
              </p>
            </div>
            <DialogFooter>
              <Button variant="ghost" onClick={() => setConflict(null)} disabled={busy}>
                Back
              </Button>
              <Button
                variant="secondary"
                onClick={() => void importTheme("keep_both")}
                disabled={busy}
              >
                Keep Both
              </Button>
              {!immutableConflict && (
                <Button onClick={() => void importTheme("replace")} disabled={busy}>
                  Update Existing
                </Button>
              )}
            </DialogFooter>
          </div>
        ) : (
          <>
            <div
              className={cn(
                "flex min-h-20 items-center gap-3 rounded-lg border border-dashed border-border bg-muted/30 p-3 transition-colors motion-reduce:transition-none",
                dragging && "border-ring bg-accent",
              )}
              onDragEnter={(event) => {
                event.preventDefault();
                setDragging(true);
              }}
              onDragOver={(event) => event.preventDefault()}
              onDragLeave={() => setDragging(false)}
              onDrop={drop}
            >
              <FileJson className="size-5 text-muted-foreground" aria-hidden />
              <div className="min-w-0 flex-1">
                <div className="text-sm font-medium">Theme file</div>
                <div className="text-xs text-muted-foreground">Drop a .json file here.</div>
              </div>
              <input
                ref={inputRef}
                type="file"
                accept="application/json,.json"
                className="sr-only"
                onChange={(event) => void readFile(event.target.files?.[0])}
                aria-label="Choose theme file"
              />
              <Button variant="outline" size="sm" onClick={() => inputRef.current?.click()}>
                <Upload data-icon="inline-start" /> Choose file
              </Button>
            </div>

            <label className="flex flex-col gap-2 text-sm font-medium">
              Theme JSON
              <Textarea
                value={json}
                onChange={(event) => {
                  setJson(event.target.value);
                  setError(null);
                }}
                placeholder={
                  '{\n  "version": 1,\n  "name": "Aurora",\n  "appearance": "dark",\n  "colors": {}\n}'
                }
                className="min-h-64 resize-y font-mono text-xs"
                aria-invalid={error !== null}
              />
            </label>
            {error && (
              <p role="alert" className="text-xs text-destructive">
                {error}
              </p>
            )}
            <DialogFooter>
              <Button variant="ghost" onClick={close} disabled={busy}>
                Cancel
              </Button>
              <Button onClick={() => void importTheme()} disabled={!json.trim() || busy}>
                {busy ? "Importing…" : "Import theme"}
              </Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
