//! Unit tests for [`NotificationLevel`]: which severities each level admits, and that combining a
//! project level with a per-command one always yields the tighter of the two, in either order.

use super::*;

#[test]
fn level_admits_by_severity() {
    assert!(NotificationLevel::All.admits(Severity::Important));
    assert!(
        NotificationLevel::All.admits(Severity::Terminal),
        "`All` is the only level that admits terminal-class alerts"
    );

    assert!(NotificationLevel::Important.admits(Severity::Important));
    assert!(
        !NotificationLevel::Important.admits(Severity::Terminal),
        "`Important` keeps crashes and agent attention but drops bells"
    );

    assert!(!NotificationLevel::None.admits(Severity::Important));
    assert!(!NotificationLevel::None.admits(Severity::Terminal));
}

#[test]
fn most_restrictive_picks_the_tighter_level() {
    use NotificationLevel::{All, Important, None};

    let pairs = [
        (All, All, All),
        (All, Important, Important),
        (All, None, None),
        (Important, Important, Important),
        (Important, None, None),
        (None, None, None),
    ];

    for (a, b, tighter) in pairs {
        assert_eq!(a.most_restrictive(b), tighter, "{a:?} with {b:?}");
        assert_eq!(
            b.most_restrictive(a),
            tighter,
            "combining is commutative, so which side holds the override cannot change the result"
        );
    }
}
