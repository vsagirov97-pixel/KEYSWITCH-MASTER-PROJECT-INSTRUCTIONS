// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Returns the core library version (smoke test for the IPC bridge).
#[tauri::command]
fn core_version() -> &'static str {
    ks_core::VERSION
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![core_version])
        .run(tauri::generate_context!())
        .expect("failed to run KeySwitch");
}
