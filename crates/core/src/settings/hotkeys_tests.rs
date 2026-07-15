//! Unit tests for the hotkey registry — code-defined defaults (Solo `⌘`/`⌥` remapped to Ctrl/Alt),
//! override-only persistence, reset, disable, cross-scope sharing, and within-scope conflict report.

use super::*;

#[test]
fn every_action_has_a_default_and_cmd_remaps_to_ctrl() {
    // The whole closed set is covered by `default_binding` (the match is exhaustive) and a few
    // representative bindings show the Linux remap.
    let palette = HotkeyAction::OpenCommandPalette.default_binding();
    assert_eq!(palette, Binding::ctrl("K"));
    assert!(
        palette.ctrl && !palette.super_key,
        "⌘K remaps to Ctrl+K, not Super"
    );

    let next_section = HotkeyAction::NextSection.default_binding();
    assert!(next_section.alt, "⌥ (Option) remaps to Alt");
    assert_eq!(next_section.key, "ArrowDown");

    // Restart is an unmodified single key in the sidebar.
    assert_eq!(
        HotkeyAction::RestartSelection.default_binding(),
        Binding::plain("R")
    );
}

#[test]
fn terminal_search_is_a_terminal_scope_action() {
    // Ctrl+F is dispatched by the terminal-focused key handler, so it must be Terminal-scoped —
    // not General — or that handler (which filters to its own scope) never receives the chord.
    assert_eq!(
        HotkeyAction::OpenTerminalSearch.scope(),
        HotkeyScope::Terminal
    );
    assert_eq!(
        HotkeyAction::OpenTerminalSearch.default_binding(),
        Binding::ctrl("F")
    );
}

#[test]
fn a_fresh_keymap_is_all_defaults() {
    let hotkeys = Hotkeys::default();
    let view = hotkeys.view();
    assert_eq!(view.len(), HotkeyAction::ALL.len());
    assert!(view.iter().all(|row| row.is_default), "no overrides yet");
    // No within-scope collisions ship in the defaults.
    assert!(
        hotkeys.conflicts().is_empty(),
        "the shipped defaults must not conflict"
    );
}

#[test]
fn the_same_key_across_scopes_is_not_a_conflict() {
    // Previous-project (Sidebar) and previous-process (Terminal) both default to Ctrl+ArrowUp.
    let prev_project = HotkeyAction::PrevProjectGroup;
    let prev_process = HotkeyAction::PreviousProcess;
    assert_eq!(
        prev_project.default_binding(),
        prev_process.default_binding()
    );
    assert_ne!(prev_project.scope(), prev_process.scope());
    assert!(
        Hotkeys::default().conflicts().is_empty(),
        "a shared key in different scopes does not conflict"
    );
}

#[test]
fn remap_persists_only_the_override_and_reset_restores_the_default() {
    let mut hotkeys = Hotkeys::default();
    let custom = Binding::ctrl("J");
    hotkeys.remap(HotkeyAction::QuickJump, custom.clone());

    assert_eq!(hotkeys.binding(HotkeyAction::QuickJump), Some(custom));
    let row = row_for(&hotkeys, HotkeyAction::QuickJump);
    assert!(!row.is_default, "a remapped action is no longer default");

    hotkeys.reset(HotkeyAction::QuickJump);
    assert_eq!(
        hotkeys.binding(HotkeyAction::QuickJump),
        Some(HotkeyAction::QuickJump.default_binding())
    );
    assert!(row_for(&hotkeys, HotkeyAction::QuickJump).is_default);
}

#[test]
fn disable_drops_the_binding_until_reset() {
    let mut hotkeys = Hotkeys::default();
    hotkeys.disable(HotkeyAction::OpenTerminalSearch);

    assert_eq!(hotkeys.binding(HotkeyAction::OpenTerminalSearch), None);
    assert!(!row_for(&hotkeys, HotkeyAction::OpenTerminalSearch).is_default);

    hotkeys.reset(HotkeyAction::OpenTerminalSearch);
    assert_eq!(
        hotkeys.binding(HotkeyAction::OpenTerminalSearch),
        Some(HotkeyAction::OpenTerminalSearch.default_binding())
    );
}

#[test]
fn reset_all_clears_every_override() {
    let mut hotkeys = Hotkeys::default();
    hotkeys.remap(HotkeyAction::QuickJump, Binding::ctrl("J"));
    hotkeys.disable(HotkeyAction::NewAgentOrTerminal);

    hotkeys.reset_all();

    assert!(hotkeys.view().iter().all(|row| row.is_default));
}

#[test]
fn a_within_scope_collision_is_reported() {
    // Remap two General actions to the same chord — both should be flagged.
    let mut hotkeys = Hotkeys::default();
    let chord = Binding::ctrl("G");
    hotkeys.remap(HotkeyAction::OpenCommandPalette, chord.clone());
    hotkeys.remap(HotkeyAction::QuickActions, chord);

    let conflicts = hotkeys.conflicts();
    assert!(conflicts.contains(&HotkeyAction::OpenCommandPalette));
    assert!(conflicts.contains(&HotkeyAction::QuickActions));
    // An untouched action in the same scope is not implicated.
    assert!(!conflicts.contains(&HotkeyAction::QuickJump));

    // The view carries the same flag per row, so the UI never re-derives the rule.
    assert!(row_for(&hotkeys, HotkeyAction::OpenCommandPalette).conflict);
    assert!(row_for(&hotkeys, HotkeyAction::QuickActions).conflict);
    assert!(!row_for(&hotkeys, HotkeyAction::QuickJump).conflict);
}

#[test]
fn an_empty_stored_record_reads_as_the_defaults() {
    // The document is transparent over its override map, so a record an older build wrote (an empty
    // object) deserializes to no overrides — every action at its code default.
    let hotkeys: Hotkeys = serde_json::from_str("{}").expect("parse empty");
    assert!(hotkeys.view().iter().all(|row| row.is_default));
}

#[test]
fn a_populated_keymap_survives_a_serde_round_trip() {
    // A regression in this serde surface silently resets every user's keybindings on reload, so a
    // remapped binding, a disabled (`None`) entry, and the `super` wire rename must all survive
    // to_string → from_str unchanged.
    let mut hotkeys = Hotkeys::default();
    let remapped = Binding {
        ctrl: false,
        alt: false,
        shift: false,
        super_key: true,
        key: "J".to_string(),
    };
    hotkeys.remap(HotkeyAction::QuickJump, remapped.clone());
    hotkeys.disable(HotkeyAction::OpenTerminalSearch);

    let json = serde_json::to_string(&hotkeys).expect("serialize");
    // The `#[serde(rename = "super")]` is only observable in the wire form — a symmetric round-trip
    // would pass even if the rename were dropped, so pin the literal tag.
    assert!(
        json.contains("\"super\":true"),
        "the super modifier serializes under its renamed tag, got {json}"
    );
    // A disabled action persists as an explicit null override, distinct from an absent (default) one.
    assert!(
        json.contains("null"),
        "a disabled binding persists as a null override, got {json}"
    );

    let back: Hotkeys = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, hotkeys, "the whole populated keymap round-trips");
    assert_eq!(back.binding(HotkeyAction::QuickJump), Some(remapped));
    assert_eq!(
        back.binding(HotkeyAction::OpenTerminalSearch),
        None,
        "the disabled binding stays disabled after reload, not reset to its default"
    );
}

fn row_for(hotkeys: &Hotkeys, action: HotkeyAction) -> HotkeyBindingView {
    hotkeys
        .view()
        .into_iter()
        .find(|row| row.action == action)
        .expect("action is in the keymap")
}
