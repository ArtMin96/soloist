//! Longevity soak — the leak gate.
//!
//! The architecture's longevity contract requires bounded resources and deterministic
//! reclamation: a process supervisor that stays open for weeks, spawning and killing
//! processes, must return to a flat resource baseline. These tests run the real spawner and
//! clock through the [`Facade`] over real OS processes and assert exactly that after
//! sustained churn:
//!
//! - a start/stop loop of many processes leaves an identical file-descriptor, OS-thread, and
//!   tokio-task count and zero surviving process groups (no leaked PIDs);
//! - a crash → auto-restart storm stops at exactly the rate-limit gate (no hot-loop), reaps
//!   every child (no zombies), and holds RSS and the descriptor/task counts flat;
//! - the metrics sampler restarts itself after a panicking sample while the facade keeps
//!   serving commands.
//!
//! Every figure is measured from `/proc/self` and the live tokio runtime — never assumed.
//! The tests are `#[ignore]`d so the fast per-change run skips them; the nightly soak job
//! (and `just soak`) runs them with `--ignored`.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use nix::errno::Errno;
use nix::sys::signal::killpg;
use nix::unistd::Pid;
use soloist_core::testing::{
    terminal_registration, FakeMetricsProbe, FakeProjectRepo, FakeTrustRepo,
};
use soloist_core::{
    CorePorts, DomainEvent, Facade, ProcStatus, ProcessId, ProcessSpec, ProjectId, Registration,
    TokioClock, TrustRepo,
};
use soloist_pty::PtyProcessSpawner;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio::time::{sleep, timeout};

/// The project every fixture process is registered under; the soak never opens a real one.
const PROJECT: ProjectId = ProjectId::from_raw(1);

/// Headroom over the RSS baseline for allocator slack, fragmentation, and the per-process
/// scrollback the supervisor keeps for attach replay. A genuine per-cycle leak over the
/// churn below would grow well past this; transient slack stays under it.
const RSS_TOLERANCE_KIB: usize = 8 * 1024;

// ---------------------------------------------------------------------------------------
// Resource probes — read straight from `/proc/self` and the tokio runtime, never fabricated.
// ---------------------------------------------------------------------------------------

/// Open file descriptors held by this process: PTY masters, pipes, sockets, and the
/// runtime's epoll/eventfds. A descriptor leaked per iteration shows up here as growth.
fn open_fds() -> usize {
    count_dir("/proc/self/fd")
}

/// OS threads in this process: the runtime workers plus the spawner's per-running-process
/// blocking I/O threads. Returns to baseline once every child is reaped and its PTY closed.
fn os_threads() -> usize {
    count_dir("/proc/self/task")
}

fn count_dir(path: &str) -> usize {
    std::fs::read_dir(path)
        .map(|dir| dir.flatten().count())
        .unwrap_or(0)
}

/// Resident set size in KiB, from `/proc/self/status` `VmRSS`. Zero if it cannot be read.
fn rss_kib() -> usize {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|kib| kib.parse().ok())
        .unwrap_or(0)
}

/// Tasks alive on the current tokio runtime — the per-process actor tasks plus the long-lived
/// reactor/sampler loops. Returns to the loop-only baseline once each actor finishes.
fn alive_tasks() -> usize {
    tokio::runtime::Handle::current()
        .metrics()
        .num_alive_tasks()
}

/// The PIDs of this process's direct children, optionally only those left un-reaped (zombie,
/// state `Z`). A reaping leak surfaces as a lingering child; a zombie surfaces as state `Z`.
fn child_pids(only_zombies: bool) -> Vec<u32> {
    let me = std::process::id() as i32;
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut children = Vec::new();
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        // `pid (comm) state ppid ...`; comm may contain spaces and parens, so parse the
        // fixed fields from after the final ')'.
        let Some(after_comm) = stat.rfind(')').map(|i| &stat[i + 1..]) else {
            continue;
        };
        let mut fields = after_comm.split_whitespace();
        let state = fields.next().unwrap_or("");
        let ppid: i32 = fields.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        if ppid == me && (!only_zombies || state == "Z") {
            children.push(pid);
        }
    }
    children
}

// ---------------------------------------------------------------------------------------
// Settling helpers — turn "the resource returns to baseline shortly after the last reap"
// into a bounded wait. A real leak never settles, so the caller's assert still trips.
// ---------------------------------------------------------------------------------------

/// Polls `probe` until it no longer exceeds `ceiling`, or the bound elapses; returns the
/// final reading either way so the caller asserts on it.
async fn settle_to(ceiling: usize, probe: impl Fn() -> usize) -> usize {
    for _ in 0..300 {
        let value = probe();
        if value <= ceiling {
            return value;
        }
        sleep(Duration::from_millis(20)).await;
    }
    probe()
}

/// Waits until this process has no live children, or the bound elapses.
async fn await_no_children() {
    for _ in 0..300 {
        if child_pids(false).is_empty() {
            return;
        }
        sleep(Duration::from_millis(20)).await;
    }
}

