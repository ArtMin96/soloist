use super::*;

#[test]
fn the_guide_teaches_binding_via_the_injected_process_id() {
    let guide = agent_guide();
    assert!(guide.contains(PROCESS_ID_ENV));
    assert!(guide.contains("bind_session_process"));
    assert!(guide.contains("register_agent"));
    assert!(guide.contains("whoami"));
}

#[test]
fn the_guide_covers_scope_trust_and_idle_timers() {
    let guide = agent_guide();
    assert!(guide.contains("select_project"));
    assert!(guide.contains("untrusted"));
    assert!(guide.contains("timer_fire_when_idle_any"));
    assert!(guide.contains("wait_for_bound_port"));
}

#[test]
fn the_guide_prescribes_coordination_etiquette() {
    let guide = agent_guide();
    assert!(guide.contains("lock_acquire"));
    assert!(guide.contains("todo_lock"));
    assert!(guide.contains("revision"));
}
