Implement Slice 8 — test isolation from the DSA codebase audit.

Follow the start-of-session protocol in CLAUDE.md first (PROGRESS.md, the
architecture set, the phase file).

The audit lives in Soloist MCP scratchpads. Read, in this order:
  scratchpad_read "dsa-audit-contract"   — section 6 has the priority ranking
  scratchpad_read "dsa-audit-S30"
  scratchpad_read "dsa-audit-S37"
  scratchpad_read "dsa-audit-verify-G"

Work items, in this order:
1. S30-F1 (narrowed scope), with S37-F1 as corroboration

These are two derivations of the same remedy from opposite sides of the boundary — one fix, not two. The lanes disagree on whether the credential-prompt escape is still live; resolve that empirically before coding. Verifier G narrowed S30-F1 and rejected the tagged union entirely. S37-F1 is unverified.

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
