//! Tiện ích nhỏ dùng chung: chuỗi UTF-16 cho Win32, đổi thời gian FILETIME, mở Explorer, và đường dẫn
//! hệ thống lấy qua API.
//!
//! App chạy quyền Admin nhưng người dùng thường đặt được biến môi trường của chính mình (HKCU\Environment):
//! `SystemRoot`, `windir`, `ProgramData`, `ProgramFiles`, `USERPROFILE`, `APPDATA`, `TEMP`… đều có thể bị
//! trỏ đi nơi khác. Vì vậy mọi đường dẫn hệ thống trong crate này lấy qua API ở đây, không đọc biến môi trường.
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf, Prefix};

use windows::Win32::System::Com::CoTaskMemFree;
use windows_sys::core::{GUID, PWSTR};
use windows_sys::Win32::Foundation::{
    CloseHandle, LocalFree, ERROR_FILE_NOT_FOUND, ERROR_INSUFFICIENT_BUFFER, ERROR_MORE_DATA, HANDLE,
};
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
use windows_sys::Win32::System::Registry::{
    RegGetValueW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_NOEXPAND, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
};
use windows_sys::Win32::System::SystemInformation::{GetSystemDirectoryW, GetSystemWindowsDirectoryW};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::UI::Shell::{
    SHGetKnownFolderPath, FOLDERID_ProgramFiles, FOLDERID_ProgramFilesX86, FOLDERID_Startup, KF_FLAG_DEFAULT,
};

/// Chuỗi UTF-16 kết thúc bằng 0 cho hàm Win32 `...W`.
pub fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

/// Đọc chuỗi UTF-16 tới ký tự 0 đầu tiên (hoặc hết mảng).
pub fn from_wide(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// Số khoảng 100 ns từ 1601-01-01 tới 1970-01-01.
const EPOCH_DIFF_100NS: i64 = 116_444_736_000_000_000;

/// FILETIME (100 ns từ 1601) ⇒ giây Unix, kẹp vào khoảng của u32.
pub fn filetime_to_unix(ft: i64) -> u32 {
    if ft <= EPOCH_DIFF_100NS {
        return 0;
    }
    ((ft - EPOCH_DIFF_100NS) / 10_000_000).min(u32::MAX as i64) as u32
}

/// Mở Explorer và chọn sẵn mục (`explorer.exe /select,<path>`). Lỗi là thông điệp nguyên văn.
pub fn reveal_in_explorer(path: &Path) -> Result<(), String> {
    let mut arg = OsString::from("/select,");
    arg.push(path.as_os_str());
    std::process::Command::new("explorer.exe").arg(arg).spawn().map(|_| ()).map_err(|e| format!("explorer.exe: {e}"))
}

/// Đọc một thư mục hệ thống qua API kiểu `GetSystemDirectoryW` (không qua biến môi trường).
fn api_dir(f: unsafe extern "system" fn(PWSTR, u32) -> u32, what: &str) -> Result<PathBuf, String> {
    let mut buf = vec![0u16; 260];
    loop {
        // SAFETY: buf có đúng buf.len() phần tử u16 khả ghi; API ghi không quá độ dài được báo.
        let n = unsafe { f(buf.as_mut_ptr(), buf.len() as u32) } as usize;
        if n == 0 {
            return Err(format!("{what}: {}", std::io::Error::last_os_error()));
        }
        if n < buf.len() {
            buf.truncate(n);
            return Ok(PathBuf::from(OsString::from_wide(&buf)));
        }
        // Bộ đệm thiếu: n là kích thước cần, gồm cả NUL.
        buf.resize(n, 0);
    }
}

/// Thư mục Windows dùng chung (`C:\Windows`), không phải bản riêng từng phiên Terminal Server.
pub fn windows_dir() -> Result<PathBuf, String> {
    api_dir(GetSystemWindowsDirectoryW, "GetSystemWindowsDirectoryW")
}

/// `C:\Windows\System32`.
pub fn system_dir() -> Result<PathBuf, String> {
    api_dir(GetSystemDirectoryW, "GetSystemDirectoryW")
}

/// Gốc ổ chứa Windows, dạng `C:\`.
pub fn system_drive_root() -> Result<PathBuf, String> {
    let windir = windows_dir()?;
    match windir.components().next() {
        Some(Component::Prefix(p)) => match p.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => Ok(PathBuf::from(format!("{}:\\", letter as char))),
            _ => Err(format!("windows dir is not on a drive letter: {}", windir.display())),
        },
        _ => Err(format!("windows dir has no drive prefix: {}", windir.display())),
    }
}

