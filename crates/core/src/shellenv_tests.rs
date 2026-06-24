//! Unit tests for the shell-environment resolver: precedence, caching, and the
//! capture-failure fallback — all on a mock [`Clock`] and a fake probe, no real shell.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::{ShellEnv, CACHE_TTL};
use crate::testing::{FakeShellEnvProbe, MockClock};

/// Builds an environment map from `(key, value)` pairs.
fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[tokio::test]
async fn process_env_overrides_captured_which_carries_through() {
    // The captured shell environment is the base of the override layer; a per-process
    // `env` value wins over the captured one for a shared key, while a captured-only key
    // and a process-only key both survive. (The app environment, the spawner's inherited
    // base, is intentionally not part of the override layer.)
    let probe = Arc::new(FakeShellEnvProbe::returning(env(&[
        ("SHARED", "captured"),
        ("CAP_ONLY", "c"),
    ])));
    let shell = ShellEnv::new(probe, Arc::new(MockClock::new()), BTreeMap::new());

    let resolved = shell
        .resolve(&env(&[("SHARED", "process"), ("PROC_ONLY", "p")]))
        .await;

    assert_eq!(resolved.get("SHARED"), Some(&"process".to_string()));
    assert_eq!(resolved.get("CAP_ONLY"), Some(&"c".to_string()));
    assert_eq!(resolved.get("PROC_ONLY"), Some(&"p".to_string()));
}

#[tokio::test]
async fn capture_is_reused_within_the_ttl_and_refreshed_after_it() {
    let probe = Arc::new(FakeShellEnvProbe::returning(env(&[("PATH", "/captured")])));
    let clock = MockClock::new();
    let shell = ShellEnv::new(probe.clone(), Arc::new(clock.clone()), BTreeMap::new());

    shell.resolve(&BTreeMap::new()).await;
    shell.resolve(&BTreeMap::new()).await;
    // Both spawns within the window share the one capture.
    assert_eq!(probe.calls(), 1);

    // Past the TTL the next spawn recaptures.
    clock.advance(CACHE_TTL + std::time::Duration::from_secs(1));
    shell.resolve(&BTreeMap::new()).await;
    assert_eq!(probe.calls(), 2);
}

#[tokio::test]
async fn failed_capture_falls_back_to_app_path_with_common_dirs_prepended() {
    // With no captured environment, the fallback prepends the common user bin directories
    // (the `~` expanded against the app `HOME`) ahead of the app's own `PATH`.
    let probe = Arc::new(FakeShellEnvProbe::failing());
    let app_env = env(&[("HOME", "/home/dev"), ("PATH", "/usr/bin:/bin")]);
    let shell = ShellEnv::new(probe, Arc::new(MockClock::new()), app_env);

    let resolved = shell.resolve(&BTreeMap::new()).await;

    assert_eq!(
        resolved.get("PATH"),
        Some(&"/home/dev/.local/bin:/usr/local/bin:/usr/bin:/bin".to_string())
    );
}

#[tokio::test]
async fn process_path_still_wins_over_the_fallback() {
    let probe = Arc::new(FakeShellEnvProbe::failing());
    let app_env = env(&[("HOME", "/home/dev"), ("PATH", "/usr/bin")]);
    let shell = ShellEnv::new(probe, Arc::new(MockClock::new()), app_env);

    let resolved = shell.resolve(&env(&[("PATH", "/only/this")])).await;

    assert_eq!(resolved.get("PATH"), Some(&"/only/this".to_string()));
}

#[tokio::test]
async fn fallback_drops_a_tilde_dir_when_home_is_unknown() {
    // No `HOME` in the app environment: the `~/.local/bin` entry is dropped rather than
    // emitted literally, leaving the absolute fallback dir and the app's `PATH`.
    let probe = Arc::new(FakeShellEnvProbe::failing());
    let shell = ShellEnv::new(
        probe,
        Arc::new(MockClock::new()),
        env(&[("PATH", "/usr/bin")]),
    );

    let resolved = shell.resolve(&BTreeMap::new()).await;

    assert_eq!(
        resolved.get("PATH"),
        Some(&"/usr/local/bin:/usr/bin".to_string())
    );
}

#[tokio::test]
async fn a_failed_capture_is_not_cached_so_the_next_spawn_retries() {
    let probe = Arc::new(FakeShellEnvProbe::failing());
    let shell = ShellEnv::new(probe.clone(), Arc::new(MockClock::new()), BTreeMap::new());

    shell.resolve(&BTreeMap::new()).await;
    shell.resolve(&BTreeMap::new()).await;

    assert_eq!(probe.calls(), 2);
}
