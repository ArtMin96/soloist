import { useState } from "react";
import { Plus, Trash2, X } from "lucide-react";
import { CommandField, ToggleRow } from "@/components/project-settings/fields";
import { SettingChoice } from "@/components/settings/controls/SettingChoice";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  commandLevelChoices,
  commandLevelFromValue,
  commandLevelValue,
  levelLabel,
} from "@/lib/notifications";
import type { CommandOps } from "@/components/project-settings/commands";
import type { NotificationLevel, ProjectCommandView } from "@/domain";

// The expanded editing form for one command: its command line, name, start / restart toggles, its
// notification level, file-watch globs, where it is stored, and delete. Text fields commit on blur or Enter;
// toggles persist on change. Each edit sends only the field it changed as a patch — the pane owns
// merging it onto the command's current spec — and the pane reloads the page after every mutation.
export function CommandEditor({
  command,
  projectLevel,
  ops,
}: {
  command: ProjectCommandView;
  projectLevel: NotificationLevel;
  ops: CommandOps;
}) {
  const [newGlob, setNewGlob] = useState("");

  // The core resolved this command against its project and can only have quietened it. Saying so
  // where they differ is the difference between the rule looking deliberate and looking broken;
  // the comparison is of two values the core handed down, never a re-decision of which one wins.
  const heldDown =
    command.notification_level !== null &&
    command.notification_level !== command.effective_notification_level;

  const commitCommand = (value: string) => {
    const next = value.trim();
    if (next && next !== command.command) ops.edit(command, { command: next });
  };
  const commitRename = (value: string) => {
    const next = value.trim();
    if (next && next !== command.name) ops.rename(command, next);
  };
  const addGlob = () => {
    const glob = newGlob.trim();
    if (!glob || command.restart_when_changed.includes(glob)) return;
    ops.edit(command, { restart_when_changed: [...command.restart_when_changed, glob] });
    setNewGlob("");
  };
  const removeGlob = (glob: string) =>
    ops.edit(command, {
      restart_when_changed: command.restart_when_changed.filter((g) => g !== glob),
    });

  return (
    <div className="flex flex-col gap-4 border-t border-border bg-muted/30 px-3 py-3.5">
      <CommandField label="Command">
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
      </CommandField>

      <CommandField label="Name">
        <Input
          key={command.name}
          defaultValue={command.name}
          onBlur={(e) => commitRename(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
          }}
          aria-label="Name"
        />
      </CommandField>

      <div className="flex flex-col gap-2.5">
        <ToggleRow
          label="Start when the project opens"
          checked={command.auto_start}
          onChange={(v) => ops.edit(command, { auto_start: v })}
        />
        <ToggleRow
          label="Restart automatically when it exits"
          checked={command.auto_restart}
          onChange={(v) => ops.edit(command, { auto_restart: v })}
        />
      </div>

      <fieldset className="flex flex-col">
        <legend className="mb-1.5 text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
          Notify me about
        </legend>
        <SettingChoice
          value={commandLevelValue(command.notification_level)}
          choices={commandLevelChoices(projectLevel)}
          onChange={(value) => ops.setNotificationLevel(command, commandLevelFromValue(value))}
          ariaLabel="Notify me about"
        />
        {heldDown && (
          <p className="mt-1.5 text-xs text-muted-foreground">
            The project setting holds this command to{" "}
            {levelLabel(command.effective_notification_level)}.
          </p>
        )}
      </fieldset>

      <CommandField label="Restart when files change">
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
      </CommandField>

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
