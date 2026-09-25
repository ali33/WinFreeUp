//! Nhật ký mỗi lượt dọn: %LOCALAPPDATA%\WinFreeUp\logs\YYYY-MM-DD_HHmmss.log (UTF-8).
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use chrono::{DateTime, Local};

use crate::error::{io_err, Result};

pub fn log_dir(local_appdata: &Path) -> PathBuf {
    local_appdata.join("WinFreeUp").join("logs")
}

pub fn log_file_name(at: DateTime<Local>) -> String {
    at.format("%Y-%m-%d_%H%M%S.log").to_string()
}

pub struct CleanLog {
    path: PathBuf,
    file: Mutex<File>,
    failed: AtomicBool,
}

impl CleanLog {
    pub fn create(dir: &Path, at: DateTime<Local>) -> Result<CleanLog> {
        fs::create_dir_all(dir).map_err(|e| io_err(dir, e))?;
        let path = dir.join(log_file_name(at));
        let file = OpenOptions::new().create(true).append(true).open(&path).map_err(|e| io_err(&path, e))?;
        Ok(CleanLog { path, file: Mutex::new(file), failed: AtomicBool::new(false) })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn line(&self, text: &str) {
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
        let ok = match self.file.lock() {
            Ok(mut f) => writeln!(f, "{ts} {text}").and_then(|_| f.flush()).is_ok(),
            Err(_) => false,
        };
        if !ok {
            self.failed.store(true, Ordering::SeqCst);
        }
    }

    /// true nếu có ít nhất một dòng không ghi được (đĩa đầy, bị khóa…) — giao diện phải báo.
    pub fn write_failed(&self) -> bool {
        self.failed.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn log_dir_and_file_name_follow_spec() {
        assert_eq!(log_dir(Path::new(r"C:\Users\a\AppData\Local")), PathBuf::from(r"C:\Users\a\AppData\Local\WinFreeUp\logs"));
        let at = Local.with_ymd_and_hms(2026, 9, 5, 7, 3, 9).unwrap();
        assert_eq!(log_file_name(at), "2026-09-05_070309.log");
    }

    #[test]
    fn creates_dir_and_appends_timestamped_lines() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("WinFreeUp").join("logs");
        let log = CleanLog::create(&dir, Local::now()).unwrap();
        log.line("START dry_run=false ids=user_temp");
        log.line("[user_temp] DELETED 10 C:\\Temp\\tệp.tmp");
        let text = std::fs::read_to_string(log.path()).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with("[user_temp] DELETED 10 C:\\Temp\\tệp.tmp"));
        assert!(!log.write_failed());
    }
}
