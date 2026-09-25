//! Xóa vào Thùng rác bằng `IFileOperation` (spec 3.3).
//!
//! Luật số một: **không bao giờ xóa vĩnh viễn một cách im lặng.** Ba lớp chặn:
//!
//! 1. Trước thao tác, ổ phải có Thùng rác đang bật (`ensure_recycle_bin`): chỉ ổ cố định (`DRIVE_FIXED`) — USB
//!    rời, ổ mạng, CD, RAM disk không có Thùng rác nên Windows sẽ xóa hẳn ⇒ từ chối `no_recycle_bin`; người dùng
//!    tắt Thùng rác cho ổ đó (`NukeOnDelete`) hoặc chính sách `NoRecycleFiles` ⇒ từ chối `recycle_disabled`.
//! 2. Cờ thao tác **không có** `FOF_NOCONFIRMATION` (xem `recycle_flags`): trường hợp còn lại mà Windows vẫn xóa
//!    hẳn (mục lớn hơn dung lượng Thùng rác) thì hộp «xóa vĩnh viễn?» của Windows luôn hiện; bấm Hủy ⇒ `aborted`.
//! 3. Đường dẫn không được đi qua reparse point (junction, symlink, điểm gắn ổ…) ở bất kỳ thư mục tổ tiên nào
//!    dưới gốc ổ, và thư mục cha sau `canonicalize` phải trùng đường chữ ⇒ không thì `redirected_path`.
//!    Kiểm lại ngay trước `PerformOperations` để thu hẹp cửa sổ TOCTOU.
use std::ffi::OsStr;
use std::os::windows::fs::MetadataExt;
use std::path::{Component, Path, PathBuf, Prefix};

use windows::core::HSTRING;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows::Win32::UI::Shell::{
    FileOperation, IFileOperation, IShellItem, SHCreateItemFromParsingName, FILEOPERATION_FLAGS, FOFX_EARLYFAILURE,
    FOFX_RECYCLEONDELETE, FOF_ALLOWUNDO, FOF_NOERRORUI, FOF_SILENT, FOF_WANTNUKEWARNING,
};
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetVolumeNameForVolumeMountPointW, FILE_ATTRIBUTE_REPARSE_POINT};
use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD};
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

/// Tách gốc ổ `X:\` và các thành phần tên của một đường dẫn tuyệt đối trên ổ có ký tự.
/// Từ chối UNC, đường tương đối, gốc ổ, `.`/`..`, thành phần rỗng hoặc có `/` (đường `\\?\` không chuẩn hóa).
fn split_drive_path(path: &Path) -> Result<(String, Vec<&OsStr>)> {
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
        match c {
            Component::Normal(n) => {
                let s = n.to_string_lossy();
                if s.is_empty() || s == "." || s == ".." || s.contains('/') {
                    return Err(code("bad_path"));
                }
                names.push(n);
            }
            _ => return Err(code("bad_path")),
        }
    }
    if names.is_empty() {
        // Gốc ổ không bao giờ là thứ để xóa.
        return Err(code("bad_path"));
    }
    Ok((root, names))
}

