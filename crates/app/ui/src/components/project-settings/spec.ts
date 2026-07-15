import type { ProcessSpec, ProjectCommandView } from "@/domain";

// The full ProcessSpec shape this surface persists for a command. The forms edit some fields
// (command, the toggles, the watch globs) and carry the rest untouched — `working_dir` and `env`,
// which this surface never renders — so an edit never drops a field it does not show.
export interface CommandFields {
  command: string;
  working_dir: string | null;
  auto_start: boolean;
  auto_restart: boolean;
  restart_when_changed: string[];
  env: Record<string, string>;
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
    env: fields.env,
  };
}

// The spec a command currently carries — the base an edit patches a single field of.
export function specOf(command: ProjectCommandView): ProcessSpec {
  return buildSpec(command);
}
