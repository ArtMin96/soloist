use super::*;

#[test]
fn builtin_defaults_seed_the_known_providers_in_order() {
    let kinds: Vec<AgentKind> = AgentTool::builtin_defaults()
        .iter()
        .map(|tool| tool.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            AgentKind::Claude,
            AgentKind::Codex,
            AgentKind::Amp,
            AgentKind::Gemini,
            AgentKind::OpenCode,
            AgentKind::Copilot,
            AgentKind::Kimi,
        ],
        "the seeded built-ins are the known providers, in their canonical order"
    );
}

#[test]
fn every_builtin_command_is_a_bare_binary_with_no_seed_args() {
    // The command is run directly (and, for the auto-detected providers, probed with
    // `--version`), so a built-in's command must be the bare binary name with no arguments
    // baked in.
    for tool in AgentTool::builtin_defaults() {
        assert!(
            !tool.command.contains(char::is_whitespace),
            "{} command should be a bare binary, got {:?}",
            tool.name,
            tool.command
        );
        assert!(tool.default_args.is_empty(), "{} seeds no args", tool.name);
    }
}

#[test]
fn auto_detection_covers_exactly_the_documented_probe_set() {
    // Solo documents probing `--version` for these five providers.
    for kind in [
        AgentKind::Claude,
        AgentKind::Codex,
        AgentKind::Amp,
        AgentKind::Gemini,
        AgentKind::OpenCode,
    ] {
        assert!(
            kind.auto_detectable(),
            "{kind:?} is in the documented probe set"
        );
    }
    // Copilot and Kimi are built-in tool types but outside that set; Generic is
    // user-configured. None are auto-detected.
    for kind in [AgentKind::Copilot, AgentKind::Kimi, AgentKind::Generic] {
        assert!(!kind.auto_detectable(), "{kind:?} is not auto-detected");
    }
}
