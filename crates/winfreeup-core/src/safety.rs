//! Luật an toàn (spec mục 6): gốc được phép + canonicalize, không đi theo reparse point,
//! file khóa/không quyền thì bỏ qua và đếm.
//!
//! Chống TOCTOU: `Guard::check` chỉ là lọc sớm theo đường dẫn. Việc xóa thật luôn đi qua
//! một handle mở bằng `FILE_FLAG_OPEN_REPARSE_POINT`; đường dẫn thật của chính handle đó
//! (`GetFinalPathNameByHandleW`) được kiểm lại với các gốc, rồi xóa qua handle
//! (`SetFileInformationByHandle`). Thay thư mục cha bằng junction giữa hai bước không còn
//! làm app xóa nhầm file ngoài gốc.
use std::fs;
use std::io;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::error::{io_err, CoreError, Result};

const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const ERROR_FILE_NOT_FOUND: i32 = 2;
const ERROR_PATH_NOT_FOUND: i32 = 3;
const ERROR_SHARING_VIOLATION: i32 = 32;
const ERROR_LOCK_VIOLATION: i32 = 33;
const ERROR_DELETE_PENDING: i32 = 303;

pub fn is_reparse_point(meta: &fs::Metadata) -> bool {
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Chuẩn hóa thư mục cha rồi ghép tên: bản thân liên kết không bị phân giải sang đích.
fn canonical_location(path: &Path) -> io::Result<PathBuf> {
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => {
            Ok(fs::canonicalize(parent)?.join(name))
        }
        _ => fs::canonicalize(path),
    }
}

#[derive(Debug, Clone)]
pub struct Guard {
    roots: Vec<PathBuf>,
}

impl Guard {
    /// Gốc không tồn tại bị bỏ qua (không có gì để xóa ở đó).
    pub fn new(roots: &[PathBuf]) -> Guard {
        Guard { roots: roots.iter().filter_map(|r| fs::canonicalize(r).ok()).collect() }
    }

    pub fn is_allowed(&self, path: &Path) -> bool {
        match canonical_location(path) {
            Ok(loc) => self.contains_final(&loc),
            Err(_) => false,
        }
    }

    /// So một đường dẫn ĐÃ là đường dẫn thật (dạng `\\?\` như `fs::canonicalize` trả về)
    /// với các gốc — thuần so chuỗi, không chạm hệ thống file nên không bị tráo giữa chừng.
    fn contains_final(&self, final_path: &Path) -> bool {
        self.roots.iter().any(|r| final_path.starts_with(r))
    }

    pub fn check(&self, path: &Path) -> Result<()> {
        if self.is_allowed(path) {
            Ok(())
        } else {
            Err(CoreError::OutsideRoots(path.to_path_buf()))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted(u64),
    WouldDelete(u64),
    Locked,
    Vanished,
}

fn is_locked(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(ERROR_SHARING_VIOLATION) | Some(ERROR_LOCK_VIOLATION))
}

fn is_vanished(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::NotFound
        || matches!(
            e.raw_os_error(),
            Some(ERROR_FILE_NOT_FOUND) | Some(ERROR_PATH_NOT_FOUND) | Some(ERROR_DELETE_PENDING)
        )
}

/// Phân loại lỗi mở/xóa: biến mất ⇒ `Vanished` (kể cả sau khi đã bỏ read-only);
/// khóa hoặc không quyền ⇒ `Locked`; còn lại là lỗi thật.
fn classify(path: &Path, e: io::Error) -> Result<DeleteOutcome> {
    if is_vanished(&e) {
        Ok(DeleteOutcome::Vanished)
    } else if is_locked(&e) || e.kind() == io::ErrorKind::PermissionDenied {
        Ok(DeleteOutcome::Locked)
    } else {
        Err(io_err(path, e))
    }
}

/// Mở handle phục vụ xóa: không đi theo reparse point ở phần tử cuối, mở được cả thư mục.
/// `std::fs::File` gọi `CreateFileW` (tự thêm `\\?\` cho đường dẫn dài) và đóng handle khi drop.
#[cfg(windows)]
fn open_for_delete(path: &Path) -> io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_WRITE_ATTRIBUTES,
    };
    fs::OpenOptions::new()
        // FILE_WRITE_ATTRIBUTES: chỉ để đường lùi bỏ cờ read-only qua handle trên Windows cũ.
        .access_mode(DELETE | FILE_READ_ATTRIBUTES | FILE_WRITE_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

/// Đường dẫn thật của đối tượng mà handle đang trỏ tới, dạng `\\?\C:\...` / `\\?\UNC\...`
/// — cùng dạng với gốc đã `fs::canonicalize` trong `Guard`.
#[cfg(windows)]
fn final_path(file: &fs::File) -> io::Result<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFinalPathNameByHandleW, FILE_NAME_NORMALIZED, VOLUME_NAME_DOS,
    };
    let mut buf: Vec<u16> = vec![0; 512];
    loop {
        let cap = u32::try_from(buf.len()).unwrap_or(u32::MAX);
        // SAFETY: handle sống suốt lời gọi (mượn từ `file`); `buf` ghi được `cap` phần tử u16.
        let n = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle(),
                buf.as_mut_ptr(),
                cap,
                FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
            )
        } as usize;
        if n == 0 {
            return Err(io::Error::last_os_error());
        }
        if n < buf.len() {
            buf.truncate(n);
            return Ok(PathBuf::from(OsString::from_wide(&buf)));
        }
        // Thiếu chỗ: `n` là số phần tử cần (kể cả NUL).
        buf.resize(n + 1, 0);
    }
}

