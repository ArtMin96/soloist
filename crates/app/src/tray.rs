//! The system-tray presence: a status icon whose menu shows the window, toggles
//! launch-on-login, runs a manual update check, and quits. Every action is a
//! window-shell or plugin call — the tray holds no domain logic. Quit goes through the
//! normal exit path (`app.exit`), so the deterministic shutdown still reaps every
//! managed process group before the app closes.

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::UpdaterExt;

use crate::open_project;

const TRAY_ID: &str = "soloist-tray";
const ITEM_SHOW: &str = "show";
const ITEM_AUTOSTART: &str = "autostart";
const ITEM_UPDATES: &str = "check-updates";
const ITEM_QUIT: &str = "quit";

/// Installs the tray icon and its menu. Skips silently when the app has no window icon to
/// display (a tray with no icon is useless), so a missing icon never blocks launch.
pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let Some(icon) = app.default_window_icon().cloned() else {
        eprintln!("soloist: no window icon available; skipping the system tray");
        return Ok(());
    };

    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
    let show = MenuItem::with_id(app, ITEM_SHOW, "Show Soloist", true, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(
        app,
        ITEM_AUTOSTART,
        "Start on login",
        true,
        autostart_on,
        None::<&str>,
    )?;
    let updates = MenuItem::with_id(app, ITEM_UPDATES, "Check for Updates…", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, ITEM_QUIT, "Quit Soloist", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &autostart, &updates, &separator, &quit])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Soloist")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            ITEM_SHOW => open_project::reveal(app),
            ITEM_AUTOSTART => toggle_autostart(app),
            ITEM_UPDATES => check_for_updates(app),
            ITEM_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                open_project::reveal(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Flips launch-on-login through the autostart plugin to match the checkbox the user just
/// toggled. Opt-in: it stays disabled until the user enables it.
fn toggle_autostart(app: &AppHandle) {
    let manager = app.autolaunch();
    let outcome = if manager.is_enabled().unwrap_or(false) {
        manager.disable()
    } else {
        manager.enable()
    };
    if let Err(err) = outcome {
        eprintln!("soloist: could not change launch-on-login ({err})");
    }
}

/// Runs a manual update check — the updater never checks on its own (disabled by
/// default). On a found update it reports it, then downloads, installs, and restarts;
/// otherwise it reports the outcome as a desktop notification (the same channel as the
/// crash/restart toasts).
fn check_for_updates(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let updater = match app.updater() {
            Ok(updater) => updater,
            Err(err) => return notify(&app, "Update check unavailable", &err.to_string()),
        };
        match updater.check().await {
            Ok(Some(update)) => {
                notify(
                    &app,
                    "Updating Soloist",
                    &format!(
                        "Downloading version {} — the app will restart when it is ready.",
                        update.version
                    ),
                );
                match update.download_and_install(|_, _| {}, || {}).await {
                    Ok(()) => app.restart(),
                    Err(err) => notify(&app, "Update failed", &err.to_string()),
                }
            }
            Ok(None) => notify(&app, "Soloist", "You're running the latest version."),
            Err(err) => notify(&app, "Update check failed", &err.to_string()),
        }
    });
}

/// Shows a desktop notification.
fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}