/// Thư mục đặc biệt qua `SHGetKnownFolderPath`.
///
/// CHÚ Ý: Shell mở rộng đường dẫn của nhiều thư mục (ProgramData, Startup, Common Startup…) bằng biến môi
/// trường của tiến trình — đã đo: `ProgramData`/`USERPROFILE` giả ⇒ kết quả đổi theo. Chỉ dùng hàm này cho
/// thư mục mà test `system_paths_come_from_the_api_not_from_environment_variables` đã chứng minh không phụ
/// thuộc biến môi trường (hiện là Program Files, Program Files (x86)).
pub(crate) fn known_folder(id: &GUID) -> Result<PathBuf, String> {
    let mut p: PWSTR = std::ptr::null_mut();
    // SAFETY: id trỏ vào GUID còn sống; token rỗng = người dùng của tiến trình; p là biến cục bộ khả ghi.
    let hr = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT as u32, std::ptr::null_mut(), &mut p) };
    let res = if hr < 0 {
        Err(format!(
            "SHGetKnownFolderPath({:08X}-{:04X}-{:04X}-…): {}",
            id.data1,
            id.data2,
            id.data3,
            windows::core::Error::from_hresult(windows::core::HRESULT(hr)).message()
        ))
    } else if p.is_null() {
        Err("SHGetKnownFolderPath: null path".into())
    } else {
        let mut len = 0;
        // SAFETY: khi thành công p là chuỗi kết thúc NUL do API cấp phát, còn sống tới lúc CoTaskMemFree.
        while unsafe { *p.add(len) } != 0 {
            len += 1;
        }
        // SAFETY: p có đúng len ký tự trước NUL.
        Ok(PathBuf::from(OsString::from_wide(unsafe { std::slice::from_raw_parts(p, len) })))
    };
    // Tài liệu API: người gọi giải phóng *ppszPath bằng CoTaskMemFree dù thành công hay không (null thì vô hại).
    // SAFETY: p do SHGetKnownFolderPath cấp phát bằng bộ cấp phát COM (hoặc null), giải phóng đúng một lần.
    unsafe { CoTaskMemFree(Some(p as *const _)) };
    res
}

/// `C:\Program Files`.
pub fn program_files() -> Result<PathBuf, String> {
    known_folder(&FOLDERID_ProgramFiles)
}

/// `C:\Program Files (x86)`.
pub fn program_files_x86() -> Result<PathBuf, String> {
    known_folder(&FOLDERID_ProgramFilesX86)
}

const PROFILE_LIST_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList";
const SHELL_FOLDERS_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders";
const STARTUP_REL: &str = r"Microsoft\Windows\Start Menu\Programs\Startup";

/// `C:\ProgramData`, theo HKLM `ProfileList\ProgramData` (thường `%SystemDrive%\ProgramData`), ổ lấy qua API.
pub fn program_data() -> Result<PathBuf, String> {
    let raw = read_reg_string(HKEY_LOCAL_MACHINE, "HKLM", PROFILE_LIST_KEY, "ProgramData")?;
    expand_vars(&raw, &[("SystemDrive", &system_drive()?)])
}

/// Hồ sơ của người dùng sở hữu token tiến trình (`C:\Users\<tên>`), theo HKLM `ProfileList\<SID>`.
/// Nâng quyền bằng một tài khoản Admin khác thì đây là hồ sơ của tài khoản đó.
pub fn user_profile() -> Result<PathBuf, String> {
    let key = format!(r"{PROFILE_LIST_KEY}\{}", current_user_sid()?);
    let raw = read_reg_string(HKEY_LOCAL_MACHINE, "HKLM", &key, "ProfileImagePath")?;
    expand_vars(&raw, &[("SystemDrive", &system_drive()?)])
}

