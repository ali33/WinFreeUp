use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use winfreeup_core::cleaners::registry::all_cleaners;
use winfreeup_core::engine::{self, CleanSummary, GroupScan, RestorePointStatus};
use winfreeup_core::log::log_dir;
use winfreeup_core::sys_windows::disk_free as core_disk_free;
use winfreeup_core::{CancelToken, CleanOptions, Cleaner, Env, ScanResult};

pub struct AppState {
    env: Env,
    cleaners: Vec<Box<dyn Cleaner>>,
    cancel: Mutex<CancelToken>,
    scans: Mutex<HashMap<String, ScanResult>>,
    busy: AtomicBool,
    dry_run_cli: bool,
}

pub type Shared = Arc<AppState>;

impl AppState {
    pub fn new(env: Env, dry_run_cli: bool) -> Shared {
        Arc::new(AppState {
            env,
            cleaners: all_cleaners(),
            cancel: Mutex::new(CancelToken::new()),
            scans: Mutex::new(HashMap::new()),
            busy: AtomicBool::new(false),
            dry_run_cli,
        })
    }

    /// "Bây giờ" của mỗi thao tác, để luật "cũ hơn 24 giờ" không dùng giờ lúc mở ứng dụng.
    fn fresh_env(&self) -> Env {
        let mut e = self.env.clone();
        e.now = SystemTime::now();
        e
    }
}

/// Chặn thao tác chồng nhau ở phía lõi; tự nhả khi thao tác kết thúc (kể cả lỗi).
struct BusyGuard(Shared);

impl BusyGuard {
    fn acquire(state: &Shared) -> Result<BusyGuard, String> {
        if state.busy.swap(true, Ordering::SeqCst) {
            Err("busy".into())
        } else {
            Ok(BusyGuard(state.clone()))
        }
    }
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.busy.store(false, Ordering::SeqCst);
    }
}

#[derive(Serialize)]
pub struct AppInfo {
    version: String,
    dry_run: bool,
    system_drive: String,
}

#[tauri::command]
pub fn app_info(app: AppHandle, state: State<'_, Shared>) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        dry_run: state.dry_run_cli,
        system_drive: state.env.system_drive.display().to_string(),
    }
}

#[tauri::command]
pub fn disk_free(state: State<'_, Shared>, drive: Option<String>) -> Result<u64, String> {
    let path = drive.map(PathBuf::from).unwrap_or_else(|| state.env.system_drive.clone());
    core_disk_free(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scan_all(app: AppHandle, state: State<'_, Shared>) -> Result<Vec<GroupScan>, String> {
    let st: Shared = state.inner().clone();
    let guard = BusyGuard::acquire(&st)?;
    let token = CancelToken::new();
    *st.cancel.lock().map_err(|e| e.to_string())? = token.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let env = st.fresh_env();
        let groups = engine::scan_all(&st.cleaners, &env, &token, &|g| {
            let _ = app.emit("scan-progress", g);
        });
        if let Ok(mut map) = st.scans.lock() {
            map.clear();
            for g in &groups {
                if let Some(r) = &g.result {
                    map.insert(g.id.clone(), r.clone());
                }
            }
        }
        groups
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cancel_scan(state: State<'_, Shared>) {
    if let Ok(token) = state.cancel.lock() {
        token.cancel();
    }
}

#[tauri::command]
pub async fn prepare_restore_point(state: State<'_, Shared>, ids: Vec<String>, dry_run: bool) -> Result<RestorePointStatus, String> {
    let st: Shared = state.inner().clone();
    let guard = BusyGuard::acquire(&st)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let env = st.fresh_env();
        engine::prepare_restore_point(&st.cleaners, &env, &ids, dry_run || st.dry_run_cli)
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clean(app: AppHandle, state: State<'_, Shared>, ids: Vec<String>, dry_run: bool) -> Result<CleanSummary, String> {
    let st: Shared = state.inner().clone();
    let guard = BusyGuard::acquire(&st)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let env = st.fresh_env();
        let scans = st.scans.lock().map(|m| m.clone()).unwrap_or_default();
        let opts = CleanOptions { dry_run: dry_run || st.dry_run_cli };
        let result = engine::run_clean(&st.cleaners, &env, &ids, &scans, &opts, &log_dir(&env.local_appdata), &|ev| {
            let _ = app.emit("clean-progress", ev);
        });
        // Buộc quét lại trước lần dọn kế tiếp: id vừa dọn (kể cả dọn lỗi/not_scanned/unknown)
        // không còn phản ánh đúng trạng thái ổ đĩa hiện tại.
        if let Ok(mut map) = st.scans.lock() {
            for id in &ids {
                map.remove(id);
            }
        }
        result.map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn open_log_folder(state: State<'_, Shared>) -> Result<(), String> {
    let dir = log_dir(&state.env.local_appdata);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::process::Command::new("explorer.exe").arg(&dir).spawn().map(|_| ()).map_err(|e| e.to_string())
}
