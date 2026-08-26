import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

/** What the run's two git configuration files are called inside the directory they share. */
const CONFIG = "config";
const EXCLUDES = "ignore";

/**
 * The one path the run's global git configuration tells every repository to ignore.
 *
 * The configuration is deliberately not empty. An app that never received it and an app that
 * received an empty one behave identically, so there would be nothing to assert about which of
 * them happened; this entry is what makes the containment observable — a fixture holding this
 * path is reported ignored only because the app really read the run's configuration.
 */
export const CONFIGURED_IGNORE = "draft-note.md";

/** Where the run's global git configuration lives under `sandbox`. */
export function sandboxGitConfig(sandbox: string): string {
  return path.join(sandbox, CONFIG);
}

/**
 * Writes that configuration and the excludes file it names.
 *
 * Naming it and writing it are separate because they happen at different times: the app inherits
 * the launcher's environment, so the name has to exist at module load, while the file itself can
 * only be written once the scratch tree has been wiped.
 */
export function writeSandboxGitConfig(sandbox: string): void {
  mkdirSync(sandbox, { recursive: true });
  const excludes = path.join(sandbox, EXCLUDES);
  writeFileSync(excludes, `${CONFIGURED_IGNORE}\n`);
  writeFileSync(sandboxGitConfig(sandbox), `[core]\n\texcludesFile = ${quoted(excludes)}\n`);
}

/**
 * A path as a git configuration value, so a checkout under a folder whose name carries a comment
 * character, a quote or a backslash still reads back as itself.
 */
function quoted(value: string): string {
  return `"${value.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}
