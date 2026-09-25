//! Luật an toàn (spec mục 6): gốc được phép + canonicalize, không đi theo reparse point,
//! file khóa/không quyền thì bỏ qua và đếm.
//!
//! Chống TOCTOU: `Guard::check` chỉ là lọc sớm theo đường dẫn. Việc xóa thật luôn đi qua
//! một handle mở bằng `FILE_FLAG_OPEN_REPARSE_POINT`; đường dẫn thật của chính handle đó
//! (`GetFinalPathNameByHandleW`) được kiểm lại với các gốc, rồi xóa qua handle
//! (`SetFileInformationByHandle`). Thay thư mục cha bằng junction giữa hai bước không còn
//! làm app xóa nhầm file ngoài gốc.
//!
//! Bản thân gốc cũng không được tin theo đường dẫn: `Guard::new` mở gốc bằng handle, loại gốc
//! là liên kết hoặc có tổ tiên là liên kết, và giữ handle ghim gốc thư mục suốt đời Guard.
use std::fs;
use std::io;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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
    rejected: Vec<PathBuf>,
    /// Gốc tồn tại nhưng không mở/đọc được (AccessDenied, sharing violation, tên lỗi…): khác
    /// "bị loại vì là liên kết" nên tách riêng để giao diện báo đúng nghĩa. Kèm thông điệp gốc
    /// để ghi nhật ký.
    unreadable: Vec<(PathBuf, String)>,
    /// Handle ghim các gốc, cùng thứ tự với `roots` (không `FILE_SHARE_DELETE`): suốt đời
    /// Guard không ai đổi tên hay tráo được gốc. `Arc` vì Guard `Clone`; handle đóng khi bản
    /// sao cuối bị drop, hoặc khi chính gốc đó được xóa (`remove_empty_dir`).
    pins: Arc<Mutex<Vec<Option<fs::File>>>>,
}

/// Kết quả xét một gốc lúc dựng Guard.
enum RootCheck {
    Accepted(PathBuf, Option<fs::File>),
    Missing,
    Rejected,
    Unreadable(String),
}

impl Guard {
    /// Gốc không tồn tại bị bỏ qua (không có gì để xóa ở đó). Gốc bản thân là reparse point,
    /// hoặc có tổ tiên là junction/symlink (đường dẫn thật lệch đường dẫn chữ), bị loại —
    /// xem `rejected_roots`. Không tin `canonicalize` vì gốc như %TEMP% thuộc quyền người dùng.
    pub fn new(roots: &[PathBuf]) -> Guard {
        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        let mut unreadable = Vec::new();
        let mut pins = Vec::new();
        for r in roots {
            match check_root(r) {
                RootCheck::Accepted(real, pin) => {
                    accepted.push(real);
                    pins.push(pin);
                }
                RootCheck::Missing => {}
                RootCheck::Rejected => rejected.push(r.clone()),
                RootCheck::Unreadable(msg) => unreadable.push((r.clone(), msg)),
            }
        }
        Guard { roots: accepted, rejected, unreadable, pins: Arc::new(Mutex::new(pins)) }
    }

    /// Các gốc bị loại vì là liên kết hoặc nằm dưới liên kết: không xóa gì dưới chúng.
    pub fn rejected_roots(&self) -> &[PathBuf] {
        &self.rejected
    }

    /// Các gốc tồn tại nhưng không mở/đọc được, kèm thông điệp lỗi gốc: cũng không xóa gì dưới chúng.
    pub fn unreadable_roots(&self) -> &[(PathBuf, String)] {
        &self.unreadable
    }

    pub fn is_allowed(&self, path: &Path) -> bool {
        match canonical_location(path) {
            Ok(loc) => self.contains_final(&loc),
            Err(_) => false,
        }
    }