#[cfg(windows)]
fn set_info<T>(file: &fs::File, class: i32, info: &T) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::SetFileInformationByHandle;
    // SAFETY: handle sống suốt lời gọi; `info` là struct #[repr(C)] đúng lớp `class`,
    // con trỏ và kích thước lấy từ cùng một tham chiếu hợp lệ.
    let ok = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            class,
            (info as *const T).cast(),
            std::mem::size_of::<T>() as u32,
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Đặt cờ xóa qua handle. Ưu tiên `FileDispositionInfoEx` (POSIX + bỏ qua read-only);
/// Windows/hệ thống file không hỗ trợ ⇒ lùi về `FileDispositionInfo`.
#[cfg(windows)]
fn dispose(file: &fs::File, attrs: u32) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{
        FileDispositionInfoEx, FILE_DISPOSITION_FLAG_DELETE,
        FILE_DISPOSITION_FLAG_IGNORE_READONLY_ATTRIBUTE, FILE_DISPOSITION_FLAG_POSIX_SEMANTICS,
        FILE_DISPOSITION_INFO_EX,
    };
    const ERROR_INVALID_FUNCTION: i32 = 1;
    const ERROR_NOT_SUPPORTED: i32 = 50;
    const ERROR_INVALID_PARAMETER: i32 = 87;
    let info = FILE_DISPOSITION_INFO_EX {
        Flags: FILE_DISPOSITION_FLAG_DELETE
            | FILE_DISPOSITION_FLAG_POSIX_SEMANTICS
            | FILE_DISPOSITION_FLAG_IGNORE_READONLY_ATTRIBUTE,
    };
    match set_info(file, FileDispositionInfoEx, &info) {
        Err(e)
            if matches!(
                e.raw_os_error(),
                Some(ERROR_INVALID_PARAMETER)
                    | Some(ERROR_NOT_SUPPORTED)
                    | Some(ERROR_INVALID_FUNCTION)
            ) =>
        {
            dispose_legacy(file, attrs)
        }
        r => r,
    }
}

/// Đường lùi: tự bỏ read-only qua handle (nếu có) rồi đặt `FileDispositionInfo`.
#[cfg(windows)]
fn dispose_legacy(file: &fs::File, attrs: u32) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{
        FileBasicInfo, FileDispositionInfo, FILE_ATTRIBUTE_NORMAL, FILE_BASIC_INFO,
        FILE_DISPOSITION_INFO,
    };
    if attrs & FILE_ATTRIBUTE_READONLY != 0 {
        let cleared = attrs & !FILE_ATTRIBUTE_READONLY;
        // Thời gian = 0 nghĩa là "giữ nguyên"; thuộc tính 0 cũng là "giữ nguyên" nên dùng NORMAL.
        let basic = FILE_BASIC_INFO {
            CreationTime: 0,
            LastAccessTime: 0,
            LastWriteTime: 0,
            ChangeTime: 0,
            FileAttributes: if cleared == 0 { FILE_ATTRIBUTE_NORMAL } else { cleared },
        };
        set_info(file, FileBasicInfo, &basic)?;
    }
    set_info(file, FileDispositionInfo, &FILE_DISPOSITION_INFO { DeleteFile: true })
}

/// Lõi xóa chống TOCTOU: mở handle → kiểm lại đường dẫn thật với gốc → xóa qua handle.
/// Tự đứng được kể cả khi `Guard::check` trước đó đã bị qua mặt.
#[cfg(windows)]
fn delete_checked_by_handle(guard: &Guard, path: &Path) -> Result<DeleteOutcome> {
    let file = match open_for_delete(path) {
        Ok(f) => f,
        Err(e) => return classify(path, e),
    };
    let real = final_path(&file).map_err(|e| io_err(path, e))?;
    if !guard.contains_final(&real) {
        return Err(CoreError::OutsideRoots(path.to_path_buf()));
    }
    let meta = file.metadata().map_err(|e| io_err(path, e))?;
    let reparse = is_reparse_point(&meta);
    if meta.is_dir() && !reparse {
        return Err(CoreError::System(format!(
            "refusing to delete a directory as a file: {}",
            path.display()
        )));
    }
    let bytes = if reparse { 0 } else { meta.len() };
    match dispose(&file, meta.file_attributes()) {
        Ok(()) => Ok(DeleteOutcome::Deleted(bytes)),
        Err(e) => classify(path, e),
    }
    // `file` drop khi ra khỏi hàm: handle đóng, tên file biến mất.
}

