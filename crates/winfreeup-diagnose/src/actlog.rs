//! Nhật ký hành động của Khám máy (xóa vào Thùng rác, bật/tắt khởi động, kết thúc app): file
//! `YYYY-MM-DD_HHmmss.log` nằm thẳng trong thư mục người gọi truyền vào `ActionLog::new` — crate này không
//! tự tính thư mục nhật ký (vỏ app truyền thư mục dữ liệu an toàn của app, vd `secure_data_dir()?.join("logs")`).
//! File chỉ được tạo ở dòng đầu tiên, để mở tab mà không làm gì thì không sinh file rỗng.
//! App chạy quyền Admin nên không bao giờ đi theo reparse point (junction/symlink): thư mục nhật ký, các thư mục
//! mình tạo ra và chính file nhật ký đều phải là thật; file luôn là file MỚI (`create_new`), không mở lại file có sẵn.
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::Local;
use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT};

pub struct ActionLog {
    dir: PathBuf,
    file: Mutex<Option<(PathBuf, File)>>,
}

/// `Err` nếu `p` là reparse point (junction, symlink…).
fn refuse_reparse(p: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(p).map_err(|e| format!("{}: {e}", p.display()))?;
    if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(format!("{}: is a reparse point (junction/symlink), refusing to write the log", p.display()));
    }
    Ok(())
}

/// Tạo `dir` và các thư mục cha còn thiếu; kiểm `dir` cùng mọi thư mục vừa tạo không phải reparse point.
fn ensure_dir(dir: &Path) -> Result<(), String> {
    let missing: Vec<&Path> = dir.ancestors().take_while(|p| fs::symlink_metadata(p).is_err()).collect();
    for p in missing.iter().rev() {
        match fs::create_dir(p) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
            Err(e) => return Err(format!("{}: {e}", p.display())),
        }
        refuse_reparse(p)?;
    }
    refuse_reparse(dir)
}

/// Mở file nhật ký MỚI `YYYY-MM-DD_HHmmss[_N].log`; không đi theo reparse point, không dùng lại file có sẵn.
fn create_log_file(dir: &Path) -> Result<(PathBuf, File), String> {
    let stem = Local::now().format("%Y-%m-%d_%H%M%S").to_string();
    for n in 0..100 {
        let name = if n == 0 { format!("{stem}.log") } else { format!("{stem}_{n}.log") };
        let path = dir.join(name);
        match OpenOptions::new().create_new(true).append(true).custom_flags(FILE_FLAG_OPEN_REPARSE_POINT).open(&path) {
            Ok(f) => return Ok((path, f)),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("{}: {e}", path.display())),
        }
    }
    Err(format!("{}: too many log files named {stem}*.log", dir.display()))
}

impl ActionLog {
    /// `dir` là thư mục nhật ký do người gọi cấp. Người gọi PHẢI truyền thư mục có ACL chỉ cho SYSTEM và
    /// Administrators ghi (vd `winfreeup_core::secure_data_dir()?.join("logs")`) — thư mục người dùng thường
    /// ghi được thì họ cài sẵn liên kết hay file giả được.
    pub fn new(dir: PathBuf) -> Self {
        ActionLog { dir, file: Mutex::new(None) }
    }

    /// Ghi một dòng có dấu giờ. Trả `Err(thông điệp)` nếu không ghi được — người gọi báo lên màn.
    pub fn line(&self, text: &str) -> Result<(), String> {
        let mut guard = self.file.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            ensure_dir(&self.dir)?;
            *guard = Some(create_log_file(&self.dir)?);
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

    #[test]
    fn junction_log_dir_is_refused_and_target_is_untouched() {
        let t = tempfile::tempdir().unwrap();
        let target = t.path().join("dich");
        fs::create_dir(&target).unwrap();
        let logs = t.path().join("logs");
        junction::create(&target, &logs).unwrap();
        let log = ActionLog::new(logs.clone());
        let err = log.line("DISK_RECYCLE bytes=1 files=1 C:\\x").unwrap_err();
        assert!(err.contains("reparse point"), "{err}");
        assert!(err.contains(&logs.display().to_string()), "{err}");
        assert!(log.path().is_none());
        assert_eq!(fs::read_dir(&target).unwrap().count(), 0, "không được ghi vào đích của junction");
    }

    #[test]
    fn two_logs_in_the_same_second_never_share_a_file() {
        let t = tempfile::tempdir().unwrap();
        let a = ActionLog::new(t.path().to_path_buf());
        let b = ActionLog::new(t.path().to_path_buf());
        a.line("A").unwrap();
        b.line("B").unwrap();
        assert_ne!(a.path(), b.path());
        assert_eq!(fs::read_to_string(a.path().unwrap()).unwrap().lines().count(), 1);
        assert_eq!(fs::read_to_string(b.path().unwrap()).unwrap().lines().count(), 1);
    }

    /// Symlink file cài sẵn đúng tên file nhật ký sắp tạo: không được ghi xuyên qua nó.
    /// Tạo symlink file cần quyền SeCreateSymbolicLink hoặc Developer Mode — không có thì bỏ qua (in lý do).
    #[test]
    fn planted_symlink_with_the_log_name_is_not_followed() {
        let t = tempfile::tempdir().unwrap();
        let victim = t.path().join("victim.txt");
        fs::write(&victim, "nguyên vẹn").unwrap();
        let dir = t.path().join("logs");
        fs::create_dir(&dir).unwrap();
        let now = Local::now();
        let mut planted = Vec::new();
        for s in 0..3 {
            let name = (now + chrono::Duration::seconds(s)).format("%Y-%m-%d_%H%M%S.log").to_string();
            let link = dir.join(name);
            if let Err(e) = std::os::windows::fs::symlink_file(&victim, &link) {
                eprintln!("bỏ qua: không tạo được symlink file không cần quyền ({e})");
                return;
            }
            planted.push(link);
        }
        let log = ActionLog::new(dir);
        log.line("STARTUP_DISABLE hkcu_run OneDrive").unwrap();
        assert!(!planted.contains(&log.path().unwrap()));
        assert_eq!(fs::read_to_string(&victim).unwrap(), "nguyên vẹn");
    }
}
