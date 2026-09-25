//! Xóa vào Thùng rác bằng `IFileOperation` (spec 3.3).
//!
//! Luật số một: **không bao giờ xóa vĩnh viễn một cách im lặng.** Các lớp chặn, theo thứ tự chạy:
//!
//! 1. Đường dẫn phải là đường tuyệt đối trên ổ có ký tự, không có tên mà Windows sẽ tự sửa hoặc hiểu khác
//!    (`bad_path`). Shell nhận đúng chuỗi đã kiểm.
//! 2. Ổ phải có Thùng rác đang bật (`ensure_recycle_bin`): chỉ ổ cố định (`DRIVE_FIXED`) — USB rời, ổ mạng, CD,
//!    RAM disk không có Thùng rác nên Windows sẽ xóa hẳn ⇒ `no_recycle_bin`; người dùng tắt Thùng rác cho ổ đó
//!    (`NukeOnDelete`) hoặc chính sách `NoRecycleFiles` ⇒ `recycle_disabled`.
//! 3. Nằm trong thư mục OneDrive ⇒ `onedrive` (người dùng quyết giữ chặn).
//! 4. **Khóa tổ tiên**: mở từng thư mục tổ tiên dưới gốc ổ bằng handle không đi theo reparse point, chia sẻ
//!    đọc/ghi nhưng **không** `FILE_SHARE_DELETE` ⇒ trong lúc giữ không ai đổi tên, xóa hay tráo được thư mục đó
//!    thành junction. Trên chính handle: không phải reparse point (`redirected_path`, hoặc `onedrive` nếu là
//!    thẻ cloud files), là thư mục, và `GetFinalPathNameByHandleW` trùng đường chữ. Giữ khóa tới khi
//!    `PerformOperations` xong — đóng cửa sổ TOCTOU phía tổ tiên.
//! 5. `guard` của người gọi (luật bảo vệ, Task 19) chạy SAU khi đã khóa và TRƯỚC `PerformOperations`.
//! 6. Cờ thao tác **không có** `FOF_NOCONFIRMATION` (xem `recycle_flags`): trường hợp còn lại mà Windows vẫn xóa
//!    hẳn (mục lớn hơn dung lượng Thùng rác) thì hộp «xóa vĩnh viễn?» của Windows luôn hiện; bấm Hủy ⇒ `aborted`.
use std::ffi::c_void;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Component, Path, PathBuf, Prefix};

use windows::core::HSTRING;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows::Win32::UI::Shell::{
    FileOperation, IFileOperation, IShellItem, SHCreateItemFromParsingName, FILEOPERATION_FLAGS, FOFX_EARLYFAILURE,
    FOFX_RECYCLEONDELETE, FOF_ALLOWUNDO, FOF_NOERRORUI, FOF_SILENT, FOF_WANTNUKEWARNING,
};
use windows_sys::Win32::Foundation::{ERROR_SUCCESS, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FileAttributeTagInfo, GetDriveTypeW, GetFileInformationByHandleEx, GetFinalPathNameByHandleW,
    GetVolumeNameForVolumeMountPointW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_LIST_DIRECTORY, FILE_NAME_NORMALIZED,
    FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, SYNCHRONIZE, VOLUME_NAME_DOS,
};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ,
    RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
};
use winfreeup_core::{CoreError, Result};

use super::volumes::{drive_kind, DriveKind};
use crate::util::{from_wide, wide};

