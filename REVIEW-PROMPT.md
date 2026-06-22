# Review prompt

Paste the block below to have a session **comprehensively research and review** a set of Soloist
changes — a phase's work, a commit range, or the current diff. It reviews, verifies, and reports; it
does **not** build features and does **not** silently change code. Edit only the `REVIEW TARGET =` line.

---

```
You are reviewing **Soloist** in /home/arthur/Downloads/soloist — a clean-room, native-Linux rebuild of
the macOS app "Solo" (soloterm.com). This is a FRESH session with no memory of previous ones; the repo's
files are the only shared brain. Your job is to **comprehensively research and review** the changes
below — find problems, verify claims, and report. Do NOT implement features and do NOT change code unless
I explicitly tell you to after seeing your findings.

REVIEW TARGET = ____________________   (e.g. "the latest commit", "Phase 0", "origin/main since the last
                                        phase", or "the current uncommitted diff". Leave blank to review
                                        the most recent commit on the current branch.)

STEP 1 — Ground yourself (CLAUDE.md §1): read PROGRESS.md (incl. "Critical details"), CLAUDE.md in full,
plan/05 (Solo behavior), plan/04 (architecture), and the relevant plan/phases/phase-NN file + its rows in
plan/02. Then pin down the exact changes under review with git (`git show`, `git diff <range>`, or
`git status`) and state precisely what you are reviewing and which files it touches.

STEP 2 — Research, don't assume (CLAUDE.md §4): for anything touching Tauri / Claude Code / MCP / a
library API, consult the official docs (tauri.app/llms.txt, code.claude.com/llms.txt) and the context7
MCP BEFORE judging it — name the source you used. Verify any claim about Solo's behavior against plan/05;
if it isn't documented there, it's a recorded gap, not a bug.

STEP 3 — Review across every dimension and collect findings:
  1. Architecture (plan/04): hexagonal boundaries; `crates/core` stays pure (no tauri/rmcp/axum/
     rusqlite/notify-rust); adapters depend on core, never the reverse; one behavior routed through one
     core command; FSMs as explicit transitions; actors not shared mutexes; bounded buffers/channels +
     backpressure; no unwrap/expect/panic in long-running tasks; deterministic resource reclamation
     (PTYs/FDs/process groups).
  2. Correctness & bugs: logic errors, edge cases, error handling, concurrency races, lifecycle/cleanup.
  3. Security (plan/04 §12): trust gate enforced in core; MCP/HTTP scope + auth; capabilities are
     least-privilege; CSP; env sanitization. No adapter/tool can reach another project's state.
  4. Performance & size (CLAUDE.md §6): bounded everything; coalesce chatty output; avoid needless
     clones/allocs in hot paths (PTY loop, event fan-out); release profile; bundle/RSS budget. Measure,
     never invent a number.
  5. Comments & cleanliness (CLAUDE.md §8): docblocks + genuinely-important comments ONLY — flag every
     phase number, plan/doc citation, changelog/placeholder note, or comment that merely restates code.
  6. Clean-room (CLAUDE.md §9): no copied Solo source/assets/branding; MCP tool names may mirror Solo but
     schemas are ours; every Solo-behavior fact traces to plan/05.
  7. Tests & gates: actually RUN `just lint` and `just test` and report the real results — never claim
     green you did not see. Check that new behavior has tests and the dependency-direction guard passes.
  8. Docs & ledger: PROGRESS.md updated with evidence; plan docs consistent and non-contradictory;
     CLAUDE.md §14 commands current; intentional Solo divergences recorded.

STEP 4 — Report. Group findings by severity — **Blocker** (must fix before merge), **Should-fix**,
**Nit** — each with `file:line`, the problem, and a concrete proposed fix. Separately list what you
VERIFIED (with the command and its output) from what you only read. End with an overall verdict:
ship / fix-then-ship / needs-rework. Propose fixes; apply them only if I say so.

If any doc contradicts another, STOP and surface it — never resolve a contradiction by guessing.
```

---

## Notes for you

- This is a **read-and-verify** prompt: it runs the gates and inspects the diff, but proposes rather than
  applies changes. Tell it to fix once you've agreed on the findings.
- **Invocable equivalent: `/soloist-review [target]`** (`.claude/skills/soloist-review/`). The slash command
  is the expanded version of this prompt — same read-and-verify contract, plus first-class checks for domain
  boundaries & separation, duplication/DRY, and security gaps, and it **consults the project's `tauri-*`
  skills** before judging any Tauri surface. Prefer it in a fresh session; keep this paste-block for when you
  want to drop the methodology inline.
- For a deep multi-agent cloud review of a branch or PR, `/code-review ultra` is the heavier alternative.
