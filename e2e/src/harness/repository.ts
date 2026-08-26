import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

/** What the branch a fixture repository is created on is called. */
const BRANCH = "main";

// A fixed identity, carried per invocation because there is no configuration left to carry one:
// the run replaces the developer's global and system git configuration with its own, so nothing
// they configured decides whether the fixture commits at all or under whose name.
const IDENTITY = [
  "-c",
  "user.name=Soloist e2e",
  "-c",
  "user.email=e2e@example.invalid",
];

// Where a fixture repository's stand-in credential helper lives, and where it records having been
// asked. Both inside `.git`, so neither is a change to the working tree that a status read reports.
const HELPER = ".git/credential-helper-stub";
const HELPER_CONSULTED = ".git/credential-helper-consulted";

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
  isolateCredentials(root);
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

/**
 * Points the fixture at a credential helper of its own and discards every helper a wider
 * configuration named.
 *
 * This binds the **app** as well as the harness: the adapter runs `git` inside this repository, so
 * its repo-local configuration is part of what those invocations read. The empty value first is
 * what does the discarding — version control appends helpers across configuration files and an
 * empty one resets the list — so the stub is the only helper left, and it records having been asked
 * and then fails rather than answering. A real helper is a program with access to the developer's
 * own credential store, and one of them opens a window and waits.
 */
function isolateCredentials(root: string): void {
  const stub = path.join(root, HELPER);
  const consulted = path.basename(HELPER_CONSULTED);
  writeFileSync(
    stub,
    `#!/bin/sh\nprintf 'asked\\n' >> "$(dirname "$0")/${consulted}"\nexit 1\n`,
    { mode: 0o755 },
  );
  git(root, "config", "credential.helper", "");
  git(root, "config", "--add", "credential.helper", stub);
}

/** Runs one git command in the fixture, reporting what git said rather than only that it failed. */
function git(root: string, ...args: string[]): void {
  try {
    // The run's own environment, so these invocations read the same sandboxed git configuration
    // the app under test does rather than a curated one of the harness's.
    execFileSync("git", ["-C", root, ...args], { env: process.env, stdio: "pipe" });
  } catch (reason) {
    const said =
      (reason as { stderr?: Buffer }).stderr?.toString().trim() ??
      String(reason);
    throw new Error(
      `preparing the fixture repository failed: git ${args.join(" ")} — ${said}`,
    );
  }
}
