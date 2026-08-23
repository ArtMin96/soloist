Implement Slice 9 — structural P2 (no live defect) from the DSA codebase audit.

Follow the start-of-session protocol in CLAUDE.md first (PROGRESS.md, the
architecture set, the phase file).

The audit lives in Soloist MCP scratchpads. Read, in this order:
  scratchpad_read "dsa-audit-contract"   — section 6 has the priority ranking
  scratchpad_read "dsa-audit-S10"
  scratchpad_read "dsa-audit-S16"
  scratchpad_read "dsa-audit-S26"
  scratchpad_read "dsa-audit-S27"
  scratchpad_read "dsa-audit-verify-B"
  scratchpad_read "dsa-audit-verify-C"
  scratchpad_read "dsa-audit-verify-F"

Work items, in this order:
1. S27-F2 (4 clone pairs, not the editor pair)
2. S16-F1
3. S10-F1(a)(b)
4. S26-F1

S16-F1 is blocked on S08's row-type decision — settle that first. S27-F2 is NOT blocked on core/store; no shared artifact exists, so keep the UI cut presentational. S10-F1's proposed enum loses a live state — implement only the narrowed claim.

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
