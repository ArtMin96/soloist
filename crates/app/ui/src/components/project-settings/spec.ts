import type { ProcessSpec, ProjectCommandView } from "@/domain";

// The editable shape of a command on this surface: the ProcessSpec fields the editor and the add
// modal collect. `env` is absent — this surface neither shows nor edits a command's environment.
export interface CommandFields {
  command: string;
  working_dir: string | null;
  auto_start: boolean;
  auto_restart: boolean;
  restart_when_changed: string[];
}

// Build the ProcessSpec a command add or edit persists from the collected fields. One place both
// the editor and the add modal construct a spec, so the wire shape never drifts between them.
export function buildSpec(fields: CommandFields): ProcessSpec {
  return {
    command: fields.command.trim(),
    working_dir: fields.working_dir,
    auto_start: fields.auto_start,
    auto_restart: fields.auto_restart,
    restart_when_changed: fields.restart_when_changed,
  };
}

// The spec a command currently carries — the base an edit patches a single field of.
export function specOf(command: ProjectCommandView): ProcessSpec {
  return buildSpec(command);
}
