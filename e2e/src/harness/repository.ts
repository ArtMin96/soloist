import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

/** What the branch a fixture repository is created on is called. */
const BRANCH = "main";

// A fixed identity, with the developer's own git configuration out of the way: a global
// `commit.gpgsign`, `core.hooksPath` or commit template would otherwise decide whether the
// fixture commits at all, and the run's result would depend on whose machine it ran on.
const CONFIG = {
  ...process.env,
  GIT_CONFIG_GLOBAL: "/dev/null",
  GIT_CONFIG_SYSTEM: "/dev/null",
};
const IDENTITY = [
  "-c",
  "user.name=Soloist e2e",
  "-c",
  "user.email=e2e@example.invalid",
];

/** A path the repository holds twice: as the last commit left it, and as it stands now. */
export interface RepositoryChange {
  /** Where the path sits inside the project. */
  path: string;
  /** What the last commit holds. */
  committed: string;
  /** What the working tree holds — the change a reader opens. */
  working: string;
}

/**
 * Turns a materialized fixture project into a git repository carrying exactly one uncommitted
 * change, and returns the project root.
 *
 * Everything the fixture ships is committed alongside the change's original content, so the
 * working tree differs from the commit in one path only and a spec naming that path is naming
 * the one thing the rail can show. The diff a spec then reads is whatever version control
 * itself produces — both sides of it exist only because they were really committed and really
 * edited, which no repaint can invent.
 *
 * Called before the app loads the project: the repository surfaces read their first status when
 * a project opens, so a repository that appeared afterwards would be one the window had already
 * answered for.
 */
export function makeRepository(root: string, change: RepositoryChange): string {
  const file = path.join(root, change.path);
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, change.committed);

  git(root, "init", "--quiet", "--initial-branch", BRANCH);
  git(root, "add", "--all");
  git(root, ...IDENTITY, "commit", "--quiet", "--message", "Baseline");

  writeFileSync(file, change.working);
  return root;
}

/** Adds files that remain outside the fixture repository's index. */
export function addUntrackedFiles(root: string, files: Record<string, string>): void {
  for (const [relativePath, contents] of Object.entries(files)) {
    const file = path.join(root, relativePath);
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, contents);
  }
}

/** Runs one git command in the fixture, reporting what git said rather than only that it failed. */
function git(root: string, ...args: string[]): void {
  try {
    execFileSync("git", ["-C", root, ...args], { env: CONFIG, stdio: "pipe" });
  } catch (reason) {
    const said =
      (reason as { stderr?: Buffer }).stderr?.toString().trim() ??
      String(reason);
    throw new Error(
      `preparing the fixture repository failed: git ${args.join(" ")} — ${said}`,
    );
  }
}
