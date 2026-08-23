Implement Slice 10 — config rows (UNVERIFIED) from the DSA codebase audit.

Follow the start-of-session protocol in CLAUDE.md first (PROGRESS.md, the
architecture set, the phase file).

The audit lives in Soloist MCP scratchpads. Read, in this order:
  scratchpad_read "dsa-audit-contract"   — section 6 has the priority ranking
  scratchpad_read "dsa-audit-S32"
  scratchpad_read "dsa-audit-S33"
  scratchpad_read "dsa-audit-S34"
  scratchpad_read "dsa-audit-S35"
  scratchpad_read "dsa-audit-S36"

Work items, in this order:
1. S36-F1
2. S35-F1
3. S35-F2
4. S32-F1
5. S33-F1
6. S34-F1

These had NO verifier pass. Verify each against the actual code before implementing, and drop any that doesn't hold. S34-F1's fix line is in S18's file — do it after slice 1, and note its Tauri version-fallback assumption is unproven.

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