pub fn delete_path(guard: &Guard, path: &Path, dry_run: bool) -> Result<DeleteOutcome> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(DeleteOutcome::Vanished),
        Err(e) => return Err(io_err(path, e)),
    };
    guard.check(path)?;
    if !dry_run {
        return delete_checked_by_handle(guard, path);
    }
    // Dry-run: không mở handle DELETE, chỉ báo theo metadata.
    let reparse = is_reparse_point(&meta);
    if meta.is_dir() && !reparse {
        return Err(CoreError::System(format!(
            "refusing to delete a directory as a file: {}",
            path.display()
        )));
    }
    Ok(DeleteOutcome::WouldDelete(if reparse { 0 } else { meta.len() }))
}

/// Như `remove_empty_dir` nhưng không có lọc sớm: kiểm lại bằng đường dẫn thật của handle.
#[cfg(windows)]
fn remove_empty_dir_checked(guard: &Guard, dir: &Path) -> bool {
    let Ok(file) = open_for_delete(dir) else { return false };
    let Ok(real) = final_path(&file) else { return false };
    if !guard.contains_final(&real) {
        return false;
    }
    match file.metadata() {
        // Thư mục không rỗng ⇒ SetFileInformationByHandle báo ERROR_DIR_NOT_EMPTY ⇒ false.
        Ok(m) if m.is_dir() && !is_reparse_point(&m) => dispose(&file, m.file_attributes()).is_ok(),
        _ => false,
    }
}