/// Cờ cho `IFileOperation::SetOperationFlags`.
///
/// Theo tài liệu Microsoft (learn.microsoft.com, «IFileOperation::SetOperationFlags»):
/// - `FOF_NOCONFIRMATION` = «Respond with Yes to All for any dialog box that is displayed» — kể cả hộp hỏi xóa
///   vĩnh viễn; `FOF_WANTNUKEWARNING` chỉ «partially overrides FOF_NOCONFIRMATION» (không nói phần nào). Vì vậy
///   **bỏ hẳn `FOF_NOCONFIRMATION`** (lệch kế hoạch): Windows định xóa hẳn thì hộp xác nhận của Windows chắc
///   chắn hiện. Cái giá: ai đã bật «Display delete confirmation dialog» sẽ gặp thêm hộp hỏi cho Thùng rác —
///   chấp nhận được, còn hơn mất dữ liệu. Windows 10/11 mặc định tắt hộp đó.
/// - `FOF_WANTNUKEWARNING` giữ lại: «Send a warning if a file or folder is being destroyed during a delete
///   operation rather than recycled».
/// - `FOF_SILENT` chỉ tắt hộp tiến độ («Do not display a progress dialog box»), không tắt hộp xác nhận.
/// - `FOF_NOERRORUI` + `FOFX_EARLYFAILURE`: gặp lỗi thì dừng cả thao tác và trả lỗi. Không có
///   `FOFX_EARLYFAILURE` thì lỗi bị coi như bấm «Ignore» rồi làm tiếp — không biết đã xóa được gì.
/// - `FOF_ALLOWUNDO` + `FOFX_RECYCLEONDELETE`: cho vào Thùng rác thay vì xóa hẳn.
pub fn recycle_flags() -> u32 {
    (FOF_ALLOWUNDO | FOF_SILENT | FOF_NOERRORUI | FOF_WANTNUKEWARNING).0 | FOFX_RECYCLEONDELETE.0 | FOFX_EARLYFAILURE.0
}

fn code(c: &str) -> CoreError {
    CoreError::System(c.to_string())
}

/// HRESULT nghĩa là người dùng (hoặc shell thay mặt người dùng) đã hủy: `HRESULT_FROM_WIN32(ERROR_CANCELLED)`,
/// `COPYENGINE_E_USER_CANCELLED`, `E_ABORT`.
fn is_cancel(hr: u32) -> bool {
    matches!(hr, 0x800704C7 | 0x80270000 | 0x80004004)
}

/// Thẻ reparse của cloud files (OneDrive…): `IO_REPARSE_TAG_CLOUD` … `IO_REPARSE_TAG_CLOUD_F`.
fn is_cloud_tag(tag: u32) -> bool {
    tag & 0xFFFF_0FFF == 0x9000_001A
}

/// `path` bằng hoặc nằm dưới `folder` (không phân biệt hoa thường, so theo từng thành phần).
fn under_folder(path: &str, folder: &str) -> bool {
    let folder = folder.trim_end_matches('\\').to_lowercase();
    if folder.is_empty() {
        return false;
    }
    let path = path.to_lowercase();
    path == folder || path.strip_prefix(&folder).is_some_and(|rest| rest.starts_with('\\'))
}