/// Thư mục Startup của người dùng, theo HKCU `User Shell Folders\Startup` (người dùng đổi được chỗ, đó là
/// thư mục của chính họ); chưa đặt thì là chỗ mặc định trong hồ sơ. Giá trị dùng biến ngoài nhóm hồ sơ
/// (vd `%OneDrive%`) ⇒ lùi về `SHGetKnownFolderPath(FOLDERID_Startup)`: chấp nhận được vì đây là thư mục của
/// chính người dùng và chỉ dùng để liệt kê/bật tắt khởi động, không xóa gì trong đó.
pub fn user_startup() -> Result<PathBuf, String> {
    let raw = read_reg_opt(HKEY_CURRENT_USER, "HKCU", SHELL_FOLDERS_KEY, "Startup")?;
    match user_startup_from(raw.as_deref(), &user_profile()?, &system_drive()?) {
        Some(p) => Ok(p),
        None => known_folder(&FOLDERID_Startup),
    }
}

/// Hàm thuần của `user_startup`: mở rộng giá trị registry bằng các biến suy ra từ hồ sơ (lấy qua registry theo
/// SID). `None` nếu gặp biến khác hoặc chuỗi hỏng — người gọi lùi về Shell.
fn user_startup_from(raw: Option<&str>, profile: &Path, system_drive: &str) -> Option<PathBuf> {
    let appdata = profile.join(r"AppData\Roaming");
    let Some(raw) = raw else {
        return Some(appdata.join(STARTUP_REL));
    };
    let profile_s = profile.to_string_lossy();
    let appdata_s = appdata.to_string_lossy();
    let local_s = profile.join(r"AppData\Local").to_string_lossy().into_owned();
    let mut vars: Vec<(&str, &str)> = vec![
        ("SystemDrive", system_drive),
        ("USERPROFILE", &profile_s),
        ("APPDATA", &appdata_s),
        ("LOCALAPPDATA", &local_s),
    ];
    let home = match profile.components().next() {
        Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_)) => {
            let drive = p.as_os_str().to_string_lossy().into_owned();
            let rest = profile_s[drive.len()..].to_string();
            Some((drive, rest))
        }
        _ => None,
    };
    if let Some((drive, rest)) = &home {
        vars.push(("HOMEDRIVE", drive));
        vars.push(("HOMEPATH", rest));
    }
    expand_vars(raw, &vars).ok()
}

/// Thư mục Startup chung, theo HKLM `User Shell Folders\Common Startup` (thường
/// `%ProgramData%\Microsoft\Windows\Start Menu\Programs\Startup`); chưa đặt thì là chỗ mặc định.
pub fn common_startup() -> Result<PathBuf, String> {
    let data = program_data()?;
    match read_reg_opt(HKEY_LOCAL_MACHINE, "HKLM", SHELL_FOLDERS_KEY, "Common Startup")? {
        Some(raw) => expand_vars(
            &raw,
            &[
                ("SystemDrive", &system_drive()?),
                ("ProgramData", &data.to_string_lossy()),
                ("ALLUSERSPROFILE", &data.to_string_lossy()),
            ],
        ),
        None => Ok(data.join(STARTUP_REL)),
    }
}

/// `C:` — ổ của thư mục Windows, không có dấu `\` cuối (đúng dạng biến `%SystemDrive%`).
fn system_drive() -> Result<String, String> {
    Ok(system_drive_root()?.to_string_lossy().trim_end_matches('\\').to_string())
}

/// Thay `%tên%` (không phân biệt hoa thường) bằng giá trị trong `vars` — các giá trị này do chính crate lấy
/// qua API. Gặp biến nào khác, hoặc dấu `%` lẻ ⇒ từ chối, không bao giờ mở rộng theo môi trường tiến trình.
fn expand_vars(raw: &str, vars: &[(&str, &str)]) -> Result<PathBuf, String> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after.find('%').ok_or_else(|| format!("unexpected '%' in registry path: {raw}"))?;
        let name = &after[..end];
        let value = vars
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| *v)
            .ok_or_else(|| format!("unexpected environment variable in registry path: {raw}"))?;
        out.push_str(value);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    if out.is_empty() {
        return Err("empty registry path".into());
    }
    Ok(PathBuf::from(out))
}

