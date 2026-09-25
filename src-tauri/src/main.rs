#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod webview_check;

fn main() {
    // Kiểm WebView2 TRƯỚC khi tạo cửa sổ (spec mục 2.2).
    if let Err(e) = tauri::webview_version() {
        webview_check::show_missing(&e.to_string());
        return;
    }
    let dry_run = std::env::args().any(|a| a == "--dry-run");
    let env = match winfreeup_core::Env::from_system() {
        Ok(env) => env,
        Err(e) => {
            webview_check::show_fatal(&e.to_string());
            return;
        }
    };
    let result = tauri::Builder::default()
        .manage(commands::AppState::new(env, dry_run))
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::disk_free,
            commands::scan_all,
            commands::cancel_scan,
            commands::prepare_restore_point,
            commands::clean,
            commands::open_log_folder,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        webview_check::show_fatal(&e.to_string());
    }
}
