# PRD-10 — Close the real test-coverage holes (and delete the trivial tautologies)

Status: ready-for-agent
Blocked by: 06

- **Severity:** P2 (the suite is healthy — ~98.7% of 1040 tests are real — but has specific holes
  over behavior that, if it regressed, would ship silently)
- **Area:** tests across `core`, `httpapi`, `cli`, `store`, `ipc`, `config`, UI
- **Evidence:** derived from the four test-honesty sub-audits. **Framing for the owner:** the belief
  "most tests are pretend" did **not** hold — 0 over-mocked tests, only 12 trivial tautologies +
  2 prose-substring smokes across the whole workspace. The value here is **adding** the missing
  tests, not deleting fake ones.

## The coverage holes to fill (ranked; each is real behavior with no/weak test)
1. **HTTP trust-gate (B1) — highest:** POST `start`/`restart` on an **untrusted command** returns
   **403** over HTTP; the CLI surfaces "that command is not trusted". Core enforcement IS tested
   (`supervisor.rs:692`); the **adapter** path is not. (Overlaps PRD-06's test.)
2. **HTTP 401 per route (B2):** the 8 mutation routes without a direct 401 test get one (or a
   single parametrized test over all 14), so a route accidentally moved to the open read router is
   caught.
3. **`Todos::transfer` / `Scratchpads::transfer` success path:** a cross-project move re-keys the
   doc, clears its blockers + lock, and the moved doc is readable only from the new scope. Only the
   `ForeignProject` refusal is currently tested.
4. **Populated hotkey-keymap serde round-trip:** a remapped binding + a disabled (`None`) entry +
   the `#[serde(rename="super")]` survive `to_string`→`from_str`. A regression here **silently
   resets every user's keybindings on reload** — user-visible, untested.
5. **Config write-side 1 MB ceiling:** a `write()` exceeding `MAX_CONFIG_BYTES` returns
   `ConfigError::TooLarge` and leaves `solo.yml` byte-unchanged (`config/sync.rs:192`).
6. **Multi-process port/metric attribution:** two concurrent live groups — ports and metrics
   attributed to the correct pgid; one process's heartbeat doesn't suppress another's sample.
7. **`facade/output` public reads under scope:** `process_output` (default/explicit/cap counts),
   `search_output`, `process_raw_output`, `process_ports`, and `None`/refusal for unknown or
   out-of-scope ids.
8. **IPC frame error paths (B4):** truncated body after a valid prefix (`FrameError::Io`) and
   garbage/non-JSON payload (`FrameError::Codec`).
9. **Populated-DB migration upgrade (B3):** upgrade a populated intermediate-version DB to current,
   preserving rows — so the first `ALTER TABLE` migration lands with a harness.
10. **peer_cred fail-closed (B5):** the `None` (unresolvable-peer) and unreadable-creds paths drop
    the connection.
11. Boundary ticks: restart-window strict-`<60s` edge; exact 5 s SIGKILL grace; config rejects a
    LIST-form `processes:`; feedback exact-`MAX_LEN` + char-vs-byte; integration-file atomic
    replace / symlink→regular.

## Cleanup (trivial — do alongside)
Delete or strengthen the 12 tautological tests + 2 prose smokes named in the findings log §B:
UI (`lib/todo.test.ts:5`, `store/timerPanel.test.ts:54`, `store/signalStore.test.ts:57`);
cli (`client_tests.rs:20`); store/ipc (`kv_tests.rs:36`/`:43`, `project_settings_tests.rs:44`,
`settings_tests.rs:93`, `protocol_tests.rs:359`); core (`ids.rs:118`,
`agents/lineage_tests.rs:7`, `coordination/kv_tests.rs:85`, `support/guide_tests.rs:22`/`:43`).
Where a tautology duplicates a real neighbor, just delete it; where it names a real invariant
weakly, strengthen it (e.g. make the guide tests assert a structural property, not a prose word).

## Approach
Each item is a small, independent test addition — these can be spread across sessions or done as
one "test-hardening" pass. Prefer behavior tests using the existing faithful fakes (`core::testing`)
and `MockClock`. Do **not** weaken any existing test to make a change pass (CLAUDE.md §15).

## Acceptance
- Every hole above has a test that fails against a deliberately-broken version and passes on
  `main`. The 14 trivial tests are removed or strengthened. `just test` + `just lint` green.

## Out of scope
Rewriting healthy tests. Coverage for `later`-scope features.