/// Thư mục OneDrive của người dùng: `HKCU\Software\Microsoft\OneDrive\Accounts\*\UserFolder` (không đọc biến
/// môi trường `%OneDrive%`). Không đọc được ⇒ danh sách rỗng (lớp khóa tổ tiên vẫn bắt thẻ cloud files).
fn onedrive_folders() -> Vec<String> {
    let mut out = Vec::new();
    let sub = wide(r"Software\Microsoft\OneDrive\Accounts");
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: sub là chuỗi kết thúc NUL; key là biến cục bộ khả ghi.
    if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, sub.as_ptr(), 0, KEY_READ, &mut key) } != ERROR_SUCCESS {
        return out;
    }
    let value = wide("UserFolder");
    for i in 0.. {
        let mut name = [0u16; 256];
        let mut len = name.len() as u32;
        // SAFETY: key mở ở trên; name khả ghi đúng len ký tự; các tham số tùy chọn để null.
        let rc = unsafe {
            RegEnumKeyExW(
                key,
                i,
                name.as_mut_ptr(),
                &mut len,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if rc != ERROR_SUCCESS {
            break;
        }
        let mut buf = vec![0u16; 1024];
        let mut bytes = (buf.len() * 2) as u32;
        // SAFETY: name kết thúc NUL (RegEnumKeyExW ghi NUL); buf khả ghi đúng `bytes` byte.
        let rc = unsafe {
            RegGetValueW(
                key,
                name.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr() as *mut c_void,
                &mut bytes,
            )
        };
        if rc == ERROR_SUCCESS {
            let s = from_wide(&buf);
            if !s.is_empty() {
                out.push(s);
            }
        }
    }
    // SAFETY: key mở thành công ở trên, đóng đúng một lần.
    unsafe { RegCloseKey(key) };
    out
}

/// Tách gốc ổ `X:\` và các tên thành phần của một đường dẫn tuyệt đối trên ổ có ký tự.
///
/// Từ chối (`bad_path`): UNC, đường tương đối, gốc ổ; tên rỗng, `.`, `..`, kết thúc bằng `.` hoặc dấu cách
/// (Win32 tự cắt ⇒ trỏ sang mục khác), chứa `: / * ? " < > |` hoặc ký tự điều khiển (luồng dữ liệu phụ, ký tự
/// đại diện, hoặc chữ thường trong đường `\\?\`).
fn split_drive_path(path: &Path) -> Result<(String, Vec<String>)> {
    let mut comps = path.components();
    let root = match comps.next() {
        Some(Component::Prefix(p)) => match p.kind() {
            Prefix::Disk(l) | Prefix::VerbatimDisk(l) => format!("{}:\\", (l as char).to_ascii_uppercase()),
            _ => return Err(code("bad_path")),
        },
        _ => return Err(code("bad_path")),
    };
    if comps.next() != Some(Component::RootDir) {
        return Err(code("bad_path"));
    }
    let mut names = Vec::new();
    for c in comps {
        let Component::Normal(n) = c else {
            return Err(code("bad_path"));
        };
        let s = n.to_str().ok_or_else(|| code("bad_path"))?;
        let bad = s.is_empty()
            || s == "."
            || s == ".."
            || s.ends_with('.')
            || s.ends_with(' ')
            || s.chars().any(|ch| (ch as u32) < 0x20 || ":/*?\"<>|".contains(ch));
        if bad {
            return Err(code("bad_path"));
        }
        names.push(s.to_string());
    }
    if names.is_empty() {
        // Gốc ổ không bao giờ là thứ để xóa.
        return Err(code("bad_path"));
    }
    Ok((root, names))
}

/// Các thư mục tổ tiên đang bị giữ (handle không cho xóa/đổi tên). Thả khi drop.
#[derive(Debug)]
struct AncestorLock {
    _handles: Vec<OwnedHandle>,
}

/// Mở thư mục `p` không đi theo reparse point, chia sẻ đọc/ghi nhưng không chia sẻ xóa.
///
/// Phải xin quyền `FILE_LIST_DIRECTORY` (đọc dữ liệu): handle chỉ có `FILE_READ_ATTRIBUTES`/`SYNCHRONIZE` thì
/// Windows bỏ qua kiểm chia sẻ, và việc không cho `FILE_SHARE_DELETE` sẽ vô tác dụng.
fn open_dir_locked(p: &str) -> Result<OwnedHandle> {
    let w = wide(p);
    // SAFETY: w là chuỗi kết thúc NUL; không có thuộc tính bảo mật hay template.
    let h = unsafe {
        CreateFileW(
            w.as_ptr(),
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if h == INVALID_HANDLE_VALUE || h.is_null() {
        return Err(CoreError::Io { path: PathBuf::from(p), source: std::io::Error::last_os_error() });
    }
    // SAFETY: h là handle hợp lệ vừa mở, chưa ai sở hữu.
    Ok(unsafe { OwnedHandle::from_raw_handle(h) })
}

/// Đường thật của handle (`GetFinalPathNameByHandleW`), bỏ tiền tố `\\?\`.
fn final_path(h: &OwnedHandle, what: &str) -> Result<String> {
    let mut buf = vec![0u16; 512];
    loop {
        // SAFETY: handle hợp lệ; buf khả ghi đúng buf.len() ký tự.
        let n = unsafe {
            GetFinalPathNameByHandleW(h.as_raw_handle(), buf.as_mut_ptr(), buf.len() as u32, FILE_NAME_NORMALIZED | VOLUME_NAME_DOS)
        } as usize;
        if n == 0 {
            return Err(CoreError::Io { path: PathBuf::from(what), source: std::io::Error::last_os_error() });
        }
        if n < buf.len() {
            let s = String::from_utf16_lossy(&buf[..n]);
            return Ok(s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s));
        }
        buf.resize(n + 1, 0);
    }
}

/// Khóa mọi thư mục tổ tiên của `root\names…` (dưới gốc ổ, trên đích), từ trên xuống, và kiểm trên chính handle.
fn lock_ancestors(root: &str, names: &[String]) -> Result<AncestorLock> {
    let mut handles = Vec::new();
    let mut cur = root.trim_end_matches('\\').to_string();
    for n in &names[..names.len() - 1] {
        cur.push('\\');
        cur.push_str(n);
        let h = open_dir_locked(&cur)?;
        let mut info = FILE_ATTRIBUTE_TAG_INFO { FileAttributes: 0, ReparseTag: 0 };
        // SAFETY: handle hợp lệ; info là biến cục bộ khả ghi đúng kích thước truyền vào.
        let ok = unsafe {
            GetFileInformationByHandleEx(
                h.as_raw_handle(),
                FileAttributeTagInfo,
                &mut info as *mut _ as *mut c_void,
                std::mem::size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
            )
        };
        if ok == 0 {
            return Err(CoreError::Io { path: PathBuf::from(&cur), source: std::io::Error::last_os_error() });
        }
        if info.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(code(if is_cloud_tag(info.ReparseTag) { "onedrive" } else { "redirected_path" }));
        }
        if info.FileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
            return Err(code("bad_path"));
        }
        // Bắt ổ subst, tên 8.3 và mọi kiểu chuyển hướng khác.
        if final_path(&h, &cur)?.to_lowercase() != cur.to_lowercase() {
            return Err(code("redirected_path"));
        }
        handles.push(h);
    }
    Ok(AncestorLock { _handles: handles })
}

