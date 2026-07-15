import { cpSync, mkdirSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const dir = path.dirname(fileURLToPath(import.meta.url));
const fixtures = path.resolve(dir, "../../fixtures/projects");
const scratch = path.resolve(dir, "../../.tmp/projects");

/**
 * Copies a fixture project into a scratch directory and returns its absolute path.
 *
 * Specs name a fixture and never a path. The copy matters: opening a project writes to it (a
 * `solo.yml` is created when absent, and app state is written alongside), so pointing the app at
 * the checked-in fixture would dirty the working tree and let one run's leftovers decide the next
 * run's result. Each call starts from a clean copy.
 */
export function materializeProject(name: string): string {
  const target = path.join(scratch, name);
  rmSync(target, { recursive: true, force: true });
  mkdirSync(path.dirname(target), { recursive: true });
  cpSync(path.join(fixtures, name), target, { recursive: true });
  return target;
}