/// Polls until process group `pgid` is fully gone — every member reaped, including any
/// descendant reparented to and reaped by init asynchronously — or a short timeout elapses.
async fn await_group_gone(pgid: Pid) -> bool {
    for _ in 0..200 {
        if killpg(pgid, None).err() == Some(Errno::ESRCH) {
            return true;
        }
        sleep(Duration::from_millis(20)).await;
    }
    killpg(pgid, None).err() == Some(Errno::ESRCH)
}

// ---------------------------------------------------------------------------------------
// Composition + drivers.
// ---------------------------------------------------------------------------------------

/// A facade over the real process spawner and clock, with the in-memory trust/project repos
/// (returned so a test can trust a command). The optional metrics probe overrides the noop.
fn facade(metrics: Option<Arc<FakeMetricsProbe>>) -> (Facade, Arc<FakeTrustRepo>) {
    let trust = Arc::new(FakeTrustRepo::new());
    let mut ports = CorePorts::builder(
        Arc::new(PtyProcessSpawner),
        Arc::new(TokioClock),
        trust.clone(),
        Arc::new(FakeProjectRepo::new()),
    );
    if let Some(probe) = metrics {
        ports = ports.metrics(probe);
    }
    (Facade::new(ports.build()), trust)
}

/// A trusted, auto-restarting command that crashes on every launch — the crash-storm fixture.
fn crasher_spec() -> ProcessSpec {
    ProcessSpec {
        command: "exit 1".into(),
        working_dir: None,
        auto_start: false,
        auto_restart: true,
        restart_when_changed: Vec::new(),
        env: BTreeMap::new(),
    }
}

async fn wait_for_status(events: &mut Receiver<DomainEvent>, id: ProcessId, target: ProcStatus) {
    let reached = timeout(Duration::from_secs(10), async {
        loop {
            match events.recv().await {
                Ok(DomainEvent::ProcessStatusChanged { id: got, to, .. })
                    if got == id && to == target =>
                {
                    return true
                }
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return false,
            }
        }
    })
    .await
    .unwrap_or(false);
    assert!(reached, "expected process {id:?} to reach {target:?}");
}

/// Polls the read model until the running process has recorded its group leader pgid.
async fn wait_for_pgid(facade: &Facade, id: ProcessId) -> Pid {
    for _ in 0..200 {
        if let Some(pgid) = facade.supervisor().pgid_of(id) {
            return Pid::from_raw(pgid);
        }
        sleep(Duration::from_millis(10)).await;
    }
    panic!("process {id:?} never recorded a process group");
}

/// Registers, starts, and gracefully stops one terminal, returning the group it ran in so the
/// caller can assert it was reaped.
async fn start_then_stop(facade: &Facade, events: &mut Receiver<DomainEvent>) -> Pid {
    let id = facade
        .supervisor()
        .register(terminal_registration(PROJECT, "soak", "sleep 60"));
    facade.supervisor().start(id).expect("start terminal");
    wait_for_status(events, id, ProcStatus::Running).await;
    let pgid = wait_for_pgid(facade, id).await;
    assert!(
        facade.supervisor().stop(id),
        "stop finds the running process"
    );
    wait_for_status(events, id, ProcStatus::Stopped).await;
    pgid
}

/// Drives one crash storm: a fresh trusted auto-restart crasher is started and relaunched
/// until the gate holds it exhausted. Returns how many times it was auto-restarted.
async fn run_crash_storm(facade: &Facade, trust: &FakeTrustRepo, spec: &ProcessSpec) -> u32 {
    let mut events = facade.subscribe();
    let cwd = std::env::current_dir().expect("cwd");
    let id = facade
        .supervisor()
        .register(Registration::command(PROJECT, &cwd, "Crasher", spec));
    trust
        .set_trusted(PROJECT, &spec.variant_hash())
        .expect("trust the crasher");
    facade
        .supervisor()
        .start(id)
        .expect("start trusted crasher");

    let mut restarts = 0u32;
    let exhausted = timeout(Duration::from_secs(30), async {
        loop {
            match events.recv().await {
                Ok(DomainEvent::RestartScheduled { id: got, .. }) if got == id => restarts += 1,
                Ok(DomainEvent::RestartExhausted { id: got }) if got == id => return true,
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return false,
            }
        }
    })
    .await
    .unwrap_or(false);
    assert!(exhausted, "the crasher reached RestartExhausted");
    await_no_children().await;
    restarts
}

// ---------------------------------------------------------------------------------------
// The soak tests.
// ---------------------------------------------------------------------------------------