/// Kiểm đường dẫn, khóa tổ tiên, và trả đường dẫn dạng thường (`X:\a\b`) — đúng chuỗi sẽ giao cho shell.
/// Bản thân đích là link thì được: shell cho chính link vào Thùng rác, không đi theo nó.
fn prepare(path: &Path) -> Result<(PathBuf, AncestorLock)> {
    let (root, names) = split_drive_path(path)?;
    let full = format!("{root}{}", names.join("\\"));
    if onedrive_folders().iter().any(|f| under_folder(&full, f)) {
        return Err(code("onedrive"));
    }
    let lock = lock_ancestors(&root, &names)?;
    let target = PathBuf::from(full);
    std::fs::symlink_metadata(&target).map_err(|e| CoreError::Io { path: target.clone(), source: e })?;
    Ok((target, lock))
}

/// Đọc một giá trị DWORD trong registry; không có khóa/giá trị ⇒ `None`.
fn reg_dword(hive: HKEY, subkey: &str, value: &str) -> Option<u32> {
    let (k, v) = (wide(subkey), wide(value));
    let mut data = 0u32;
    let mut len = std::mem::size_of::<u32>() as u32;
    // SAFETY: k, v là chuỗi kết thúc NUL; data/len là biến cục bộ khả ghi, len đúng kích thước data.
    let rc = unsafe {
        RegGetValueW(
            hive,
            k.as_ptr(),
            v.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut data as *mut u32 as *mut c_void,
            &mut len,
        )
    };
    (rc == ERROR_SUCCESS).then_some(data)
}

/// Ổ `root` (`X:\`) có Thùng rác đang bật không. Chỉ đọc hệ thống.
fn ensure_recycle_bin(root: &str) -> Result<()> {
    let w = wide(root);
    // SAFETY: w là chuỗi kết thúc NUL còn sống.
    if drive_kind(unsafe { GetDriveTypeW(w.as_ptr()) }) != DriveKind::Fixed {
        return Err(code("no_recycle_bin"));
    }
    const POLICY: &str = r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer";
    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        if reg_dword(hive, POLICY, "NoRecycleFiles").unwrap_or(0) != 0 {
            return Err(code("recycle_disabled"));
        }
    }
    let mut buf = [0u16; 64];
    // SAFETY: w là chuỗi kết thúc NUL; buf khả ghi đúng độ dài truyền vào (tài liệu: 50 ký tự là đủ).
    if unsafe { GetVolumeNameForVolumeMountPointW(w.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) } == 0 {
        return Err(CoreError::System(format!(
            "GetVolumeNameForVolumeMountPointW({root}): {}",
            std::io::Error::last_os_error()
        )));
    }
    // "\\?\Volume{GUID}\" ⇒ "{GUID}"
    let name = from_wide(&buf);
    let guid = match (name.find('{'), name.find('}')) {
        (Some(a), Some(b)) if b > a => &name[a..=b],
        _ => return Err(CoreError::System(format!("unexpected volume name: {name}"))),
    };
    let key = format!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume\{guid}");
    if reg_dword(HKEY_CURRENT_USER, &key, "NukeOnDelete").unwrap_or(0) != 0 {
        return Err(code("recycle_disabled"));
    }
    Ok(())
}

