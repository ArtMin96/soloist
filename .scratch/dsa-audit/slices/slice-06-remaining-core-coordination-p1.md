Implement Slice 6 — remaining core/coordination P1 from the DSA codebase audit.

Follow the start-of-session protocol in CLAUDE.md first (PROGRESS.md, the
architecture set, the phase file).

The audit lives in Soloist MCP scratchpads. Read, in this order:
  scratchpad_read "dsa-audit-contract"   — section 6 has the priority ranking
  scratchpad_read "dsa-audit-S07"
  scratchpad_read "dsa-audit-S09"
  scratchpad_read "dsa-audit-S12"
  scratchpad_read "dsa-audit-S13"
  scratchpad_read "dsa-audit-S14"
  scratchpad_read "dsa-audit-verify-B"
  scratchpad_read "dsa-audit-verify-C"

Work items, in this order:
1. S09-F1
2. S07-F2
3. S12-F1
4. S14-F1
5. S13-F2

All were NARROWED. S07-F2's revision guard does not close the TOCTOU — implement only the surviving claim. S13 owns the shared/local command roster (S03-F1 is superseded by it).

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
