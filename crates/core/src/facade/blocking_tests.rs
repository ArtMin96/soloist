use std::sync::{Arc, Barrier};
use std::time::Duration;

use tokio::time::timeout;

use crate::composition::CorePorts;
use crate::facade::Facade;
use crate::ports::TokioClock;
use crate::testing::{FakeProjectRepo, FakeSpawner, FakeTrustRepo};

/// A façade over in-memory fakes — enough state to call through.
fn fake_facade() -> Arc<Facade> {
    Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    ))
}

/// Store-touching callers must run their façade call **off** the runtime worker, so a durable
/// write's `fsync` never parks a worker thread. This proves it without timing: on the
/// single-threaded test runtime, launch several `blocking` ops that each wait on a shared barrier.
/// If the ops ran inline on the runtime thread, the first would block it forever and the barrier
/// could never be reached — deadlock. They pass the barrier only because each runs on the blocking
/// pool, off the runtime thread.
#[tokio::test]
async fn blocking_runs_facade_ops_off_the_runtime_worker() {
    const OPS: usize = 4;
    let facade = fake_facade();
    let barrier = Arc::new(Barrier::new(OPS));
    let mut handles = Vec::with_capacity(OPS);
    for _ in 0..OPS {
        let facade = Arc::clone(&facade);
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            facade
                .blocking(move |_facade| {
                    barrier.wait();
                })
                .await;
        }));
    }
    for handle in handles {
        timeout(Duration::from_secs(5), handle)
            .await
            .expect("no deadlock: the blocking ops ran concurrently off the single worker")
            .expect("the op task did not panic");
    }
}

/// A panic inside the op must reach the caller rather than be swallowed or turned into a value the
/// caller cannot distinguish from a real result — the façade's callers report errors as values, so
/// a panic here is a bug that must stay loud.
#[tokio::test]
async fn blocking_resumes_an_ops_panic_in_the_caller() {
    let facade = fake_facade();

    let joined = tokio::spawn(async move {
        facade
            .blocking(|_facade| {
                panic!("the op panicked");
            })
            .await;
    })
    .await;

    let err = joined.expect_err("the op's panic must surface in the calling task");
    assert!(err.is_panic(), "the panic is resumed, not swallowed");
}