fn hr_error(e: &windows::core::Error) -> CoreError {
    CoreError::System(format!("{} (0x{:08X})", e.message(), e.code().0 as u32))
}

/// Cho `path` vào Thùng rác. Chạy shell trên một luồng STA riêng, chờ xong mới trả.
///
/// - `owner`: HWND cửa sổ app (dạng số) để hộp hỏi của Windows nằm trên cửa sổ app; `None` ⇒ không có cha.
/// - `guard`: kiểm của người gọi (luật bảo vệ), chạy khi tổ tiên đã bị khóa và trước `PerformOperations`;
///   trả `Err` ⇒ không xóa gì, lỗi được trả nguyên.
///
/// Mã lỗi (giao diện dịch): `bad_path`, `redirected_path`, `onedrive`, `no_recycle_bin`, `recycle_disabled`,
/// `aborted` (người dùng hủy — ví dụ bấm Hủy ở hộp xóa vĩnh viễn); còn lại là thông điệp hệ thống nguyên văn.
pub fn recycle(path: &Path, owner: Option<isize>, guard: &dyn Fn(&Path) -> Result<()>) -> Result<()> {
    let (root, _) = split_drive_path(path)?;
    ensure_recycle_bin(&root)?;
    let (target, lock) = prepare(path)?;
    guard(&target)?;

    let p = target.clone();
    let joined = std::thread::spawn(move || -> Result<()> {
        // SAFETY: COM khởi tạo STA trên chính luồng này và gỡ ở cuối; mọi đối tượng COM được thả trong closure,
        // trước CoUninitialize.
        unsafe {
            let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
            if hr.is_err() {
                return Err(CoreError::System(format!("CoInitializeEx: {} (0x{:08X})", hr.message(), hr.0 as u32)));
            }
            let result = (|| -> Result<bool> {
                let op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(|e| hr_error(&e))?;
                op.SetOperationFlags(FILEOPERATION_FLAGS(recycle_flags())).map_err(|e| hr_error(&e))?;
                if let Some(h) = owner {
                    op.SetOwnerWindow(HWND(h as *mut c_void)).map_err(|e| hr_error(&e))?;
                }
                let item: IShellItem =
                    SHCreateItemFromParsingName(&HSTRING::from(p.as_path()), None).map_err(|e| hr_error(&e))?;
                op.DeleteItem(&item, None).map_err(|e| hr_error(&e))?;
                if let Err(e) = op.PerformOperations() {
                    return Err(if is_cancel(e.code().0 as u32) { code("aborted") } else { hr_error(&e) });
                }
                Ok(op.GetAnyOperationsAborted().map_err(|e| hr_error(&e))?.as_bool())
            })();
            CoUninitialize();
            if result? {
                Err(code("aborted"))
            } else {
                Ok(())
            }
        }
    })
    .join()
    .map_err(|_| CoreError::System("recycle thread panicked".into()));
    drop(lock);
    joined??;
    if std::fs::symlink_metadata(&target).is_ok() {
        return Err(CoreError::System(format!("still exists after recycle: {}", target.display())));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::Shell::FOF_NOCONFIRMATION;

    fn err_code<T: std::fmt::Debug>(r: Result<T>) -> String {
        match r {
            Err(CoreError::System(s)) => s,
            other => panic!("mong lỗi mã, được {other:?}"),
        }
    }

    /// Thư mục tạm theo đường thật (không tên 8.3, không `\\?\`) để khớp `GetFinalPathNameByHandleW`.
    fn real_tempdir() -> (tempfile::TempDir, PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let real = std::fs::canonicalize(t.path()).unwrap();
        let s = real.to_string_lossy().to_string();
        let base = PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s));
        (t, base)
    }

    fn no_guard(_: &Path) -> Result<()> {
        Ok(())
    }

    #[test]
    fn flags_recycle_and_still_warn_before_permanent_delete() {
        let f = recycle_flags();
        assert_ne!(f & FOFX_RECYCLEONDELETE.0, 0);
        assert_ne!(f & FOF_ALLOWUNDO.0, 0);
        assert_ne!(f & FOF_WANTNUKEWARNING.0, 0);
    }

    #[test]
    fn flags_never_auto_answer_yes_to_the_permanent_delete_prompt() {
        let f = recycle_flags();
        assert_eq!(f & FOF_NOCONFIRMATION.0, 0);
        assert_ne!(f & FOFX_EARLYFAILURE.0, 0);
        assert_ne!(f & FOF_NOERRORUI.0, 0);
    }

    #[test]
    fn user_cancel_hresults_map_to_aborted() {
        assert!(is_cancel(0x800704C7)); // HRESULT_FROM_WIN32(ERROR_CANCELLED)
        assert!(is_cancel(0x80270000)); // COPYENGINE_E_USER_CANCELLED
        assert!(is_cancel(0x80004004)); // E_ABORT
        assert!(!is_cancel(0x80070005)); // E_ACCESSDENIED
        assert!(!is_cancel(0));
    }

    #[test]
    fn missing_path_is_an_error_not_a_panic() {
        let t = tempfile::tempdir().unwrap();
        assert!(recycle(&t.path().join("khong-co.txt"), None, &no_guard).is_err());
    }

    #[test]
    fn plain_path_passes_the_check() {
        let (_t, base) = real_tempdir();
        let f = base.join("a").join("f.txt");
        std::fs::create_dir_all(base.join("a")).unwrap();
        std::fs::write(&f, "x").unwrap();
        let (p, _lock) = prepare(&f).unwrap();
        assert_eq!(p, f);
    }

    #[test]
    fn junction_ancestor_is_refused() {
        let (_t, base) = real_tempdir();
        std::fs::create_dir_all(base.join("that")).unwrap();
        std::fs::write(base.join("that").join("f.txt"), "x").unwrap();
        junction::create(base.join("that"), base.join("link")).unwrap();
        assert_eq!(err_code(prepare(&base.join("link").join("f.txt"))), "redirected_path");
        // Tiền tố \\?\ cũng không lách được.
        let verbatim = PathBuf::from(format!(r"\\?\{}", base.join("link").join("f.txt").display()));
        assert_eq!(err_code(prepare(&verbatim)), "redirected_path");
        // Bản thân junction (không phải tổ tiên) thì được — shell cho chính link vào Thùng rác.
        assert_eq!(prepare(&base.join("link")).unwrap().0, base.join("link"));
    }

    #[test]
    fn dot_dot_unc_relative_and_drive_root_are_refused() {
        let (_t, base) = real_tempdir();
        std::fs::create_dir_all(base.join("a")).unwrap();
        let dd = PathBuf::from(format!(r"{}\a\..\a", base.display()));
        assert_eq!(err_code(prepare(&dd)), "bad_path");
        let vdd = PathBuf::from(format!(r"\\?\{}\a\..\a", base.display()));
        assert_eq!(err_code(prepare(&vdd)), "bad_path");
        assert_eq!(err_code(prepare(Path::new(r"\\server\share\x"))), "bad_path");
        assert_eq!(err_code(prepare(Path::new(r"a\b"))), "bad_path");
        let root = crate::util::system_drive_root().unwrap();
        assert_eq!(err_code(prepare(&root)), "bad_path");
    }

    #[test]
    fn names_windows_would_rewrite_or_misread_are_refused() {
        let (_t, base) = real_tempdir();
        for name in ["a.", "a ", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b", "a\u{1}b"] {
            let p = PathBuf::from(format!(r"{}\{}", base.display(), name));
            assert_eq!(err_code(prepare(&p)), "bad_path", "{name:?}");
        }
        // '/' chỉ là chữ trong đường \\?\ — cũng phải bị từ chối.
        let v = PathBuf::from(format!(r"\\?\{}\a/b", base.display()));
        assert_eq!(err_code(prepare(&v)), "bad_path");
    }

    #[test]
    fn held_lock_blocks_renaming_ancestors_until_dropped() {
        let (_t, base) = real_tempdir();
        let f = base.join("a").join("b").join("f.txt");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, "x").unwrap();
        let (_, lock) = prepare(&f).unwrap();
        assert!(std::fs::rename(base.join("a"), base.join("a2")).is_err());
        assert!(std::fs::rename(base.join("a").join("b"), base.join("a").join("b2")).is_err());
        assert!(std::fs::remove_dir(base.join("a").join("b")).is_err());
        drop(lock);
        std::fs::rename(base.join("a").join("b"), base.join("a").join("b2")).unwrap();
    }

    #[test]
    fn guard_refusal_leaves_the_file_untouched_and_runs_while_locked() {
        let (_t, base) = real_tempdir();
        let f = base.join("a").join("f.txt");
        std::fs::create_dir_all(base.join("a")).unwrap();
        std::fs::write(&f, "x").unwrap();
        let called = std::cell::Cell::new(false);
        let guard = |p: &Path| -> Result<()> {
            called.set(true);
            assert_eq!(p, f.as_path());
            assert!(std::fs::rename(base.join("a"), base.join("a2")).is_err(), "tổ tiên phải đang bị giữ");
            Err(code("protected"))
        };
        let r = err_code(recycle(&f, None, &guard));
        // Máy tắt Thùng rác ở ổ tạm thì dừng sớm hơn — file vẫn phải còn nguyên.
        assert!(r == "protected" || r == "recycle_disabled", "{r}");
        if r == "protected" {
            assert!(called.get());
        }
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "x");
    }

    #[test]
    fn cloud_file_reparse_tags_are_recognized() {
        assert!(is_cloud_tag(0x9000001A)); // IO_REPARSE_TAG_CLOUD
        assert!(is_cloud_tag(0x9000101A)); // IO_REPARSE_TAG_CLOUD_1
        assert!(is_cloud_tag(0x9000F01A)); // IO_REPARSE_TAG_CLOUD_F
        assert!(!is_cloud_tag(0xA0000003)); // mount point / junction
        assert!(!is_cloud_tag(0xA000000C)); // symlink
        assert!(!is_cloud_tag(0x80000013)); // dedup
    }

    #[test]
    fn paths_under_a_onedrive_user_folder_are_detected() {
        let folders = [r"C:\Users\an\OneDrive".to_string(), r"D:\OneDrive - Cty\".to_string()];
        let hit = |p: &str| folders.iter().any(|f| under_folder(p, f));
        assert!(hit(r"C:\Users\an\OneDrive\x.txt"));
        assert!(hit(r"c:\users\AN\onedrive\sub\x.txt"));
        assert!(hit(r"C:\Users\an\OneDrive"));
        assert!(hit(r"D:\OneDrive - Cty\a"));
        assert!(!hit(r"C:\Users\an\OneDriveOld\x.txt"));
        assert!(!hit(r"C:\Users\an\Downloads\x.txt"));
        assert!(!under_folder(r"C:\x", ""));
    }

    #[test]
    fn system_drive_has_a_recycle_bin_unmapped_letters_do_not() {
        let root = crate::util::system_drive_root().unwrap();
        match ensure_recycle_bin(root.to_str().unwrap()) {
            Ok(()) => {}
            Err(CoreError::System(s)) if s == "recycle_disabled" => {}
            other => panic!("{other:?}"),
        }
        // Ký tự ổ chưa gắn: GetDriveTypeW trả DRIVE_NO_ROOT_DIR ⇒ không phải ổ cố định.
        // SAFETY: không tham số.
        let mask = unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() };
        let i = (3..26u8).rev().find(|i| mask & (1 << i) == 0).expect("còn ký tự ổ trống");
        assert_eq!(err_code(ensure_recycle_bin(&format!("{}:\\", (b'A' + i) as char))), "no_recycle_bin");
    }

    /// Chạm Thùng rác thật của máy — chạy tay: `cargo test -- --ignored recycle_moves`.
    #[test]
    #[ignore]
    fn recycle_moves_a_folder_to_the_recycle_bin() {
        let (_t, base) = real_tempdir();
        let dir = base.join("WinFreeUp-thu-thung-rac");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "x").unwrap();
        recycle(&dir, None, &no_guard).unwrap();
        assert!(!dir.exists());
    }

    /// Chạm Thùng rác thật — chạy tay: `cargo test -- --ignored recycle_moves_while_ancestors_locked`.
    /// Chứng minh shell vẫn cho đích vào Thùng rác khi mọi tổ tiên đang bị giữ không cho xóa/đổi tên.
    #[test]
    #[ignore]
    fn recycle_moves_while_ancestors_locked() {
        let (_t, base) = real_tempdir();
        let dir = base.join("khoa").join("WinFreeUp-thu-khoa-to-tien");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "x").unwrap();
        let guard = |_: &Path| -> Result<()> {
            assert!(std::fs::rename(base.join("khoa"), base.join("khoa2")).is_err(), "tổ tiên phải đang bị giữ");
            Ok(())
        };
        recycle(&dir, None, &guard).unwrap();
        assert!(!dir.exists());
        assert!(base.join("khoa").exists());
    }
}
