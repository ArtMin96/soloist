# C1 — Scratchpad/Diagram row-type decision

**Status:** decided. Recorded alongside the S08-F2 fix (DSA audit slice 4).

## The question

`StoredScratchpad` and `StoredDiagram` differ only in `body` vs `source` and declare
identical `WriteResult` / `RenameResult` enums. Every layer — core aggregate, store,
in-memory fakes — re-implements the pair, and every layer can drift independently.
S15 and S16 both explicitly waited on an answer before proceeding.

## Decision

**Do not unify `Scratchpads` and `Diagrams` into one generic document aggregate.**

The duplication is real and measured: `crates/core/src/coordination/diagram_repo.rs` is
roughly 70% line-identical to `scratchpad_repo.rs` after renaming, and
`crates/store/src/diagrams.rs` vs `crates/store/src/scratchpads.rs` likewise, as do the
two in-memory fakes. The aggregates nevertheless differ in ways that are not incidental:

- **`transfer` and the derived-todo cascade** exist on scratchpads only.
- **Template seeding** applies to scratchpads only.
- **The heading-skipping `gist` rule** is scratchpad-specific.

A generic-over-kind rewrite would span core + store + testing fakes to remove
duplication that has produced exactly one defect to date, and that defect is fixable
locally. The payoff does not clear the risk.

## What the drift actually cost, and what was done instead

The one live consequence was **S08-F2**: `diagram.rs` validated its rename target and
`scratchpad.rs` did not, so `scratchpad_rename` accepted a blank/whitespace name. Fixed
by mirroring `diagram.rs`'s `validate_name` + `RenameError::Invalid` into
`scratchpad.rs` — the same rule, applied at both mutating entry points of both
aggregates.

The verifier explicitly **rejected** the `DocName` newtype that S08 proposed as the
structural remedy: a shared validated handle spanning both aggregates and both facades
is speculative abstraction for one missing call.

## Consequences for the waiting rows

- **S16-F1 (`doc_table` in the store)** is unblocked. It may proceed on its own merits.
  Prefer static per-table SQL over interpolated identifiers.
- **The S15 fakes note** is unblocked on the same basis.
- Neither should reopen aggregate unification as a prerequisite.

## Scope note

This decision covers the `Scratchpads` / `Diagrams` pair only. It says nothing about
todos or templates, which have their own validation and were outside the lane.
