//! Công cụ test: hệ thống giả và cây thư mục giả. Không bao giờ chạm hệ thống thật.
#![allow(dead_code)]
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, SetFileTime, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_WRITE_ATTRIBUTES, OPEN_EXISTING,
};

use crate::env::{Env, SystemOps};
use crate::error::{CoreError, Result};
use crate::types::{CancelToken, CleanOptions, CleanReport, Cleaner, ItemAction, Progress, RiskLevel, ScanResult};

#[derive(Default)]
pub struct FakeSys {
    pub calls: Mutex<Vec<String>>,
    pub running: Vec<String>,
    pub service_was_running: bool,
    pub start_fails: bool,
    pub dism_output: String,
    pub dism_fails: bool,
    pub recycle: (u64, u64),
    pub restore_error: Option<String>,
}

impl FakeSys {
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    fn record(&self, s: String) {
        self.calls.lock().unwrap().push(s);
    }
}

impl SystemOps for FakeSys {
    fn is_process_running(&self, exe_name: &str) -> bool {
        self.running.iter().any(|r| r.eq_ignore_ascii_case(exe_name))
    }
    fn stop_service(&self, name: &str) -> Result<bool> {
        self.record(format!("stop:{name}"));
        Ok(self.service_was_running)
    }
    fn start_service(&self, name: &str) -> Result<()> {
        self.record(format!("start:{name}"));
        if self.start_fails {
            Err(CoreError::System("start failed".into()))
        } else {
            Ok(())
        }
    }
    fn run_dism(&self, args: &[&str], cancel: &CancelToken, on_line: &mut dyn FnMut(&str)) -> Result<()> {
        self.record(format!("dism:{}", args.join(" ")));
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        if self.dism_fails {
            return Err(CoreError::System("dism failed".into()));
        }
        for line in self.dism_output.split(['\r', '\n']).map(str::trim).filter(|l| !l.is_empty()) {
            on_line(line);
        }
        Ok(())
    }
    fn recycle_bin_size(&self) -> Result<(u64, u64)> {
        self.record("rb_size".into());
        Ok(self.recycle)
    }
    fn empty_recycle_bin(&self) -> Result<()> {
        self.record("rb_empty".into());
        Ok(())
    }
    fn create_restore_point(&self, _description: &str) -> Result<()> {
        self.record("restore".into());
        match &self.restore_error {
            Some(m) => Err(CoreError::System(m.clone())),
            None => Ok(()),
        }
    }
    fn take_ownership(&self, path: &Path) -> Result<()> {
        self.record(format!("own:{}", path.display()));
        Ok(())
    }
}

/// Env trỏ vào cây giả: root/Temp, root/Windows, root/Local, root/ProgramData, ổ hệ thống = root.
pub fn fake_env(root: &Path, sys: Arc<FakeSys>) -> Env {
    let env = Env {
        temp: root.join("Temp"),
        windir: root.join("Windows"),
        local_appdata: root.join("Local"),
        program_data: root.join("ProgramData"),
        system_drive: root.to_path_buf(),
        now: SystemTime::now(),
        sys,
    };
    for d in [&env.temp, &env.windir, &env.local_appdata, &env.program_data] {
        fs::create_dir_all(d).unwrap();
    }
    env
}

pub fn write_file(path: &Path, len: usize) -> PathBuf {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![b'x'; len]).unwrap();
    path.to_path_buf()
}

/// Đặt CẢ giờ sửa đổi (mtime) lẫn giờ tạo (creation time) của file/thư mục lùi về `hours` giờ
/// trước. `fsclean::found()` tính tuổi theo thời điểm MỚI HƠN giữa hai mốc này (bộ cài bung file
/// vào %TEMP% giữ mtime cũ trong gói nhưng creation time là lúc vừa bung ⇒ không tính là cũ), nên
/// muốn mô phỏng "file thật sự cũ" trong test phải lùi cả hai, không chỉ mtime.
pub fn age(path: &Path, hours: u64) {
    let t = SystemTime::now() - Duration::from_secs(hours * 3600);
    filetime::set_file_mtime(path, filetime::FileTime::from_system_time(t)).unwrap();
    set_created(path, t);
}

/// Đặt creation time qua `SetFileTime` (Windows không có API chuẩn nào khác cho việc này).
fn set_created(path: &Path, t: SystemTime) {
    let dur = t.duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let ticks = dur.as_secs() * 10_000_000 + u64::from(dur.subsec_nanos()) / 100 + 116_444_736_000_000_000;
    let ft = FILETIME { dwLowDateTime: (ticks & 0xFFFF_FFFF) as u32, dwHighDateTime: (ticks >> 32) as u32 };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    unsafe {
        let handle = CreateFileW(
            wide.as_ptr(),
            FILE_WRITE_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        );
        assert!(handle != INVALID_HANDLE_VALUE, "CreateFileW({}) failed: {}", path.display(), std::io::Error::last_os_error());
        let ok = SetFileTime(handle, &ft, std::ptr::null(), std::ptr::null());
        let err = std::io::Error::last_os_error();
        CloseHandle(handle);
        assert!(ok != 0, "SetFileTime({}) failed: {err}", path.display());
    }
}

#[derive(Default)]
pub struct Recorder {
    pub items: Mutex<Vec<(ItemAction, PathBuf, u64)>>,
    pub percents: Mutex<Vec<f32>>,
}

impl Recorder {
    pub fn actions(&self) -> Vec<ItemAction> {
        self.items.lock().unwrap().iter().map(|i| i.0).collect()
    }
}

impl Progress for Recorder {
    fn item(&self, action: ItemAction, path: &Path, bytes: u64, _detail: Option<&str>) {
        self.items.lock().unwrap().push((action, path.to_path_buf(), bytes));
    }
    fn percent(&self, pct: f32) {
        self.percents.lock().unwrap().push(pct);
    }
}

pub struct FakeCleaner {
    pub id: &'static str,
    pub risk: RiskLevel,
    pub bytes: u64,
    pub fail: Option<&'static str>,
    pub panics: bool,
}

impl FakeCleaner {
    pub fn ok(id: &'static str, risk: RiskLevel, bytes: u64) -> Box<dyn Cleaner> {
        Box::new(FakeCleaner { id, risk, bytes, fail: None, panics: false })
    }
}

impl Cleaner for FakeCleaner {
    fn id(&self) -> &'static str {
        self.id
    }
    fn risk(&self) -> RiskLevel {
        self.risk
    }
    fn default_selected(&self) -> bool {
        self.risk == RiskLevel::Safe
    }
    fn allowed_roots(&self, _env: &Env) -> Vec<PathBuf> {
        vec![]
    }
    fn scan(&self, _env: &Env, cancel: &CancelToken) -> Result<ScanResult> {
        if self.panics {
            panic!("boom in {}", self.id);
        }
        if let Some(m) = self.fail {
            return Err(CoreError::System(m.into()));
        }
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        Ok(ScanResult { total_bytes: self.bytes, file_count: 1, ..Default::default() })
    }
    fn clean(&self, _env: &Env, _scan: &ScanResult, opts: &CleanOptions, progress: &dyn Progress) -> Result<CleanReport> {
        if self.panics {
            panic!("boom in {}", self.id);
        }
        if let Some(m) = self.fail {
            return Err(CoreError::System(m.into()));
        }
        progress.item(ItemAction::Deleted, Path::new(self.id), self.bytes, None);
        progress.percent(50.0);
        Ok(CleanReport { bytes_freed: self.bytes, files_deleted: 1, dry_run: opts.dry_run, ..Default::default() })
    }
}
