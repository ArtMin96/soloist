//! The Tauri desktop shell: hosts the domain core and exposes it to the UI.

use serde::Serialize;

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

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
