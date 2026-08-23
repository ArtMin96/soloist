Implement Slice 2 — frontend latest-request guard (do after slice 1) from the DSA codebase audit.

Follow the start-of-session protocol in CLAUDE.md first (PROGRESS.md, the
architecture set, the phase file).

The audit lives in Soloist MCP scratchpads. Read, in this order:
  scratchpad_read "dsa-audit-contract"   — section 6 has the priority ranking
  scratchpad_read "dsa-audit-S22"
  scratchpad_read "dsa-audit-S23"
  scratchpad_read "dsa-audit-S25"
  scratchpad_read "dsa-audit-verify-E"

Work items, in this order:
1. S22-F1
2. S23-F1
3. S25-F1 commit 1 only (the Partial<ProcessSpec> signature)

One shared pattern — a monotonic latest-request generation guard, already implemented three times in-repo (useDiagramEditor.ts, useAttention.ts, useRepositoryRead.ts). Port the ~6-line guard per site. Do NOT build a generic DocumentEditor<H,D> abstraction — the verifier rejected it. S22 owns the reference impl and the only regression test. Do NOT include S27 in this slice.

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