/// Xóa thư mục nếu rỗng, nằm trong gốc được phép và không phải reparse point.
pub fn remove_empty_dir(guard: &Guard, dir: &Path) -> bool {
    guard.is_allowed(dir) && remove_empty_dir_checked(guard, dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::write_file;
    use std::fs;
    use std::os::windows::fs::OpenOptionsExt;

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        let outside = tmp.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        (tmp, root, outside)
    }

    #[test]
    fn deleting_outside_allowed_roots_is_refused() {
        let (_t, root, outside) = setup();
        let secret = write_file(&outside.join("secret.txt"), 5);
        let guard = Guard::new(std::slice::from_ref(&root));
        for dry in [false, true] {
            let err = delete_path(&guard, &secret, dry).unwrap_err();
            assert!(matches!(err, CoreError::OutsideRoots(_)), "{err}");
        }
        assert!(secret.exists());
    }

    #[test]
    fn dotdot_escape_is_refused() {
        let (_t, root, outside) = setup();
        let secret = write_file(&outside.join("secret.txt"), 5);
        let sneaky = root.join("..").join("outside").join("secret.txt");
        let guard = Guard::new(&[root]);
        assert!(matches!(delete_path(&guard, &sneaky, false), Err(CoreError::OutsideRoots(_))));
        assert!(secret.exists());
    }

    #[test]
    fn file_inside_root_is_deleted_and_bytes_reported() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("a").join("x.tmp"), 7);
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &f, false).unwrap(), DeleteOutcome::Deleted(7));
        assert!(!f.exists());
    }

    #[test]
    fn dry_run_keeps_the_file() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("x.tmp"), 5);
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &f, true).unwrap(), DeleteOutcome::WouldDelete(5));
        assert!(f.exists());
    }

    #[test]
    fn locked_file_is_skipped_not_failed() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("busy.tmp"), 5);
        let _handle = fs::OpenOptions::new().read(true).share_mode(0).open(&f).unwrap();
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &f, false).unwrap(), DeleteOutcome::Locked);
        assert!(f.exists());
    }

    // Review Focus 2
    #[test]
    fn readonly_file_is_deleted_not_counted_as_locked() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("ro.tmp"), 4);
        let mut p = fs::metadata(&f).unwrap().permissions();
        p.set_readonly(true);
        fs::set_permissions(&f, p).unwrap();
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &f, false).unwrap(), DeleteOutcome::Deleted(4));
        assert!(!f.exists());
    }

    // Review Focus 3
    #[test]
    fn vanished_file_is_reported_as_vanished() {
        let (_t, root, _o) = setup();
        let guard = Guard::new(std::slice::from_ref(&root));
        assert_eq!(delete_path(&guard, &root.join("gone.tmp"), false).unwrap(), DeleteOutcome::Vanished);
    }

    #[test]
    fn junction_is_removed_without_touching_its_target() {
        let (_t, root, outside) = setup();
        let secret = write_file(&outside.join("secret.txt"), 5);
        let link = root.join("link");
        junction::create(&outside, &link).unwrap();
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &link, false).unwrap(), DeleteOutcome::Deleted(0));
        assert!(fs::symlink_metadata(&link).is_err());
        assert!(secret.exists());
    }

    #[test]
    fn plain_directory_is_refused_by_delete_path() {
        let (_t, root, _o) = setup();
        let d = root.join("dir");
        fs::create_dir_all(&d).unwrap();
        let guard = Guard::new(&[root]);
        assert!(matches!(delete_path(&guard, &d, false), Err(CoreError::System(_))));
        assert!(d.exists());
    }

    #[test]
    fn path_longer_than_260_chars_is_deleted() {
        let (_t, root, _o) = setup();
        let mut deep = root.clone();
        for i in 0..25 {
            deep = deep.join(format!("thu_muc_sau_{i:02}"));
        }
        let f = write_file(&deep.join("tệp tạm.tmp"), 3);
        assert!(f.as_os_str().len() > 260);
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &f, false).unwrap(), DeleteOutcome::Deleted(3));
    }

    #[test]
    fn remove_empty_dir_respects_emptiness_and_roots() {
        let (_t, root, outside) = setup();
        let empty = root.join("empty");
        fs::create_dir_all(&empty).unwrap();
        let full = root.join("full");
        write_file(&full.join("f"), 1);
        let guard = Guard::new(&[root]);
        assert!(remove_empty_dir(&guard, &empty));
        assert!(!empty.exists());
        assert!(!remove_empty_dir(&guard, &full));
        assert!(full.exists());
        assert!(!remove_empty_dir(&guard, &outside));
        assert!(outside.exists());
    }

    /// TOCTOU: thư mục cha bị thay bằng junction trỏ ra ngoài sau khi đã qua `Guard::check`.
    /// Gọi thẳng đường xóa-qua-handle (bỏ qua check đầu) để chứng minh lớp kiểm lại
    /// bằng đường dẫn thật tự nó chặn được.
    #[test]
    fn parent_junction_swapped_after_check_is_refused_by_handle_recheck() {
        let (_t, root, outside) = setup();
        let secret = write_file(&outside.join("secret.txt"), 5);
        let swapped = root.join("cache");
        junction::create(&outside, &swapped).unwrap();
        let guard = Guard::new(std::slice::from_ref(&root));
        let via_link = swapped.join("secret.txt");
        let err = delete_checked_by_handle(&guard, &via_link).unwrap_err();
        assert!(matches!(err, CoreError::OutsideRoots(_)), "{err}");
        assert!(secret.exists());
    }

    #[test]
    fn parent_junction_swapped_after_check_keeps_outside_empty_dir() {
        let (_t, root, outside) = setup();
        let victim = outside.join("empty");
        fs::create_dir_all(&victim).unwrap();
        let swapped = root.join("cache");
        junction::create(&outside, &swapped).unwrap();
        let guard = Guard::new(std::slice::from_ref(&root));
        assert!(!remove_empty_dir_checked(&guard, &swapped.join("empty")));
        assert!(victim.exists());
    }

    #[test]
    fn handle_path_deletes_readonly_file_inside_root() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("ro2.tmp"), 6);
        let mut p = fs::metadata(&f).unwrap().permissions();
        p.set_readonly(true);
        fs::set_permissions(&f, p).unwrap();
        let guard = Guard::new(&[root]);
        assert_eq!(delete_checked_by_handle(&guard, &f).unwrap(), DeleteOutcome::Deleted(6));
        assert!(!f.exists());
    }

    #[test]
    fn legacy_disposition_clears_readonly_and_deletes() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("ro3.tmp"), 2);
        let mut p = fs::metadata(&f).unwrap().permissions();
        p.set_readonly(true);
        fs::set_permissions(&f, p).unwrap();
        let file = open_for_delete(&f).unwrap();
        let attrs = file.metadata().unwrap().file_attributes();
        dispose_legacy(&file, attrs).unwrap();
        drop(file);
        assert!(fs::symlink_metadata(&f).is_err());
    }

    #[test]
    fn file_open_without_delete_sharing_is_locked_on_handle_path() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("busy2.tmp"), 5);
        let _handle = fs::OpenOptions::new().read(true).share_mode(1).open(&f).unwrap();
        let guard = Guard::new(&[root]);
        assert_eq!(delete_checked_by_handle(&guard, &f).unwrap(), DeleteOutcome::Locked);
        assert!(f.exists());
    }
}
