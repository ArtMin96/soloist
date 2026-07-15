# PRD-10 — Close the real test-coverage holes (and delete the trivial tautologies)

Status: done
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

## Comments

Done 2026-07-14 (branch `fix/stability-audit-2026-07`; impl commit `571af0b`, docs/ledger commit
follows). All 11 holes filled and 14 trivial tests deleted/strengthened, each new test written to
fail against a broken version and green on `main`. `/code-review` (Standards + Spec) run on the
diff; the two acted-on findings are folded into `571af0b` (below).

**Holes → tests (real behaviour, discriminating):**
1. **HTTP trust-gate 403.** B1's start-403 already existed; added `restarting_an_untrusted_command_is_403`
   (extracted a shared `facade_with_untrusted_command` helper), the parametrized
   `every_mutation_route_requires_the_token` over all 14 mutation routes (B2), and the CLI
   `a_forbidden_mutation_reads_as_a_trust_prompt` (mutation vs read 403 mapping).
2–3. **Transfer success at the C6 aggregate** (the real gap — the facade layer was already tested):
   `todo_tests::transfer_moves_a_todo_to_the_new_scope_clearing_its_blockers_and_lock`,
   `scratchpad_tests::transfer_moves_a_scratchpad_to_the_new_scope_keeping_its_identity` (+ a
   NameTaken refusal) — re-key, clear blockers+lock, readable only from the new scope.
4. **Hotkeys** `a_populated_keymap_survives_a_serde_round_trip` — pins the `"super":true` literal and
   the disabled-`null` override (a symmetric round-trip alone can't catch a rename).
5. **Config write ceiling** `a_write_over_the_size_ceiling_is_refused_and_leaves_the_file_unchanged`.
6. **sys attribution** — `portscan::ports_are_attributed_to_the_group_that_holds_them` (two real
   groups, each holding a distinct listening socket past `exec` via cleared `FD_CLOEXEC`; exact port
   is the cross-attribution discriminator) + `metrics::two_live_groups_are_each_attributed_their_own_reading`.
7. **facade/output** three async streaming tests (default/explicit/capped counts, raw, search, ports,
   None for an unknown id).
8. **IPC frame** truncated-body → `Io`, non-JSON → `Codec`.
9. **store migration** `upgrading_a_populated_intermediate_database_preserves_its_rows` (faithful v6
   schema seeded with rows, upgraded to current).
10. **peer_cred** — strengthened the pure `peer_uid_permitted` gate (no root/off-by-one bypass) and,
    from the review, extracted the connection decision into a pure `peer_cred::peer_scope` used by
    `handle_connection`, unit-tested for Err→drop / Ok(None)→unauthenticated / Ok(Some)→scoped. Note:
    the ticket's "None → drop" phrasing is inaccurate — the code correctly opens an *unauthenticated*
    session on `None` (documented in `peer_cred.rs`); the uid-check call site and the unreadable-creds
    `Err` are OS-credential branches not forceable in a headless unit test (no behaviour change; the
    locked PRD-09 uid-drop stands).
11. **Boundary ticks** — restart-window exactly-`WINDOW` edge (still Exhausts), exact 5 s SIGKILL
    grace (needed `STOP_GRACE` → `pub(crate)`), list-form `processes:` rejected, feedback exact-MAX_LEN
    + multibyte char-vs-byte, integration-file symlink→regular replace.

**Cleanup (14):** deleted pure tautologies/duplicates — `ids::from_raw_round_trips`,
`lineage::parent_of_returns_the_recorded_parent`, `coordination/kv::list_returns_complex_json_value`
(a tautology only vs `FakeKvRepo`), `store/kv::set_and_get_round_trip_scalar`,
`project_settings`/`settings` `save_then_load_round_trips` duplicates, cli `carried_messages_render_verbatim`,
UI `timerPanel` self-compare. Strengthened the weak-but-real: `protocol::port_wait_outcomes_serialize_to_their_wire_tags`
(literal tags), the two `guide` prose smokes → registry-driven structural checks over `topics()`,
`signalStore` EMPTY_STORE → behavioural, `todo` status labels → distinct+non-empty.
**Decision:** kept `store/kv::set_and_get_round_trip_object` (findings §B lumped `:36`+`:43`; `:43`
crosses the real SQLite JSON↔TEXT boundary, so it is a genuine round-trip, not a tautology — deleting
it would drop coverage the ticket's Out-of-scope warns against).

**Review fixes folded in:** scrubbed audit tags (`B1`) from two `mutations.rs` comments (§8); added
the discriminating `peer_scope` connection-policy test; re-applied a required `cargo fmt`
normalization of pre-existing drift in `crates/app/src/commands/mod.rs`.

**Gates:** `just lint` exit 0 (fmt, clippy `-D warnings`, tsc, eslint, prettier, dep-direction;
file-size advisory only). `just test` — **Rust 968 passed / 0 failed / 3 ignored, UI 305 passed /
61 files** (net Rust +16 / UI −1 from the deletions). Fully headless-verified → `done`, not
`needs-human-verify`.
