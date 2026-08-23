Implement Slice 4 — "one invariant, one site forgot" trio from the DSA codebase audit.

Follow the start-of-session protocol in CLAUDE.md first (PROGRESS.md, the
architecture set, the phase file).

The audit lives in Soloist MCP scratchpads. Read, in this order:
  scratchpad_read "dsa-audit-contract"   — section 6 has the priority ranking
  scratchpad_read "dsa-audit-S08"
  scratchpad_read "dsa-audit-S11"
  scratchpad_read "dsa-audit-S12"
  scratchpad_read "dsa-audit-verify-B"

Work items, in this order:
1. S08-F2
2. S11-F1
3. S12-F2

S11-F1's real defect is crates/core/src/git/commit.rs:52-60 missing the arm branch.rs:34-41 has. Also extend the port doc at git/repository.rs to full six-method membership. S11 is the authoritative owner — S21's adapter-side finding is NOT the same issue.

Rules that always apply:
- Implement at the VERIFIER-NARROWED scope, not the lane's original proposal.
  Where the verifier rejected or narrowed a sub-claim, honour that.
- Test-first. Every new or changed test must be SHOWN TO FAIL against the
  unfixed behaviour before you fix it — break the fix, watch it redden, restore.
  Assert observable outcomes, never call shape.
- Invoke the matching tauri-* skills and the testing-guidelines skill before
  touching those surfaces. Confirm any Tauri API against the official docs.
- Do not touch the locked non-changes: panic = "unwind", freezePrototype = false,
  Cargo.lock brotli pins, release opt-level, removeUnusedCommands.
- Run the full gate set ONCE at the end: just lint && just test.
- Update PROGRESS.md before finishing.

If any finding turns out to be wrong when you read the actual code, say so and
stop rather than forcing the change.
