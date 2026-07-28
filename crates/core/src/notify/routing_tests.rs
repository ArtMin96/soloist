//! The routing table: every combination of attention kind, presence, and notification level, with
//! the delivery it must produce. Written as a table because the rule is a product of three closed
//! inputs — an example-per-case suite would leave holes, and this is the contract every surface
//! that renders an alert reads.

use crate::attention::AttentionKind;
use crate::ids::ProcessId;
use crate::settings::NotificationLevel;

use super::{route, Delivery, Presence};

/// The process an alert is about in this table.
const SUBJECT: ProcessId = ProcessId::from_raw(1);
/// Some other process the user may be looking at instead.
const ELSEWHERE: ProcessId = ProcessId::from_raw(2);

const KINDS: [AttentionKind; 5] = [
    AttentionKind::Crashed,
    AttentionKind::RestartExhausted,
    AttentionKind::AgentPermission,
    AttentionKind::AgentError,
    AttentionKind::TerminalBell,
];

const LEVELS: [NotificationLevel; 3] = [
    NotificationLevel::All,
    NotificationLevel::Important,
    NotificationLevel::None,
];

/// The four presences that matter, named for what the user is actually doing.
fn presences() -> [(&'static str, Presence); 4] {
    [
        (
            "away from the app",
            Presence {
                focused: false,
                viewing: None,
            },
        ),
        (
            // The stale-presence case: the window was hidden while this process was selected.
            "away, with this process last selected",
            Presence {
                focused: false,
                viewing: Some(SUBJECT),
            },
        ),
        (
            "here, looking at another process",
            Presence {
                focused: true,
                viewing: Some(ELSEWHERE),
            },
        ),
        (
            "here, looking at this process",
            Presence {
                focused: true,
                viewing: Some(SUBJECT),
            },
        ),
    ]
}

/// What the table says a delivery should be, derived from the rule stated independently of the
/// implementation: the level decides whether the signal survives at all, then presence decides
/// where it goes.
fn expected(kind: AttentionKind, presence: Presence, level: NotificationLevel) -> Delivery {
    let admitted = match (level, kind) {
        (NotificationLevel::None, _) => false,
        (NotificationLevel::All, _) => true,
        (NotificationLevel::Important, AttentionKind::TerminalBell) => false,
        (NotificationLevel::Important, _) => true,
    };
    if !admitted {
        return Delivery::Suppressed;
    }
    match (presence.focused, presence.viewing) {
        // Watching the process that raised it, in a window that is actually on screen.
        (true, Some(seen)) if seen == SUBJECT => Delivery::Suppressed,
        (true, _) => Delivery::Toast,
        (false, _) => Delivery::Native,
    }
}

#[test]
fn route_is_exhaustive_over_kind_presence_and_level() {
    for kind in KINDS {
        for (where_the_user_is, presence) in presences() {
            for level in LEVELS {
                assert_eq!(
                    route(SUBJECT, kind, presence, level),
                    expected(kind, presence, level),
                    "{kind:?} at level {level:?}, user {where_the_user_is}",
                );
            }
        }
    }
}

#[test]
fn a_hidden_window_still_delivers_natively_for_the_process_it_last_showed() {
    // Presence goes stale by design: the shell pushes `focused: false` on hide without clearing
    // the selection. Suppressing here would lose the alert entirely — no toast to see, and no
    // unread mark — for the one case the user most needs it.
    let hidden = Presence {
        focused: false,
        viewing: Some(SUBJECT),
    };

    assert_eq!(
        route(
            SUBJECT,
            AttentionKind::Crashed,
            hidden,
            NotificationLevel::All
        ),
        Delivery::Native,
    );
}

#[test]
fn the_default_presence_delivers_natively() {
    // Headless callers (MCP, HTTP, tests) never push presence, so the default decides their
    // routing. A default of "focused" would silently convert every one of their alerts into a
    // toast nothing renders.
    assert_eq!(
        route(
            SUBJECT,
            AttentionKind::Crashed,
            Presence::default(),
            NotificationLevel::All
        ),
        Delivery::Native,
    );
}
