//! The Tauri desktop shell: hosts the domain core and exposes it to the UI.
//!
//! This adapter holds no business logic. It builds the [`Facade`] over the real port
//! adapters (process spawner, clock, SQLite store), registers it as managed state,
//! routes `invoke` calls to facade commands, and forwards the core's `DomainEvent`
//! stream to the webview as Tauri events. The UI renders the resulting read model.

use std::sync::Arc;

use serde::Serialize;
use soloist_core::{
    Facade, NoopRuntimeState, ProcessId, ProcessView, RuntimeState, Store, TokioClock,
};
use soloist_pty::{PgidOrphanControl, PtyProcessSpawner};
use soloist_store::{FileRuntimeState, SqliteStore};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::broadcast::error::RecvError;

/// The webview event name carrying every serialized [`soloist_core::DomainEvent`].
const DOMAIN_EVENT: &str = "domain-event";

#[derive(Serialize)]
struct AppInfo {
    name: String,
    version: String,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "Soloist".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

/// Spawns the demo process end to end and returns its id.
#[tauri::command]
async fn spawn_demo(facade: State<'_, Facade>) -> Result<u64, String> {
    Ok(facade.spawn_demo_process().get())
}

/// Requests a graceful stop of the process with the given id; reports whether it was
/// found.
#[tauri::command]
async fn stop_process(id: u64, facade: State<'_, Facade>) -> Result<bool, String> {
    Ok(facade.supervisor().stop(ProcessId::from_raw(id)))
}

/// The current process read model — the snapshot half of snapshot-then-deltas.
#[tauri::command]
async fn list_processes(facade: State<'_, Facade>) -> Result<Vec<ProcessView>, String> {
    Ok(facade.snapshot())
}

/// Builds the façade over the real adapters, degrading to an in-memory store if the
/// durable location is unavailable so the app still launches.
fn build_facade() -> Facade {
    let store = Arc::new(match SqliteStore::open_default() {
        Ok(store) => store,
        Err(err) => {
            eprintln!("soloist: durable store unavailable ({err}); using in-memory store");
            SqliteStore::open_in_memory().expect("open in-memory store")
        }
    });
    // Exercise the storage thread in the real binary: record the launching version.
    let _ = store.meta_set("last_launch_version", env!("CARGO_PKG_VERSION"));

    // Running process groups are recorded to a small file (not SQLite) so a leftover
    // from a crash can be reconciled on the next launch; degrade to a no-op if the data
    // location is unavailable so the app still launches.
    let runtime: Arc<dyn RuntimeState> = match FileRuntimeState::open_default() {
        Ok(runtime) => Arc::new(runtime),
        Err(err) => {
            eprintln!("soloist: runtime-state unavailable ({err}); orphan adoption disabled");
            Arc::new(NoopRuntimeState)
        }
    };

    // One SQLite store backs the trust and project repositories the façade needs.
    Facade::new(
        Arc::new(PtyProcessSpawner),
        Arc::new(TokioClock),
        store.clone(),
        store,
        runtime,
        Arc::new(PgidOrphanControl),
    )
}

/// Subscribes to the core event bus and forwards each event to the webview. Lagged
/// receivers are skipped (the UI re-syncs via `list_processes`); a closed bus ends
/// the task at shutdown.
fn forward_events(app: AppHandle) {
    let mut events = app.state::<Facade>().subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match events.recv().await {
                Ok(event) => {
                    let _ = app.emit(DOMAIN_EVENT, event);
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });
}

pub fn run() {
    tauri::Builder::default()
        .manage(build_facade())
        .setup(|app| {
            forward_events(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            spawn_demo,
            stop_process,
            list_processes
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                // Reap every managed process group before the app exits, so no child
                // outlives it (the deterministic-shutdown contract).
                let facade = app.state::<Facade>();
                tauri::async_runtime::block_on(facade.supervisor().shutdown());
            }
        });
}