/// Đọc chuỗi (REG_SZ/REG_EXPAND_SZ), KHÔNG để Windows tự mở rộng biến môi trường. Giá trị không có ⇒ `None`.
fn read_reg_opt(root: HKEY, root_name: &str, subkey: &str, value: &str) -> Result<Option<String>, String> {
    let key = wide(subkey);
    let name = wide(value);
    let mut buf = vec![0u16; 260];
    loop {
        let mut cb = (buf.len() * 2) as u32;
        // SAFETY: chuỗi kết thúc NUL; buf khả ghi đúng cb byte; kiểu trả về không cần (null).
        let err = unsafe {
            RegGetValueW(
                root,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut cb,
            )
        };
        match err {
            0 => return Ok(Some(from_wide(&buf[..(cb as usize / 2).min(buf.len())]))),
            ERROR_MORE_DATA => buf.resize((cb as usize).div_ceil(2) + 1, 0),
            ERROR_FILE_NOT_FOUND => return Ok(None),
            e => {
                return Err(format!(r"{root_name}\{subkey}\{value}: {}", std::io::Error::from_raw_os_error(e as i32)))
            }
        }
    }
}

fn read_reg_string(root: HKEY, root_name: &str, subkey: &str, value: &str) -> Result<String, String> {
    read_reg_opt(root, root_name, subkey, value)?.ok_or_else(|| {
        format!(r"{root_name}\{subkey}\{value}: {}", std::io::Error::from_raw_os_error(ERROR_FILE_NOT_FOUND as i32))
    })
}