    /// So một đường dẫn ĐÃ là đường dẫn thật (dạng `\\?\` từ `GetFinalPathNameByHandleW`)
    /// với các gốc — thuần so chuỗi, không chạm hệ thống file nên không bị tráo giữa chừng.
    fn contains_final(&self, final_path: &Path) -> bool {
        self.roots.iter().any(|r| final_path.starts_with(r))
    }

    /// Xóa chính một gốc thì phải thả handle ghim của nó trước (handle ghim chặn DELETE).
    /// Việc xóa sau đó vẫn qua handle và kiểm lại đường dẫn thật như mọi đường xóa khác.
    fn release_pin_if_root(&self, dir: &Path) {
        let Ok(loc) = canonical_location(dir) else { return };
        let Some(i) = self.roots.iter().position(|r| same_path_ci(r, &loc)) else { return };
        let mut pins = self.pins.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(pin) = pins.get_mut(i) {
            pin.take();
        }
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

pub(crate) fn is_vanished(e: &io::Error) -> bool {
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
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    fs::OpenOptions::new()
        .access_mode(DELETE | FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

/// Đường dẫn thật của đối tượng mà handle đang trỏ tới, dạng `\\?\C:\...` / `\\?\UNC\...`
/// — cùng dạng với gốc lưu trong `Guard` (cũng lấy bằng hàm này).
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

/// Ghim một gốc: chỉ đọc thuộc tính, không đi theo reparse point, share R|W nhưng KHÔNG
/// `FILE_SHARE_DELETE` ⇒ không ai đổi tên / xóa / tráo được gốc khi handle còn mở.
#[cfg(windows)]
fn open_root_pin(path: &Path) -> io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_LIST_DIRECTORY,
        FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    fs::OpenOptions::new()
        // Phải có quyền mức dữ liệu (LIST_DIRECTORY): handle chỉ-thuộc-tính không được
        // NTFS xét chế độ chia sẻ, tức không ghim được gì.
        .access_mode(FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

/// Đường dẫn CHỮ của gốc: tuyệt đối, dạng `\\?\`, đã khử tên 8.3 (`GetLongPathNameW`).
/// Không phân giải liên kết, nên lệch với `final_path` ⇔ có junction/symlink trên đường đi.
#[cfg(windows)]
fn literal_long_path(path: &Path) -> io::Result<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetLongPathNameW;
    let abs: Vec<u16> = std::path::absolute(path)?.as_os_str().encode_wide().collect();
    let verbatim: Vec<u16> = r"\\?\".encode_utf16().collect();
    let unc: Vec<u16> = r"\\".encode_utf16().collect();
    let mut wide: Vec<u16> = if abs.starts_with(&verbatim) {
        abs
    } else if abs.starts_with(&unc) {
        // `\\server\share` ⇒ `\\?\UNC\server\share`
        let mut v: Vec<u16> = r"\\?\UNC".encode_utf16().collect();
        v.extend_from_slice(&abs[1..]);
        v
    } else {
        let mut v = verbatim;
        v.extend_from_slice(&abs);
        v
    };
    wide.push(0);
    let mut buf: Vec<u16> = vec![0; wide.len().max(512)];
    loop {
        let cap = u32::try_from(buf.len()).unwrap_or(u32::MAX);
        // SAFETY: `wide` kết thúc bằng NUL; `buf` ghi được `cap` phần tử u16.
        let n = unsafe { GetLongPathNameW(wide.as_ptr(), buf.as_mut_ptr(), cap) } as usize;
        if n == 0 {
            return Err(io::Error::last_os_error());
        }
        if n < buf.len() {
            buf.truncate(n);
            return Ok(PathBuf::from(OsString::from_wide(&buf)));
        }
        buf.resize(n + 1, 0);
    }
}

/// So hai đường dẫn không phân biệt hoa/thường, bỏ qua dấu `\` cuối. Theo bảng hoa/thường của hệ
/// điều hành (`CompareStringOrdinal`, từng đơn vị mã như NTFS) chứ không theo Unicode đầy đủ của
/// Rust — nơi "ß".to_uppercase() == "SS" dù NTFS coi đó là hai tên khác.
#[cfg(windows)]
fn same_path_ci(a: &Path, b: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Globalization::{CompareStringOrdinal, CSTR_EQUAL};
    const SEP: u16 = b'\\' as u16;
    let norm = |p: &Path| {
        let mut w: Vec<u16> = p.as_os_str().encode_wide().collect();
        while w.last() == Some(&SEP) {
            w.pop();
        }
        w
    };
    let (a, b) = (norm(a), norm(b));
    let (Ok(la), Ok(lb)) = (i32::try_from(a.len()), i32::try_from(b.len())) else {
        return false;
    };
    // SAFETY: a, b là Vec hợp lệ, độ dài truyền vào đúng số phần tử (độ dài >= 0 ⇒ không cần NUL).
    unsafe { CompareStringOrdinal(a.as_ptr(), la, b.as_ptr(), lb, 1) == CSTR_EQUAL }
}

/// Xét một gốc: phải tồn tại, không phải reparse point, và đường dẫn thật của handle
/// trùng đường dẫn chữ (không có liên kết ở tổ tiên). Handle trả về để ghim gốc.
#[cfg(windows)]
fn check_root(root: &Path) -> RootCheck {
    // Mở/đọc thuộc tính lỗi (AccessDenied, sharing violation, tên không hợp lệ…) là "không đọc
    // được", KHÔNG phải "là liên kết" — báo sai nghĩa làm người dùng đi tìm junction không có.
    // Liên kết (gốc hay tổ tiên) chỉ được kết luận khi đã mở được và thấy nó.
    let pin = match open_root_pin(root) {
        Ok(f) => f,
        Err(e) if is_vanished(&e) => return RootCheck::Missing,
        Err(e) => return RootCheck::Unreadable(io_err(root, e).to_string()),
    };
    let is_dir = match pin.metadata() {
        Ok(m) if !is_reparse_point(&m) => m.is_dir(),
        Ok(_) => return RootCheck::Rejected,
        Err(e) => return RootCheck::Unreadable(io_err(root, e).to_string()),
    };
    let (Ok(real), Ok(literal)) = (final_path(&pin), literal_long_path(root)) else {
        return RootCheck::Rejected;
    };
    if same_path_ci(&real, &literal) {
        // Gốc là file (vd MEMORY.DMP): không ghim, để chính nó xóa được; việc xóa vẫn
        // kiểm lại đường dẫn thật qua handle.
        RootCheck::Accepted(real, is_dir.then_some(pin))
    } else {
        RootCheck::Rejected
    }
}

#[cfg(windows)]
fn file_info(
    file: &fs::File,
) -> io::Result<windows_sys::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    // SAFETY: struct POD toàn số — mọi bit 0 đều hợp lệ.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: handle sống suốt lời gọi; `info` là vùng ghi hợp lệ đúng kiểu.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(info)
}

/// Số hard link của file mà handle trỏ tới.
#[cfg(windows)]
fn link_count(file: &fs::File) -> io::Result<u32> {
    file_info(file).map(|i| i.nNumberOfLinks)
}

/// Định danh dữ liệu của một file có nhiều hard link: (số serial ổ, chỉ số file).
pub(crate) type LinkId = (u32, u64);

/// Như `fs::symlink_metadata` (không đi theo reparse point), kèm định danh dữ liệu khi file có
/// hơn một hard link — để khi quét chỉ đếm byte MỘT lần cho mọi tên trỏ cùng dữ liệu.
///
/// Chi phí gần bằng `symlink_metadata` của std: std cũng mở handle quyền 0 rồi gọi
/// `GetFileInformationByHandle`; ở đây chỉ thêm một lời gọi đó nữa trên cùng handle (không mở
/// thêm), và chỉ cho file thường. Mở lỗi khác "biến mất" (vd sharing violation của pagefile) ⇒ lùi
/// về `symlink_metadata` (std tự lùi sang `FindFirstFileW`), coi như một link.
#[cfg(windows)]
pub(crate) fn lstat_with_link_id(path: &Path) -> io::Result<(fs::Metadata, Option<LinkId>)> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE,
    };
    let file = match fs::OpenOptions::new()
        .access_mode(0)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
    {
        Ok(f) => f,
        Err(e) if is_vanished(&e) => return Err(e),
        Err(_) => return fs::symlink_metadata(path).map(|m| (m, None)),
    };
    let meta = file.metadata()?;
    if meta.is_dir() || is_reparse_point(&meta) {
        return Ok((meta, None));
    }
    let id = match file_info(&file) {
        Ok(i) if i.nNumberOfLinks > 1 => Some((
            i.dwVolumeSerialNumber,
            (u64::from(i.nFileIndexHigh) << 32) | u64::from(i.nFileIndexLow),
        )),
        _ => None,
    };
    Ok((meta, id))
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

/// Đường lùi: tự bỏ read-only (nếu có) qua một handle mở lại từ chính handle đang giữ
/// (`ReOpenFile` + `FILE_WRITE_ATTRIBUTES`, không đi lại đường dẫn), rồi đặt
/// `FileDispositionInfo`. Đặt cờ xóa lỗi ⇒ trả lại thuộc tính cũ.
#[cfg(windows)]
fn dispose_legacy(file: &fs::File, attrs: u32) -> io::Result<()> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        FileBasicInfo, FileDispositionInfo, ReOpenFile, FILE_ATTRIBUTE_ARCHIVE,
        FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_NOT_CONTENT_INDEXED,
        FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_SYSTEM, FILE_ATTRIBUTE_TEMPORARY, FILE_BASIC_INFO,
        FILE_DISPOSITION_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_WRITE_ATTRIBUTES,
    };
    // Chỉ các bit FileBasicInfo đặt được; DIRECTORY/REPARSE_POINT… là bit chỉ đọc.
    const SETTABLE: u32 = FILE_ATTRIBUTE_READONLY
        | FILE_ATTRIBUTE_HIDDEN
        | FILE_ATTRIBUTE_SYSTEM
        | FILE_ATTRIBUTE_ARCHIVE
        | FILE_ATTRIBUTE_TEMPORARY
        | FILE_ATTRIBUTE_OFFLINE
        | FILE_ATTRIBUTE_NOT_CONTENT_INDEXED;
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    if attrs & FILE_ATTRIBUTE_READONLY == 0 {
        return set_info(file, FileDispositionInfo, &disposition);
    }
    // Thời gian = 0 nghĩa là "giữ nguyên"; thuộc tính 0 cũng là "giữ nguyên" nên dùng NORMAL.
    let basic = |a: u32| FILE_BASIC_INFO {
        CreationTime: 0,
        LastAccessTime: 0,
        LastWriteTime: 0,
        ChangeTime: 0,
        FileAttributes: if a == 0 { FILE_ATTRIBUTE_NORMAL } else { a },
    };
    // SAFETY: handle gốc sống suốt lời gọi; ReOpenFile không lấy quyền sở hữu nó.
    let raw = unsafe {
        ReOpenFile(
            file.as_raw_handle(),
            FILE_WRITE_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
        )
    };
    if raw == INVALID_HANDLE_VALUE || raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `raw` là handle hợp lệ vừa mở, chưa ai sở hữu; `File` đóng nó khi drop.
    let writer = unsafe { fs::File::from_raw_handle(raw) };
    let original = attrs & SETTABLE;
    set_info(&writer, FileBasicInfo, &basic(original & !FILE_ATTRIBUTE_READONLY))?;
    let r = set_info(file, FileDispositionInfo, &disposition);
    if r.is_err() {
        let _ = set_info(&writer, FileBasicInfo, &basic(original));
    }
    r
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
    // Hard link: xóa một tên không giải phóng dữ liệu khi còn tên khác trỏ tới.
    let shared = !reparse && link_count(&file).map_err(|e| io_err(path, e))? > 1;
    let bytes = if reparse || shared { 0 } else { meta.len() };
    match dispose(&file, meta.file_attributes()) {
        Ok(()) => Ok(DeleteOutcome::Deleted(bytes)),
        Err(e) => classify(path, e),
    }
    // `file` drop khi ra khỏi hàm: handle đóng, tên file biến mất.
}

