//! The Tauri desktop shell: hosts the domain core and exposes it to the UI.
//!
//! This adapter holds no business logic. It builds the [`Facade`] over the real port
//! adapters (process spawner, clock, SQLite store), registers it as managed state,
//! routes `invoke` calls to facade commands (see [`commands`]), and forwards the core's
//! `DomainEvent` stream to the webview as Tauri events. The UI renders the read model.

mod commands;
#[cfg(feature = "mcp")]
mod ipc_server;
mod notifier;
#[cfg(feature = "mcp")]
mod peer_cred;
mod pty_bridge;

// The two dev diagnostics each install a global tracing subscriber; enabling both would make
// the second registration fail at runtime. Force the choice at compile time instead.
#[cfg(all(feature = "devtools", feature = "tokio-console"))]
compile_error!("enable either `devtools` or `tokio-console`, not both");

use std::sync::Arc;

use serde::Serialize;
use soloist_core::{
    CompositeLockReleaser, CorePorts, Facade, LeaseReleaser, NoopRuntimeState, RuntimeState, Store,
    TodoLockReleaser, TokioClock,
};
use soloist_pty::{PgidOrphanControl, PtyProcessSpawner};
use soloist_store::{FileRuntimeState, SqliteStore};
use soloist_sys::{
    CommandShellEnvProbe, CommandVersionProbe, NotifyFileWatcher, ProcMetricsProbe, ProcPortProbe,
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_window_state::StateFlags;
use tokio::sync::broadcast::error::RecvError;

use notifier::TauriNotifier;
use pty_bridge::PtyBridge;

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

/// Builds the façade over the real adapters, degrading to an in-memory store if the
/// durable location is unavailable so the app still launches. Takes the [`AppHandle`] so the
/// desktop notifier can show toasts through the Tauri notification plugin.
fn build_facade(app: AppHandle) -> Facade {
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

    // One SQLite store backs the trust, project, agent-tool, and coordination (lease + timer +
    // scratchpad + todo) repositories the façade needs. The lock releaser fans a closing process's
    // close out to both its leases and its todo locks (over the same store), and the lease, timer,
    // scratchpad, and todo stores persist them; the runtime-state and orphan-control adapters are
    // wired for adoption, the metrics probe reads CPU/memory from /proc, the port probe reads /proc,
    // the file watcher reports filesystem changes via notify, the notifier shows desktop toasts via
    // the Tauri notification plugin, the version probe auto-detects installed agent CLIs, and the
    // shell-env probe captures the login shell's environment (over this process's own env as the
    // base) so launched processes see version-manager PATHs.
    let lock_releaser = CompositeLockReleaser::new(vec![
        Arc::new(LeaseReleaser::new(store.clone())),
        Arc::new(TodoLockReleaser::new(store.clone())),
    ]);
    Facade::new(
        CorePorts::builder(
            Arc::new(PtyProcessSpawner),
            Arc::new(TokioClock),
            store.clone(),
            store.clone(),
        )
        .runtime(runtime)
        .orphan_control(Arc::new(PgidOrphanControl))
        .metrics(Arc::new(ProcMetricsProbe::new()))
        .port_probe(Arc::new(ProcPortProbe::new()))
        .file_watcher(Arc::new(NotifyFileWatcher::new()))
        .notifier(Arc::new(TauriNotifier::new(app)))
        .agent_tools(store.clone())
        .version_probe(Arc::new(CommandVersionProbe::new()))
        .shell_env_probe(Arc::new(CommandShellEnvProbe::new()))
        .app_env(std::env::vars().collect())
        .lock_repo(store.clone())
        .timer_repo(store.clone())
        .scratchpad_repo(store.clone())
        .todo_repo(store.clone())
        .kv_repo(store.clone())
        .settings_repo(store.clone())
        .project_settings_repo(store)
        .locks(Arc::new(lock_releaser))
        .build(),
    )
}

/// Subscribes to the core event bus and forwards each event to the webview. Lagged
/// receivers are skipped (the UI re-syncs via `proc_list`); a closed bus ends the task
/// at shutdown.
fn forward_events(app: AppHandle) {
    let mut events = app.state::<Arc<Facade>>().subscribe();
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
    // Install the tokio-console subscriber before anything spawns, so every supervised actor
    // and sampler is instrumented from the first task. Dev-only (`tokio-console` feature).
    #[cfg(feature = "tokio-console")]
    console_subscriber::init();

    // Build the CrabNebula DevTools plugin as early as possible so it captures startup spans.
    // Dev-only (`devtools` feature); release and default builds never link it.
    #[cfg(feature = "devtools")]
    let devtools = tauri_plugin_devtools::init();

    #[cfg_attr(
        not(any(feature = "devtools", feature = "agent-bridge")),
        allow(unused_mut)
    )]
    let mut builder = tauri::Builder::default();
    #[cfg(feature = "devtools")]
    {
        builder = builder.plugin(devtools);
    }
    // Dev-only MCP bridge: lets an AI agent inspect IPC calls and drive the webview for
    // debugging over @hypothesi/tauri-mcp-server. Compiled in only under the `agent-bridge`
    // feature; release and default builds never link it.
    #[cfg(feature = "agent-bridge")]
    {
        builder = builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        // Persist and restore the window's geometry and mode across launches. Size, position,
        // maximized, and fullscreen are tracked; decorations and visibility are deliberately not —
        // the custom titlebar keeps decorations off, and the window owns its own visibility.
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    StateFlags::SIZE
                        | StateFlags::POSITION
                        | StateFlags::MAXIMIZED
                        | StateFlags::FULLSCREEN,
                )
                .build(),
        )
        .manage(PtyBridge::default())
        .setup(|app| {
            // Build the façade here (not in the builder chain) so the desktop notifier can
            // capture the AppHandle. Hold it in an Arc so the loopback HTTP server — a
            // core-only adapter that cannot see the AppHandle — can share the one core; the
            // commands read it back from managed state.
            let facade = Arc::new(build_facade(app.handle().clone()));
            #[cfg(feature = "http")]
            let http_facade = Arc::clone(&facade);
            app.manage(facade);
            // Clear coordination leases and timers left by a previous run, and the process-owned
            // locks on durable todos: all are owned by per-run process ids that are recycled, so
            // nothing from a fresh launch holds (or could be delivered) one yet. The todos
            // themselves persist (G11) — only their stale locks are dropped.
            if let Err(err) = app.state::<Arc<Facade>>().reconcile_leases() {
                eprintln!("soloist: could not reconcile stale leases on launch ({err})");
            }
            if let Err(err) = app.state::<Arc<Facade>>().reconcile_timers() {
                eprintln!("soloist: could not reconcile stale timers on launch ({err})");
            }
            if let Err(err) = app.state::<Arc<Facade>>().reconcile_todo_locks() {
                eprintln!("soloist: could not reconcile stale todo locks on launch ({err})");
            }
            forward_events(app.handle().clone());
            // Start the self-healing reactor: it watches the core event stream and
            // relaunches crashed auto_restart commands within the documented rate limit
            // (the future holds only a weak reference and ends when the app shuts down).
            tauri::async_runtime::spawn(app.state::<Arc<Facade>>().self_healing_loop());
            // Start the metrics sampler: it samples each running process group on its
            // interval and publishes CPU/memory ticks (also weakly held, also self-supervised).
            tauri::async_runtime::spawn(app.state::<Arc<Facade>>().metrics_sampler_loop());
            // Start the port scanner: it discovers each running group's listening ports and
            // reflects them on the read model (also weakly held, also self-supervised).
            tauri::async_runtime::spawn(app.state::<Arc<Facade>>().port_scanner_loop());
            // Start the idle sampler: it reclassifies each launched agent's activity from its
            // terminal output and publishes transitions (also weakly held, also self-supervised).
            tauri::async_runtime::spawn(app.state::<Arc<Facade>>().idle_sampler_loop());
            // Start the coordination timer scheduler: it fires due timers and delivers each body to
            // its owning process as a fresh turn, tracking idle state from the event stream (also
            // weakly held, also self-supervised).
            tauri::async_runtime::spawn(app.state::<Arc<Facade>>().timer_scheduler_loop());
            // Start the notification reactor: it shows a desktop toast on a crash or an
            // exhausted auto-restart via the notification plugin (also weakly held).
            tauri::async_runtime::spawn(app.state::<Arc<Facade>>().notifications_loop());
            // Re-register previously-opened projects so they reappear in the sidebar on
            // launch (resting — restore never starts a process); the UI seeds from the
            // resulting snapshots.
            app.state::<Arc<Facade>>().restore_projects();
            // Start the file-watch reactor last: it reads the restored commands at startup, so
            // it must run after restore has registered them, then re-syncs on each project
            // open. It reloads a running watched command when a matching file changes via the
            // notify watcher wired in `build_facade` (weakly held, ends when the bus closes).
            tauri::async_runtime::spawn(app.state::<Arc<Facade>>().file_watch_loop());
            // Start the local IPC server so the soloist-mcp sidecar can drive the core over
            // a Unix socket. Compiled in only under the `mcp` feature; it degrades to a
            // logged no-op if the socket cannot be bound, never blocking app launch.
            #[cfg(feature = "mcp")]
            tauri::async_runtime::spawn(ipc_server::serve(app.handle().clone()));
            // Start the loopback HTTP API so a shell or launcher can drive the core over
            // 127.0.0.1, identically to the UI and MCP. Compiled in only under the `http`
            // feature; it degrades to a logged no-op if no loopback port can be bound,
            // never blocking app launch. The focus callback raises the desktop window for
            // `POST /focus` — the one effect the core-only server cannot perform itself.
            #[cfg(feature = "http")]
            {
                let window_handle = app.handle().clone();
                let focus: soloist_httpapi::FocusFn = Arc::new(move || {
                    if let Some(window) = window_handle.get_webview_window("main") {
                        let _ = window.set_focus();
                    }
                });
                tauri::async_runtime::spawn(soloist_httpapi::serve(http_facade, focus));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            commands::proc_list,
            commands::project_list,
            commands::project_load,
            commands::config_trust,
            commands::agent_list,
            commands::agent_detect,
            commands::agent_launch,
            commands::proc_start,
            commands::proc_stop,
            commands::proc_restart,
            commands::stack_start,
            commands::stack_stop,
            commands::stack_restart_running,
            commands::pty_write,
            commands::pty_resize,
            commands::pty_attach,
            commands::pty_detach,
            commands::orphans_resolve,
            commands::appearance,
            commands::set_appearance,
            commands::sidebar_settings,
            commands::set_sidebar_settings,
            commands::hotkeys,
            commands::remap_hotkey,
            commands::disable_hotkey,
            commands::reset_hotkey,
            commands::reset_all_hotkeys,
            commands::agent_settings,
            commands::set_agent_settings,
            commands::tool_defaults,
            commands::set_tool_defaults,
            commands::integration_settings,
            commands::set_integration_settings,
            commands::mcp_tool_groups,
            commands::set_mcp_tool_group,
            commands::project_settings_page,
            commands::project_settings,
            commands::set_project_auto_start_gate,
            commands::set_project_editor_override,
            commands::set_project_crash_exit_alerts,
            commands::set_project_terminal_alerts,
            commands::set_command_terminal_alerts,
            commands::add_shared_command,
            commands::edit_shared_command,
            commands::rename_shared_command,
            commands::remove_shared_command,
            commands::add_local_command,
            commands::edit_local_command,
            commands::rename_local_command,
            commands::remove_local_command,
            commands::make_command_local,
            commands::save_command_to_yaml,
            commands::set_project_icon,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                // Reap every managed process group before the app exits, so no child
                // outlives it (the deterministic-shutdown contract).
                let facade = app.state::<Arc<Facade>>();
                tauri::async_runtime::block_on(facade.supervisor().shutdown());
                // Drop the HTTP runtime file so a stale port does not outlive the app.
                #[cfg(feature = "http")]
                soloist_httpapi::remove_runtime();
            }
        });
}