/// Kiểm `path` và trả đường dẫn dạng thường (`X:\a\b`) để giao cho shell.
///
/// - `path` phải còn tồn tại. Bản thân nó là link thì được: shell cho chính link vào Thùng rác, không đi theo.
/// - Mọi thư mục tổ tiên dưới gốc ổ không được là reparse point — `symlink_metadata` (không đi theo link).
///   Luật chặt: mọi loại reparse point, kể cả thư mục OneDrive (cloud files).
/// - Thư mục cha sau `canonicalize` (GetFinalPathNameByHandle) phải trùng đường chữ (không phân biệt hoa
///   thường) — bắt nốt ổ `subst`, tên 8.3 và mọi kiểu chuyển hướng khác.
fn check_target(path: &Path) -> Result<PathBuf> {
    let (root, names) = split_drive_path(path)?;
    let mut cur = PathBuf::from(&root);
    for (i, n) in names.iter().enumerate() {
        cur.push(n);
        let md = std::fs::symlink_metadata(&cur).map_err(|e| CoreError::Io { path: cur.clone(), source: e })?;
        let last = i + 1 == names.len();
        if !last && md.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(code("redirected_path"));
        }
    }
    let parent = cur.parent().ok_or_else(|| code("bad_path"))?;
    let real = std::fs::canonicalize(parent).map_err(|e| CoreError::Io { path: parent.to_path_buf(), source: e })?;
    let real = real.to_string_lossy();
    let real = real.strip_prefix(r"\\?\").unwrap_or(&real);
    if real.to_lowercase() != parent.to_string_lossy().to_lowercase() {
        return Err(code("redirected_path"));
    }
    Ok(cur)
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
            &mut data as *mut u32 as *mut _,
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

/// Cho `path` vào Thùng rác. Chạy trên một luồng STA riêng (yêu cầu của shell), chờ xong mới trả.
///
/// Mã lỗi (giao diện dịch): `bad_path`, `redirected_path`, `no_recycle_bin`, `recycle_disabled`, `aborted`
/// (người dùng bấm Hủy ở hộp xóa vĩnh viễn — không có gì bị xóa); còn lại là thông điệp hệ thống nguyên văn.
pub fn recycle(path: &Path) -> Result<()> {
    let target = check_target(path)?;
    let (root, _) = split_drive_path(&target)?;
    ensure_recycle_bin(&root)?;

    let p = target.clone();
    let joined = std::thread::spawn(move || -> Result<()> {
        // SAFETY: COM khởi tạo STA trên chính luồng này và gỡ ở cuối; mọi đối tượng COM được thả trong closure,
        // trước CoUninitialize.
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE)
                .ok()
                .map_err(|e| CoreError::System(e.message()))?;
            let result = (|| -> Result<bool> {
                let sys =
                    |e: windows::core::Error| CoreError::System(format!("{} (0x{:08X})", e.message(), e.code().0 as u32));
                let op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(sys)?;
                op.SetOperationFlags(FILEOPERATION_FLAGS(recycle_flags())).map_err(sys)?;
                let item: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(p.as_path()), None).map_err(sys)?;
                op.DeleteItem(&item, None).map_err(sys)?;
                // Kiểm lại sát trước thao tác: thư mục tổ tiên có thể vừa bị thay bằng junction.
                check_target(&p)?;
                op.PerformOperations().map_err(sys)?;
                Ok(op.GetAnyOperationsAborted().map_err(sys)?.as_bool())
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
    .map_err(|_| CoreError::System("recycle thread panicked".into()))?;
    joined?;
    if std::fs::symlink_metadata(&target).is_ok() {
        return Err(CoreError::System(format!("still exists after recycle: {}", target.display())));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::Shell::FOF_NOCONFIRMATION;

    fn err_code(r: Result<PathBuf>) -> String {
        match r {
            Err(CoreError::System(s)) => s,
            other => panic!("mong lỗi mã, được {other:?}"),
        }
    }

    /// Thư mục tạm theo đường thật (không tên 8.3, không `\\?\`) để so với `canonicalize` ổn định.
    fn real_tempdir() -> (tempfile::TempDir, PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let real = std::fs::canonicalize(t.path()).unwrap();
        let s = real.to_string_lossy().to_string();
        let base = PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s));
        (t, base)
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
    fn missing_path_is_an_error_not_a_panic() {
        let t = tempfile::tempdir().unwrap();
        assert!(recycle(&t.path().join("khong-co.txt")).is_err());
    }

    #[test]
    fn plain_path_passes_the_check() {
        let (_t, base) = real_tempdir();
        let f = base.join("a").join("f.txt");
        std::fs::create_dir_all(base.join("a")).unwrap();
        std::fs::write(&f, "x").unwrap();
        assert_eq!(check_target(&f).unwrap(), f);
    }

    #[test]
    fn junction_ancestor_is_refused() {
        let (_t, base) = real_tempdir();
        std::fs::create_dir_all(base.join("that")).unwrap();
        std::fs::write(base.join("that").join("f.txt"), "x").unwrap();
        junction::create(base.join("that"), base.join("link")).unwrap();
        assert_eq!(err_code(check_target(&base.join("link").join("f.txt"))), "redirected_path");
        // Tiền tố \\?\ cũng không lách được.
        let verbatim = PathBuf::from(format!(r"\\?\{}", base.join("link").join("f.txt").display()));
        assert_eq!(err_code(check_target(&verbatim)), "redirected_path");
        // Bản thân junction (không phải tổ tiên) thì được — shell cho chính link vào Thùng rác.
        assert_eq!(check_target(&base.join("link")).unwrap(), base.join("link"));
    }

    #[test]
    fn dot_dot_unc_relative_and_drive_root_are_refused() {
        let (_t, base) = real_tempdir();
        std::fs::create_dir_all(base.join("a")).unwrap();
        let dd = PathBuf::from(format!(r"{}\a\..\a", base.display()));
        assert_eq!(err_code(check_target(&dd)), "bad_path");
        let vdd = PathBuf::from(format!(r"\\?\{}\a\..\a", base.display()));
        assert_eq!(err_code(check_target(&vdd)), "bad_path");
        assert_eq!(err_code(check_target(Path::new(r"\\server\share\x"))), "bad_path");
        assert_eq!(err_code(check_target(Path::new(r"a\b"))), "bad_path");
        let root = crate::util::system_drive_root().unwrap();
        assert_eq!(err_code(check_target(&root)), "bad_path");
    }

    #[test]
    fn system_drive_has_a_recycle_bin_unmapped_letters_do_not() {
        let root = crate::util::system_drive_root().unwrap();
        ensure_recycle_bin(root.to_str().unwrap()).unwrap();
        // Ký tự ổ chưa gắn: GetDriveTypeW trả DRIVE_NO_ROOT_DIR ⇒ không phải ổ cố định.
        // SAFETY: không tham số.
        let mask = unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() };
        let i = (3..26u8).rev().find(|i| mask & (1 << i) == 0).expect("còn ký tự ổ trống");
        match ensure_recycle_bin(&format!("{}:\\", (b'A' + i) as char)) {
            Err(CoreError::System(s)) => assert_eq!(s, "no_recycle_bin"),
            other => panic!("{other:?}"),
        }
    }

    /// Chạm Thùng rác thật của máy — chạy tay: `cargo test -- --ignored recycle_moves`.
    #[test]
    #[ignore]
    fn recycle_moves_a_folder_to_the_recycle_bin() {
        let (_t, base) = real_tempdir();
        let dir = base.join("WinFreeUp-thu-thung-rac");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "x").unwrap();
        recycle(&dir).unwrap();
        assert!(!dir.exists());
    }
}