pub fn delete_path(guard: &Guard, path: &Path, dry_run: bool) -> Result<DeleteOutcome> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        // Cùng cách phân loại như đường handle: file delete-pending trả AccessDenied ⇒ Locked,
        // biến mất ⇒ Vanished — không thành lỗi thô trên băng hổ phách.
        Err(e) => return classify(path, e),
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
    if !guard.is_allowed(dir) {
        return false;
    }
    guard.release_pin_if_root(dir);
    remove_empty_dir_checked(guard, dir)
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

    /// %TEMP% (thuộc quyền người dùng) là junction → nơi khác đúng lúc dựng Guard:
    /// gốc phải bị loại, không được lưu đích của junction làm gốc.
    #[test]
    fn root_that_is_a_junction_at_guard_creation_is_rejected() {
        let (t, _root, outside) = setup();
        let secret = write_file(&outside.join("secret.txt"), 5);
        let fake_root = t.path().join("fake_temp");
        junction::create(&outside, &fake_root).unwrap();
        let guard = Guard::new(std::slice::from_ref(&fake_root));
        assert_eq!(guard.rejected_roots(), std::slice::from_ref(&fake_root));
        for p in [fake_root.join("secret.txt"), outside.join("secret.txt")] {
            assert!(matches!(delete_path(&guard, &p, false), Err(CoreError::OutsideRoots(_))));
            assert!(matches!(
                delete_checked_by_handle(&guard, &p),
                Err(CoreError::OutsideRoots(_))
            ));
        }
        assert!(secret.exists());
    }

    #[test]
    fn root_below_a_junction_ancestor_is_rejected() {
        let (t, _root, outside) = setup();
        let real_root = outside.join("cache");
        let secret = write_file(&real_root.join("secret.txt"), 5);
        let ancestor = t.path().join("appdata_link");
        junction::create(&outside, &ancestor).unwrap();
        let root_via_link = ancestor.join("cache");
        let guard = Guard::new(std::slice::from_ref(&root_via_link));
        assert_eq!(guard.rejected_roots(), std::slice::from_ref(&root_via_link));
        let p = root_via_link.join("secret.txt");
        assert!(matches!(delete_path(&guard, &p, false), Err(CoreError::OutsideRoots(_))));
        assert!(secret.exists());
    }

    #[test]
    fn plain_root_is_accepted_and_pinned_against_rename() {
        let (t, root, _o) = setup();
        let guard = Guard::new(std::slice::from_ref(&root));
        assert!(guard.rejected_roots().is_empty());
        let clone = guard.clone();
        drop(guard);
        let moved = t.path().join("moved");
        assert!(fs::rename(&root, &moved).is_err(), "gốc đang được ghim");
        drop(clone);
        fs::rename(&root, &moved).unwrap();
    }

    #[test]
    fn hard_linked_file_is_deleted_but_frees_nothing() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("a.tmp"), 9);
        let twin = root.join("b.tmp");
        fs::hard_link(&f, &twin).unwrap();
        let guard = Guard::new(&[root]);
        assert_eq!(delete_checked_by_handle(&guard, &f).unwrap(), DeleteOutcome::Deleted(0));
        assert!(!f.exists());
        assert_eq!(fs::metadata(&twin).unwrap().len(), 9);
    }

    #[test]
    fn missing_file_on_handle_path_is_vanished() {
        let (_t, root, _o) = setup();
        let guard = Guard::new(std::slice::from_ref(&root));
        for p in [root.join("gone.tmp"), root.join("no_dir").join("gone.tmp")] {
            assert_eq!(delete_checked_by_handle(&guard, &p).unwrap(), DeleteOutcome::Vanished);
        }
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
    fn root_that_cannot_be_opened_is_unreadable_not_rejected() {
        let (t, _root, _o) = setup();
        let bad = t.path().join("sai-?-*");
        let guard = Guard::new(std::slice::from_ref(&bad));
        assert!(guard.rejected_roots().is_empty());
        let un = guard.unreadable_roots();
        assert_eq!(un.len(), 1);
        assert_eq!(un[0].0, bad);
        assert!(!un[0].1.is_empty());
    }

    #[test]
    fn same_path_ci_uses_the_os_case_table() {
        assert!(same_path_ci(Path::new(r"C:\Users\Ánh\Temp\"), Path::new(r"c:\users\ánh\temp")));
        // Rust: "ß".to_uppercase() == "SS"; NTFS coi hai tên này khác nhau.
        assert!(!same_path_ci(Path::new(r"C:\straße"), Path::new(r"C:\STRASSE")));
        assert!(!same_path_ci(Path::new(r"C:\a"), Path::new(r"C:\ab")));
    }

    #[test]
    fn hard_links_share_one_link_id() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("a"), 3);
        let solo = write_file(&root.join("solo"), 3);
        fs::hard_link(&f, root.join("b")).unwrap();
        let (_, ia) = lstat_with_link_id(&f).unwrap();
        let (_, ib) = lstat_with_link_id(&root.join("b")).unwrap();
        assert!(ia.is_some());
        assert_eq!(ia, ib);
        assert_eq!(lstat_with_link_id(&solo).unwrap().1, None);
        assert_eq!(lstat_with_link_id(&root).unwrap().1, None);
        assert!(is_vanished(&lstat_with_link_id(&root.join("khong-co")).unwrap_err()));
    }

    /// File đang chờ xóa (đã đặt cờ xóa kiểu cũ, còn handle khác mở): không được thành lỗi thô.
    #[test]
    fn delete_pending_file_is_classified_not_a_raw_error() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("pending.tmp"), 5);
        let keep = fs::OpenOptions::new().read(true).share_mode(7).open(&f).unwrap();
        let del = open_for_delete(&f).unwrap();
        let attrs = del.metadata().unwrap().file_attributes();
        dispose_legacy(&del, attrs).unwrap();
        drop(del);
        let guard = Guard::new(std::slice::from_ref(&root));
        let out = delete_path(&guard, &f, false);
        assert!(matches!(out, Ok(DeleteOutcome::Locked | DeleteOutcome::Vanished)), "{out:?}");
        // Trên máy test std lùi sang FindFirstFileW nên vẫn lấy được metadata; dù sao không được Err.
        assert!(delete_path(&guard, &f, true).is_ok());
        drop(keep);
        assert!(fs::symlink_metadata(&f).is_err());
    }

    /// Các mã lỗi `symlink_metadata` có thể trả trong `delete_path` (nay đi qua `classify`, vd file
    /// delete-pending ⇒ AccessDenied khi std không lùi được sang FindFirstFileW).
    #[test]
    fn metadata_errors_are_classified_like_the_handle_path() {
        let p = Path::new(r"C:\x");
        for (code, want) in [
            (2, DeleteOutcome::Vanished),
            (3, DeleteOutcome::Vanished),
            (303, DeleteOutcome::Vanished),
            (5, DeleteOutcome::Locked),
            (32, DeleteOutcome::Locked),
        ] {
            assert_eq!(classify(p, io::Error::from_raw_os_error(code)).unwrap(), want, "code {code}");
        }
        assert!(classify(p, io::Error::from_raw_os_error(123)).is_err());
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
