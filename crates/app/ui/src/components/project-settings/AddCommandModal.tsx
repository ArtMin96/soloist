import { useState } from "react";
import { Field, ToggleRow } from "@/components/project-settings/fields";
import { buildSpec } from "@/components/project-settings/spec";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { SettingChoice } from "@/components/settings/controls/SettingChoice";
import { Input } from "@/components/ui/input";
import type { Choice } from "@/components/settings/controls/SettingChoice";
import type { ProcessSpec, Visibility } from "@/domain";

// Where a new command is stored. Neither name says what it costs — the shared file is committed and
// visible to everyone, the local one never leaves this machine — so each carries a line that does.
const STORE_CHOICES: Choice<Visibility>[] = [
  {
    value: "shared",
    label: "Save to solo.yml",
    description: "Committed with the project and shared with everyone.",
  },
  {
    value: "local",
    label: "Store locally only",
    description: "Kept on this machine; never written to solo.yml.",
  },
];

// Create a command: its name, command line, working directory (empty = the project root), start /
// restart defaults, file-watch globs, and where it is stored (the shared solo.yml or this machine
// only). On success the dialog closes and the page reloads; a failure keeps it open with the reason.
export function AddCommandModal({
  open,
  onOpenChange,
  onAdd,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAdd: (name: string, spec: ProcessSpec, visibility: Visibility) => Promise<void>;
}) {
  const [name, setName] = useState("");
  const [command, setCommand] = useState("");
  const [workingDir, setWorkingDir] = useState("");
  const [autoStart, setAutoStart] = useState(true);
  const [autoRestart, setAutoRestart] = useState(false);
  const [globs, setGlobs] = useState("");
  const [visibility, setVisibility] = useState<Visibility>("shared");
  const [error, setError] = useState<string | null>(null);

  const reset = () => {
    setName("");
    setCommand("");
    setWorkingDir("");
    setAutoStart(true);
    setAutoRestart(false);
    setGlobs("");
    setVisibility("shared");
    setError(null);
  };

  const change = (next: boolean) => {
    if (!next) reset();
    onOpenChange(next);
  };

  const submit = () => {
    setError(null);
    const spec = buildSpec({
      command,
      working_dir: workingDir.trim() || null,
      auto_start: autoStart,
      auto_restart: autoRestart,
      restart_when_changed: globs.split(",").flatMap((g) => {
        const trimmed = g.trim();
        return trimmed ? [trimmed] : [];
      }),
      env: {},
    });
    onAdd(name.trim(), spec, visibility)
      .then(() => change(false))
      .catch((e) => setError(String(e)));
  };

  // Keeps the button from inviting a submit that cannot succeed. It does not enforce the "a command
  // needs a name and a command line" rule — that is the core's single source of truth, surfaced as
  // an InvalidCommand on write and shown in `error` above.
  const canSubmit = name.trim().length > 0 && command.trim().length > 0;

  return (
    <Dialog open={open} onOpenChange={change}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>Add a command</DialogTitle>
          <DialogDescription>
            A managed process Soloist runs and supervises for this project.
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-4">
          <Field label="Name">
            <Input
              value={name}
              onChange={(e) => setName(e.currentTarget.value)}
              placeholder="Web"
              aria-label="Command name"
            />
          </Field>
          <Field label="Command">
            <Input
              value={command}
              onChange={(e) => setCommand(e.currentTarget.value)}
              placeholder="npm run dev"
              aria-label="Command"
              className="font-mono text-xs"
            />
          </Field>
          <Field label="Working directory" hint="Leave empty to use the project root.">
            <Input
              value={workingDir}
              onChange={(e) => setWorkingDir(e.currentTarget.value)}
              placeholder="(project root)"
              aria-label="Working directory"
              className="font-mono text-xs"
            />
          </Field>

          <div className="flex flex-col gap-2.5">
            <ToggleRow
              label="Start when the project opens"
              checked={autoStart}
              onChange={setAutoStart}
            />
            <ToggleRow
              label="Restart automatically when it exits"
              checked={autoRestart}
              onChange={setAutoRestart}
            />
          </div>

          <Field
            label="Restart when files change"
            hint="Comma-separated globs, e.g. src/**/*.rs, .env"
          >
            <Input
              value={globs}
              onChange={(e) => setGlobs(e.currentTarget.value)}
              placeholder="src/**/*.rs"
              aria-label="File-watch globs"
              className="font-mono text-xs"
            />
          </Field>

          <fieldset className="flex flex-col">
            <legend className="mb-1.5 text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
              Where to save
            </legend>
            <SettingChoice
              value={visibility}
              choices={STORE_CHOICES}
              onChange={setVisibility}
              ariaLabel="Where to save"
            />
          </fieldset>

          {error && <p className="text-xs text-destructive">{error}</p>}
        </div>

        <DialogFooter>
          <Button variant="ghost" size="sm" onClick={() => change(false)}>
            Cancel
          </Button>
          <Button size="sm" onClick={submit} disabled={!canSubmit}>
            Add command
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
