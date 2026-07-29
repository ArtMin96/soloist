//! [`PresenceCell`] behaviour: it reports the newest observation, and reports being away until
//! the shell says otherwise.

use super::*;

#[test]
fn a_cell_reports_being_away_until_the_shell_reports_otherwise() {
    // Every headless caller reads this default, so it decides their routing.
    assert_eq!(
        PresenceCell::new().get(),
        Presence {
            focused: false,
            viewing: None,
        },
    );
}

#[test]
fn the_newest_observation_wins() {
    let cell = PresenceCell::new();
    let web = ProcessId::from_raw(1);
    let api = ProcessId::from_raw(2);

    cell.set(Presence {
        focused: true,
        viewing: Some(web),
    });
    cell.set(Presence {
        focused: true,
        viewing: Some(api),
    });

    assert_eq!(cell.get().viewing, Some(api));
}

#[test]
fn a_repeated_observation_reports_no_change() {
    let cell = PresenceCell::new();
    let here = Presence {
        focused: true,
        viewing: Some(ProcessId::from_raw(1)),
    };

    assert!(cell.set(here), "arriving moved the user");
    assert!(!cell.set(here), "saying it again moved nobody");
}
