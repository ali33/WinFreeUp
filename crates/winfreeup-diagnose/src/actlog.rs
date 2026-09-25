//! Nhật ký hành động của Khám máy (xóa vào Thùng rác, bật/tắt khởi động, kết thúc app): file
//! `YYYY-MM-DD_HHmmss.log` nằm thẳng trong thư mục người gọi truyền vào `ActionLog::new` — crate này không
//! tự tính thư mục nhật ký (vỏ app truyền thư mục dữ liệu an toàn của app, vd `secure_data_dir()?.join("logs")`).
//! File chỉ được tạo ở dòng đầu tiên, để mở tab mà không làm gì thì không sinh file rỗng.
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use chrono::Local;

pub struct ActionLog {
    dir: PathBuf,
    file: Mutex<Option<(PathBuf, File)>>,
}

impl ActionLog {
    pub fn new(dir: PathBuf) -> Self {
        ActionLog { dir, file: Mutex::new(None) }
    }

    /// Ghi một dòng có dấu giờ. Trả `Err(thông điệp)` nếu không ghi được — người gọi báo lên màn.
    pub fn line(&self, text: &str) -> Result<(), String> {
        let mut guard = self.file.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
            let path = self.dir.join(Local::now().format("%Y-%m-%d_%H%M%S.log").to_string());
            let f = OpenOptions::new().create(true).append(true).open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            *guard = Some((path, f));
        }
        let (path, f) = guard.as_mut().expect("vừa mở");
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
        writeln!(f, "{ts} {text}").and_then(|_| f.flush()).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn path(&self) -> Option<PathBuf> {
        self.file.lock().ok().and_then(|g| g.as_ref().map(|(p, _)| p.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_is_created_lazily_and_lines_append() {
        let t = tempfile::tempdir().unwrap();
        let log = ActionLog::new(t.path().join("WinFreeUp").join("logs"));
        assert!(log.path().is_none());
        log.line(r"DISK_RECYCLE bytes=10 files=1 C:\Users\a\Tải về\x.iso").unwrap();
        log.line("STARTUP_DISABLE hkcu_run OneDrive").unwrap();
        let path = log.path().unwrap();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert_eq!(name.len(), "2026-09-25_101010.log".len());
        let text = fs::read_to_string(path).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.contains(r"Tải về\x.iso"));
    }

    #[test]
    fn log_file_lives_exactly_in_the_dir_passed_in() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("S-1-5-21-giả").join("logs");
        let log = ActionLog::new(dir.clone());
        log.line("STARTUP_ENABLE hklm_run Teams").unwrap();
        let path = log.path().unwrap();
        assert_eq!(path.parent(), Some(dir.as_path()));
        assert_eq!(fs::read_dir(t.path()).unwrap().count(), 1, "không được tạo gì ngoài dir được truyền");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }
}
