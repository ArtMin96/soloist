import { useState } from "react";
import { Plus, Trash2, X } from "lucide-react";
import { Field, ToggleRow } from "@/components/project-settings/fields";
import { specOf } from "@/components/project-settings/spec";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { CommandOps } from "@/components/project-settings/commands";
import type { ProcessSpec, ProjectCommandView } from "@/domain";

// The expanded editing form for one command: its command line, name, start / restart / alert
// toggles, file-watch globs, where it is stored, and delete. Text fields commit on blur or Enter;
// toggles persist on change. Each edit rebuilds the spec from the command's current fields so only
// the changed field moves; the pane reloads the page after every mutation.
export function CommandEditor({ command, ops }: { command: ProjectCommandView; ops: CommandOps }) {
  const [newGlob, setNewGlob] = useState("");

  const editField = (patch: Partial<ProcessSpec>) =>
    ops.edit(command, { ...specOf(command), ...patch });

  const commitCommand = (value: string) => {
    const next = value.trim();
    if (next && next !== command.command) editField({ command: next });
  };
  const commitRename = (value: string) => {
    const next = value.trim();
    if (next && next !== command.name) ops.rename(command, next);
  };
  const addGlob = () => {
    const glob = newGlob.trim();
    if (!glob || command.restart_when_changed.includes(glob)) return;
    editField({ restart_when_changed: [...command.restart_when_changed, glob] });
    setNewGlob("");
  };
  const removeGlob = (glob: string) =>
    editField({ restart_when_changed: command.restart_when_changed.filter((g) => g !== glob) });

  return (
    <div className="flex flex-col gap-4 border-t border-border bg-muted/30 px-3 py-3.5">
      <Field label="Command">
        <Input
          key={command.command}
          defaultValue={command.command}
          onBlur={(e) => commitCommand(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
          }}
          aria-label="Command"
          className="font-mono text-xs"
        />
      </Field>

      <Field label="Name">
        <Input
          key={command.name}
          defaultValue={command.name}
          onBlur={(e) => commitRename(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
          }}
          aria-label="Name"
        />
      </Field>

      <div className="flex flex-col gap-2.5">
        <ToggleRow
          label="Start when the project opens"
          checked={command.auto_start}
          onChange={(v) => editField({ auto_start: v })}
        />
        <ToggleRow
          label="Restart automatically when it exits"
          checked={command.auto_restart}
          onChange={(v) => editField({ auto_restart: v })}
        />
        <ToggleRow
          label="Terminal alerts"
          checked={command.terminal_alerts}
          onChange={(v) => ops.setTerminalAlerts(command, v)}
        />
      </div>

      <Field label="Restart when files change">
        <div className="flex flex-col gap-1.5">
          {command.restart_when_changed.length > 0 && (
            <ul className="flex flex-col gap-1">
              {command.restart_when_changed.map((glob) => (
                <li key={glob} className="flex items-center gap-2">
                  <code className="min-w-0 flex-1 truncate rounded border border-border bg-background px-2 py-1 font-mono text-xs">
                    {glob}
                  </code>
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    aria-label={`Remove ${glob}`}
                    onClick={() => removeGlob(glob)}
                  >
                    <X />
                  </Button>
                </li>
              ))}
            </ul>
          )}
          <div className="flex items-center gap-2">
            <Input
              value={newGlob}
              onChange={(e) => setNewGlob(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  addGlob();
                }
              }}
              placeholder="src/**/*.rs"
              aria-label="Add a file-watch glob"
              className="font-mono text-xs"
            />
            <Button variant="outline" size="sm" onClick={addGlob} disabled={!newGlob.trim()}>
              <Plus />
              Add
            </Button>
          </div>
        </div>
      </Field>

      <div className="flex items-center justify-between gap-3 border-t border-border pt-3">
        <Button variant="outline" size="sm" onClick={() => ops.toggleStorage(command)}>
          {command.visibility === "shared" ? "Make local" : "Save to solo.yml"}
        </Button>
        <Button variant="destructive" size="sm" onClick={() => ops.remove(command)}>
          <Trash2 />
          Delete
        </Button>
      </div>
    </div>
  );
}