/// SID người dùng của token tiến trình, dạng chuỗi (`S-1-5-21-…`).
fn current_user_sid() -> Result<String, String> {
    let fail = |what: &str| format!("{what}: {}", std::io::Error::last_os_error());
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: pseudo-handle tiến trình luôn hợp lệ; token là biến cục bộ, đóng ở dưới.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(fail("OpenProcessToken"));
    }
    // Bộ đệm u64 để TOKEN_USER (chứa con trỏ) được căn đúng.
    let mut buf = vec![0u64; 16];
    let res = loop {
        let mut needed = 0u32;
        // SAFETY: buf khả ghi đúng số byte truyền vào; needed là biến cục bộ.
        let ok = unsafe {
            GetTokenInformation(token, TokenUser, buf.as_mut_ptr().cast(), (buf.len() * 8) as u32, &mut needed)
        };
        if ok != 0 {
            break Ok(());
        }
        let e = std::io::Error::last_os_error();
        if e.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32) {
            break Err(format!("GetTokenInformation(TokenUser): {e}"));
        }
        buf.resize((needed as usize).div_ceil(8), 0);
    };
    // SAFETY: token do OpenProcessToken mở ở trên, đóng đúng một lần.
    unsafe { CloseHandle(token) };
    res?;
    // SAFETY: GetTokenInformation vừa ghi một TOKEN_USER hợp lệ ở đầu buf (căn 8 byte); SID nằm trong buf.
    let sid = unsafe { (*(buf.as_ptr() as *const TOKEN_USER)).User.Sid };
    let mut text: PWSTR = std::ptr::null_mut();
    // SAFETY: sid trỏ vào buf còn sống; text được LocalFree ở dưới.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(fail("ConvertSidToStringSidW"));
    }
    let mut len = 0;
    // SAFETY: text là chuỗi kết thúc NUL do API cấp, còn sống tới LocalFree.
    while unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: text có đúng len ký tự trước NUL.
    let s = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, len) });
    // SAFETY: text do ConvertSidToStringSidW cấp bằng LocalAlloc, giải phóng đúng một lần.
    unsafe { LocalFree(text.cast()) };
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_round_trips_vietnamese() {
        let w = wide("Tài liệu");
        assert_eq!(*w.last().unwrap(), 0);
        assert_eq!(from_wide(&w), "Tài liệu");
    }

    #[test]
    fn filetime_converts_to_unix_seconds() {
        assert_eq!(filetime_to_unix(EPOCH_DIFF_100NS + 10_000_000), 1);
        assert_eq!(filetime_to_unix(0), 0);
        // 2026-09-25 00:00:00 UTC
        assert_eq!(filetime_to_unix(EPOCH_DIFF_100NS + 1_790_294_400 * 10_000_000), 1_790_294_400);
    }

    /// Mọi đường dẫn hệ thống lấy qua API, nối bằng `|`, để so giữa tiến trình test và tiến trình con.
    fn system_paths() -> String {
        let all: [(&str, Result<PathBuf, String>); 9] = [
            ("windows_dir", windows_dir()),
            ("system_dir", system_dir()),
            ("system_drive_root", system_drive_root()),
            ("program_files", program_files()),
            ("program_files_x86", program_files_x86()),
            ("program_data", program_data()),
            ("user_profile", user_profile()),
            ("user_startup", user_startup()),
            ("common_startup", common_startup()),
        ];
        all.into_iter()
            .map(|(name, r)| {
                let p = r.unwrap_or_else(|e| panic!("{name}: {e}"));
                assert!(p.is_dir(), "{name}: {} không phải thư mục có thật", p.display());
                p.display().to_string()
            })
            .collect::<Vec<_>>()
            .join("|")
    }

    #[test]
    fn system_paths_are_existing_directories() {
        system_paths();
    }

    #[test]
    fn system_dir_lives_inside_windows_dir_and_drive_root_is_its_root() {
        let windir = windows_dir().unwrap();
        let system32 = system_dir().unwrap();
        assert!(system32.starts_with(&windir), "{} / {}", system32.display(), windir.display());
        let root = system_drive_root().unwrap();
        assert!(windir.starts_with(&root), "{} / {}", windir.display(), root.display());
        let s = root.to_string_lossy();
        assert_eq!(s.len(), 3, "{s}");
        assert!(s.ends_with(r":\"), "{s}");
    }

    #[test]
    fn known_folder_reports_the_raw_error_for_an_unknown_id() {
        let err = known_folder(&windows_sys::core::GUID::from_u128(0x1234_5678_9abc_def0_1234_5678_9abc_def0)).unwrap_err();
        assert!(err.contains("SHGetKnownFolderPath"), "{err}");
    }

    #[test]
    fn registry_paths_expand_only_the_variables_we_resolved_ourselves() {
        let vars = [("SystemDrive", "C:"), ("USERPROFILE", r"C:\Users\an")];
        assert_eq!(expand_vars(r"%systemdrive%\ProgramData", &vars).unwrap(), PathBuf::from(r"C:\ProgramData"));
        assert_eq!(
            expand_vars(r"%UserProfile%\AppData\%USERPROFILE%", &vars).unwrap(),
            PathBuf::from(r"C:\Users\an\AppData\C:\Users\an")
        );
        assert_eq!(expand_vars(r"D:\Khởi động", &vars).unwrap(), PathBuf::from(r"D:\Khởi động"));
        let err = expand_vars(r"%APPDATA%\Microsoft", &vars).unwrap_err();
        assert!(err.contains(r"%APPDATA%\Microsoft"), "{err}");
        assert!(expand_vars("", &vars).is_err());
        assert!(expand_vars(r"C:\100%", &vars).is_err());
    }

    #[test]
    fn user_startup_expands_profile_variables_and_falls_back_on_anything_else() {
        let profile = Path::new(r"C:\Users\an");
        let f = |raw: Option<&str>| user_startup_from(raw, profile, "C:");
        let dflt = PathBuf::from(r"C:\Users\an\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup");
        assert_eq!(f(None), Some(dflt.clone()));
        assert_eq!(f(Some(r"%USERPROFILE%\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup")), Some(dflt.clone()));
        assert_eq!(f(Some(r"%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup")), Some(dflt));
        assert_eq!(f(Some(r"%LOCALAPPDATA%\Khởi động")), Some(PathBuf::from(r"C:\Users\an\AppData\Local\Khởi động")));
        assert_eq!(f(Some(r"%HOMEDRIVE%%HOMEPATH%\Startup")), Some(PathBuf::from(r"C:\Users\an\Startup")));
        assert_eq!(f(Some(r"%SystemDrive%\Startup")), Some(PathBuf::from(r"C:\Startup")));
        assert_eq!(f(Some(r"D:\Khởi động")), Some(PathBuf::from(r"D:\Khởi động")));
        // Biến ngoài danh sách (vd OneDrive dời thư mục) hoặc chuỗi hỏng ⇒ None, người gọi lùi về Shell.
        assert_eq!(f(Some(r"%OneDrive%\Startup")), None);
        assert_eq!(f(Some(r"C:\100%")), None);
        assert_eq!(f(Some("")), None);
        // Hồ sơ không nằm trên ổ có chữ cái ⇒ không biết HOMEDRIVE ⇒ lùi về Shell.
        assert_eq!(user_startup_from(Some(r"%HOMEDRIVE%\x"), Path::new(r"\\srv\hoso\an"), "C:"), None);
    }

    const CLEAN_ENV_CHILD: &str = "WFU_DIAG_CLEAN_ENV_CHILD";

    /// Trong môi trường sạch (dựng lại từ API, không thừa hưởng môi trường của người chạy test), đường dẫn tự
    /// dựng từ registry phải trùng với Shell.
    #[test]
    fn registry_paths_match_the_shell_in_a_clean_environment() {
        use windows_sys::Win32::UI::Shell::{
            FOLDERID_CommonStartup, FOLDERID_Profile, FOLDERID_ProgramData, FOLDERID_Startup,
        };
        if std::env::var_os(CLEAN_ENV_CHILD).is_some() {
            let same = |a: PathBuf, b: PathBuf| {
                assert!(a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy()), "{} ≠ {}", a.display(), b.display())
            };
            same(program_data().unwrap(), known_folder(&FOLDERID_ProgramData).unwrap());
            same(user_profile().unwrap(), known_folder(&FOLDERID_Profile).unwrap());
            same(user_startup().unwrap(), known_folder(&FOLDERID_Startup).unwrap());
            same(common_startup().unwrap(), known_folder(&FOLDERID_CommonStartup).unwrap());
            return;
        }
        let windir = windows_dir().unwrap();
        let system32 = system_dir().unwrap();
        let drive = system_drive().unwrap();
        let data = program_data().unwrap();
        let profile = user_profile().unwrap();
        let home_path = profile.to_string_lossy()[drive.len()..].to_string();
        let mut path = system32.clone().into_os_string();
        path.push(";");
        path.push(&windir);
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.env_clear()
            .args(["--exact", "util::tests::registry_paths_match_the_shell_in_a_clean_environment", "--test-threads=1"])
            .env(CLEAN_ENV_CHILD, "1")
            .env("SystemRoot", &windir)
            .env("windir", &windir)
            .env("SystemDrive", &drive)
            .env("PATH", path)
            .env("TEMP", windir.join("Temp"))
            .env("TMP", windir.join("Temp"))
            .env("ProgramData", &data)
            .env("ALLUSERSPROFILE", &data)
            .env("USERPROFILE", &profile)
            .env("APPDATA", profile.join(r"AppData\Roaming"))
            .env("LOCALAPPDATA", profile.join(r"AppData\Local"))
            .env("HOMEDRIVE", &drive)
            .env("HOMEPATH", home_path);
        let out = cmd.output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
        assert!(stdout.contains("1 passed"), "{stdout}");
    }

    const FAKE_ENV_CHILD: &str = "WFU_DIAG_FAKE_ENV_CHILD";

    /// Chạy lại chính test này trong tiến trình con có SystemRoot/windir/SystemDrive/ProgramData/ProgramFiles/
    /// USERPROFILE/APPDATA… giả — kết quả phải y hệt tiến trình test. Không `set_var` trong tiến trình test
    /// (các test khác chạy song song đọc chung môi trường).
    #[test]
    fn system_paths_come_from_the_api_not_from_environment_variables() {
        if let Some(fake) = std::env::var_os(FAKE_ENV_CHILD) {
            assert_eq!(std::env::var_os("windir"), Some(fake), "env not faked");
            let expected = std::env::var("WFU_DIAG_EXPECTED").unwrap();
            assert_eq!(system_paths(), expected);
            return;
        }
        let expected = system_paths();
        let fake = tempfile::tempdir().unwrap();
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "util::tests::system_paths_come_from_the_api_not_from_environment_variables",
            "--test-threads=1",
        ])
        .env(FAKE_ENV_CHILD, fake.path())
        .env("WFU_DIAG_EXPECTED", &expected)
        .env("SystemDrive", "Z:");
        for k in [
            "SystemRoot",
            "windir",
            "ProgramData",
            "ALLUSERSPROFILE",
            "PUBLIC",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "ProgramW6432",
            "CommonProgramFiles",
            "USERPROFILE",
            "HOMEPATH",
            "APPDATA",
            "LOCALAPPDATA",
            "TEMP",
            "TMP",
        ] {
            cmd.env(k, fake.path());
        }
        let out = cmd.output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
        assert!(stdout.contains("1 passed"), "{stdout}");
    }
}
