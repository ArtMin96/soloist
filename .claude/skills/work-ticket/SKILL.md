---
name: work-ticket
description: "Work ONE ticket from a .scratch/<feature>/issues/ local-tracker backlog end to end, in a single command, so each implementation session is one step instead of many. Runs the Soloist start-of-session protocol, picks the next open+unblocked ticket (or the one you name), claims it, implements it test-first via /implement (tdd → gates → code-review → commit), then updates the ticket Status and PROGRESS.md. Use when the user says 'work the next ticket', '/work-ticket', 'do the next PRD/issue', or names a ticket number to implement. One ticket per invocation."
---

# work-ticket — one command per implementation session

Purpose: the user does not want to re-run the per-session ceremony by hand. This skill does the
whole loop for **one** ticket so a fresh session is a single command.

Default backlog: `.scratch/stability-audit-2026-07/issues/` (the stability & security audit). If
the user names a different `.scratch/<feature>/` or ticket, use that instead. Tracker conventions
are in `docs/agents/issue-tracker.md` (one file per ticket `NN-<slug>.md`; a `Status:` line and a
`Blocked by:` line near the top; frontier = lowest-numbered ticket that is open + unblocked +
unclaimed).

Do these steps in order. Do exactly one ticket, then stop.

## 1. Select the ticket
- If the user passed a ticket number or path, use that ticket.
- Otherwise scan the backlog's `issues/` and pick the **frontier**: the lowest-numbered ticket
  whose `Status:` is not `done`/`resolved`, that is **unblocked** (every id in its `Blocked by:`
  line is a ticket already `done`/`resolved`), and unclaimed. Print which ticket you chose and why.
- If nothing is unblocked, report the state (what's blocking what) and stop.

## 2. Soloist start-of-session protocol (MANDATORY — root `CLAUDE.md` §1)
Before writing any code:
- Read `PROGRESS.md` (current state) and this repo's `CLAUDE.md`.
- Read the ticket in full.
- Read the behavior + design contracts **for the area the ticket touches**: `plan/05` (Solo
  behavior), `plan/04` and `plan/06` (architecture/blueprint), and `ARCHITECTURE.md`. Invoke any
  matching `tauri-*` skill and `/impeccable` if the ticket touches Tauri config or UI (per
  `CLAUDE.md §5`). Confirm the ticket's plan still matches the code before starting.

## 3. Claim it
- Set the ticket's `Status:` line to `claimed` and save, so a parallel session won't grab it too.
- Confirm you are on the working branch `fix/stability-audit-2026-07` (the audit tickets live
  there). If the branch is missing or you're on another branch, tell the user and let them
  choose — do not silently switch or branch.

## 4. Implement it — invoke `/implement` on the ticket
Run `/implement` against the ticket file. It drives `/tdd` at the seams named in the ticket's Test
plan (a failing test first, then make it pass, slice by slice), runs typecheck + single test files
as it goes and the full suite at the end, then `/code-review` on the diff, then commits to the
current branch. While implementing, hold to the ticket's **Fix approach**, **Test plan**, and
**Acceptance**; honor the locked owner decisions recorded in the ticket and the audit README; do
**not** pull `later`-scope in; **never** weaken or skip a test to go green (`CLAUDE.md §12/§15`).

## 5. Gates (project definition of done)
Run the real gates and paste the outcome:
- `just lint` (fmt, clippy `-D warnings`, tsc, eslint, prettier, dependency-direction) — exit 0.
- `just test` (or the crate/UI subset the ticket touched, plus the full suite once).
If anything is red, **stop**: leave the ticket `Status: claimed`, report the failure with output,
and do not mark it done. Never report green you did not see.

## 6. Close out
On green:
- Set the ticket `Status:` to `done` and append a short `## Comments` note: what changed, the gate
  results, and the commit sha.
- Update `PROGRESS.md` per `CLAUDE.md §10` — a factual, evidence-backed entry (ticket id, what was
  done, tests/gates run). Commit that doc update too.
- If the ticket needs live/runtime verification this headless session can't do (e.g. a GUI walk or
  real-runtime check — currently tickets **02** and **08**), implement + unit-test it, set
  `Status: needs-human-verify` instead of `done`, and tell the user **exactly** what to check in
  the running app (`just dev`).

## 7. Report
End with: the ticket you closed (or why you stopped), gate results, the commit sha, and the **next
frontier ticket** so the user knows what the following session will pick up. Then stop — one ticket
per invocation.