/// A start/stop loop of many real processes must end at the resource baseline it started at:
/// no leaked descriptors, OS threads, tokio tasks, or process groups.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "longevity soak — run via the nightly soak job or `just soak`"]
async fn start_stop_loop_leaves_a_flat_resource_baseline() {
    let (facade, _trust) = facade(None);
    let mut events = facade.subscribe();

    // One warmup cycle, so any one-time lazy allocation (the IO driver, the blocking pool)
    // has happened before the baseline is taken.
    let warmup = start_then_stop(&facade, &mut events).await;
    assert!(
        await_group_gone(warmup).await,
        "warmup group must be reaped"
    );

    let base_fds = open_fds();
    let base_threads = os_threads();
    let base_tasks = alive_tasks();

    const ITERATIONS: usize = 40;
    let mut groups = Vec::with_capacity(ITERATIONS);
    for _ in 0..ITERATIONS {
        groups.push(start_then_stop(&facade, &mut events).await);
    }

    // Every group the loop started must be fully reaped — the no-leaked-PIDs guarantee.
    for pgid in groups {
        assert!(
            await_group_gone(pgid).await,
            "a process group survived the start/stop loop"
        );
    }

    let fds = settle_to(base_fds, open_fds).await;
    let threads = settle_to(base_threads, os_threads).await;
    let tasks = settle_to(base_tasks, alive_tasks).await;
    eprintln!(
        "start/stop x{ITERATIONS}: fds {base_fds}->{fds}, threads {base_threads}->{threads}, tasks {base_tasks}->{tasks}"
    );

    assert!(child_pids(false).is_empty(), "no child process may survive");
    assert!(fds <= base_fds, "file-descriptor leak: {base_fds} -> {fds}");
    assert!(
        threads <= base_threads,
        "OS-thread leak: {base_threads} -> {threads}"
    );
    assert!(
        tasks <= base_tasks,
        "tokio-task leak: {base_tasks} -> {tasks}"
    );
}

/// A command that crashes immediately and repeatedly stops at exactly the 10/60s gate (no
/// hot-loop), and many such storms leak no memory, descriptors, tasks, or zombies.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "longevity soak — run via the nightly soak job or `just soak`"]
async fn crash_restart_storm_honours_the_gate_and_leaks_nothing() {
    let (facade, trust) = facade(None);
    tokio::spawn(facade.self_healing_loop());
    let spec = crasher_spec();

    // Warmup storm before the baseline, as above.
    let _ = run_crash_storm(&facade, &trust, &spec).await;

    let base_fds = open_fds();
    let base_threads = os_threads();
    let base_tasks = alive_tasks();
    let base_rss = rss_kib();

    const STORMS: usize = 5;
    for _ in 0..STORMS {
        let restarts = run_crash_storm(&facade, &trust, &spec).await;
        assert_eq!(
            restarts, 10,
            "the storm stops at exactly the rate-limit gate, no hot-loop"
        );
    }

    let fds = settle_to(base_fds, open_fds).await;
    let threads = settle_to(base_threads, os_threads).await;
    let tasks = settle_to(base_tasks, alive_tasks).await;
    let rss = settle_to(base_rss + RSS_TOLERANCE_KIB, rss_kib).await;
    eprintln!(
        "crash storm x{STORMS}: fds {base_fds}->{fds}, threads {base_threads}->{threads}, tasks {base_tasks}->{tasks}, rss {base_rss}->{rss} KiB"
    );

    assert!(
        child_pids(true).is_empty(),
        "a crashed child was left un-reaped (zombie)"
    );
    assert!(child_pids(false).is_empty(), "a child survived the storm");
    assert!(fds <= base_fds, "file-descriptor leak: {base_fds} -> {fds}");
    assert!(
        threads <= base_threads,
        "OS-thread leak: {base_threads} -> {threads}"
    );
    assert!(
        tasks <= base_tasks,
        "tokio-task leak: {base_tasks} -> {tasks}"
    );
    assert!(
        rss <= base_rss + RSS_TOLERANCE_KIB,
        "RSS leak: {base_rss} -> {rss} KiB"
    );
}

/// The metrics sampler's sampling loop is panic-isolated and self-restarting: a sample that
/// panics is contained and the loop resumes, while the rest of the app keeps serving commands.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "longevity soak — run via the nightly soak job or `just soak`"]
async fn metrics_sampler_restarts_itself_after_a_panicking_sample() {
    let probe = FakeMetricsProbe::returning(5.0, 4096).panic_once();
    let (facade, _trust) = facade(Some(Arc::new(probe.clone())));
    let mut events = facade.subscribe();
    tokio::spawn(facade.metrics_sampler_loop());

    // A live group gives the sampler something to read.
    let id = facade
        .supervisor()
        .register(terminal_registration(PROJECT, "soak", "sleep 60"));
    facade.supervisor().start(id).expect("start terminal");
    wait_for_status(&mut events, id, ProcStatus::Running).await;

    // The first sample panics; a tick still arrives — the only way it can — once the loop has
    // been isolated and restarted.
    let ticked = timeout(Duration::from_secs(20), async {
        loop {
            match events.recv().await {
                Ok(DomainEvent::MetricsTick { id: got, .. }) if got == id => return true,
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return false,
            }
        }
    })
    .await
    .unwrap_or(false);
    assert!(
        ticked,
        "a metrics tick arrived after the sampler self-restarted"
    );
    assert!(
        probe.calls() >= 2,
        "the probe panicked once, then sampled again"
    );

    // The app is unaffected: a command still flows through the facade.
    assert!(
        facade.supervisor().stop(id),
        "the facade still serves commands after the sampler fault"
    );
    wait_for_status(&mut events, id, ProcStatus::Stopped).await;
}
