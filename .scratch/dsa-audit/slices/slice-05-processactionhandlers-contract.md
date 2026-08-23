Implement Slice 5 — ProcessActionHandlers contract from the DSA codebase audit.

Follow the start-of-session protocol in CLAUDE.md first (PROGRESS.md, the
architecture set, the phase file).

The audit lives in Soloist MCP scratchpads. Read, in this order:
  scratchpad_read "dsa-audit-contract"   — section 6 has the priority ranking
  scratchpad_read "dsa-audit-S26"
  scratchpad_read "dsa-audit-S28"
  scratchpad_read "dsa-audit-verify-F"

Work items, in this order:
1. S28-F1 + S26-F2 + App.tsx as one commit
2. S28-F2

S28-F1 is the authoritative owner; S26-F2 is a same-commit dependent slice. App.tsx is owned by neither lane. Net-test ProcessNode first. For S28-F2, write the context-menu test first — the duplicated half is currently unverified by any test. Drop the unprofiled "13 closures per row" perf claim.

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
