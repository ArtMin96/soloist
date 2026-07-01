use clap::{CommandFactory, Parser};

use super::*;

#[test]
fn the_command_definition_is_structurally_valid() {
    // clap's own checks: conflicting args, bad defaults, duplicate names, etc.
    Cli::command().debug_assert();
}

#[test]
fn status_parses_an_optional_status_filter() {
    let cli = Cli::try_parse_from(["soloist", "status", "--status", "running"]).expect("parse");
    assert!(matches!(
        cli.command,
        Command::Status {
            status: Some(StatusFilter::Running)
        }
    ));

    let cli = Cli::try_parse_from(["soloist", "status"]).expect("parse");
    assert!(matches!(cli.command, Command::Status { status: None }));

    // An unknown status value is a parse error rather than a silent miss.
    assert!(Cli::try_parse_from(["soloist", "status", "--status", "bogus"]).is_err());
}

#[test]
fn control_verbs_take_a_target_and_an_optional_project() {
    let cli = Cli::try_parse_from(["soloist", "restart", "web"]).expect("parse");
    let Command::Restart(target) = cli.command else {
        panic!("expected restart");
    };
    assert_eq!(target.target, "web");
    assert_eq!(target.project, None);

    let cli =
        Cli::try_parse_from(["soloist", "start", "all", "--project", "storefront"]).expect("parse");
    let Command::Start(target) = cli.command else {
        panic!("expected start");
    };
    assert_eq!(target.target, "all");
    assert_eq!(target.project.as_deref(), Some("storefront"));
}

#[test]
fn logs_parses_an_optional_line_count() {
    let cli = Cli::try_parse_from(["soloist", "logs", "web", "-n", "5"]).expect("parse");
    let Command::Logs { name, lines } = cli.command else {
        panic!("expected logs");
    };
    assert_eq!(name, "web");
    assert_eq!(lines, Some(5));
}

#[test]
fn focus_and_open_both_parse_to_a_raise_command() {
    // `open` is Solo's raise-app alias of `focus`; both are argument-free and route to the
    // same `raise` handler (`POST /focus`).
    assert!(matches!(
        Cli::try_parse_from(["soloist", "focus"])
            .expect("parse")
            .command,
        Command::Focus
    ));
    assert!(matches!(
        Cli::try_parse_from(["soloist", "open"])
            .expect("parse")
            .command,
        Command::Open
    ));
}

#[test]
fn a_missing_subcommand_is_an_error() {
    assert!(Cli::try_parse_from(["soloist"]).is_err());
}
