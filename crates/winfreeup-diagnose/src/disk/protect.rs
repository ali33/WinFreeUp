//! Danh sách đường dẫn được bảo vệ (spec 3.3): hiện 🔒, mở xem được nhưng không xóa được.
//!
//! Hai tầng: `is_protected_lexical` chỉ so chuỗi (dùng để vẽ 🔒 cho cả cây, không chạm đĩa);
//! `is_protected` là cửa chặn ngay trước khi xóa — thêm kiểm hình dạng đường dẫn, tổ tiên là link, và đường
//! thật sau `canonicalize`.
use std::path::{Component, Path, PathBuf, Prefix};

use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FileAttributeTagInfo, GetFileInformationByHandleEx, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};

use crate::util;

/// Tên ngay dưới gốc bất kỳ ổ nào mà xóa đi thì hỏng hệ thống: tệp hoán trang/ngủ đông, dữ liệu khôi phục,
/// khởi động, và metafile NTFS. KHÔNG chặn mọi tên bắt đầu bằng `$`: `$WINDOWS.~BT`, `$Windows.~WS`,
/// `$GetCurrent`, `$SysReset` là phần thừa sau cập nhật/cài lại Windows, dọn được.
const ROOT_SPECIAL: [&str; 22] = [
    "pagefile.sys",
    "hiberfil.sys",
    "swapfile.sys",
    "system volume information",
    "$recycle.bin",
    "$winreagent",
    "bootmgr",
    "bootnxt",
    "boot",
    "recovery",
    "efi",
    "$mft",
    "$mftmirr",
    "$logfile",
    "$volume",
    "$attrdef",
    "$bitmap",
    "$boot",
    "$badclus",
    "$secure",
    "$upcase",
    "$extend",
];

/// Bit «name surrogate» của reparse tag: junction, symlink, mount point… trỏ sang chỗ khác. Reparse point không
/// có bit này (vd thư mục OneDrive, dedup) không dẫn đi đâu, và bộ quét vẫn đi vào chúng.
const NAME_SURROGATE: u32 = 0x2000_0000;

#[derive(Debug, Clone)]
pub struct ProtectRules {
    /// Chặn chính nó và mọi thứ bên trong: `%WINDIR%`, `Program Files`, `Program Files (x86)`, `ProgramData`, `C:\Users`.
    system_dirs: Vec<String>,
    /// `%USERPROFILE%` (và đường thật của nó nếu khác): chặn chính nó. `true` = con bên trong được xóa — chỉ khi
    /// hồ sơ không nằm trong thư mục hệ thống nào ngoài thư mục cha của nó (thư mục Users).
    user_profiles: Vec<(String, bool)>,
}

impl ProtectRules {
    pub fn new(system_dirs: &[PathBuf], user_profile: &Path) -> Self {
        Self::from_keys(system_dirs.iter().map(|p| key(p)).collect(), vec![key(user_profile)])
    }

    fn from_keys(system_dirs: Vec<String>, profiles: Vec<String>) -> Self {
        let user_profiles = profiles
            .into_iter()
            .map(|p| {
                let parent = p.rsplit_once('\\').map(|(dir, _)| dir);
                let exempt = !system_dirs.iter().any(|d| is_within(&p, d) && Some(d.as_str()) != parent);
                (p, exempt)
            })
            .collect();
        ProtectRules { system_dirs, user_profiles }
    }

    /// Luật của máy đang chạy, lấy qua API/registry (`util`), KHÔNG qua biến môi trường: tiến trình chạy quyền
    /// Admin mà người dùng thường đặt được `SystemRoot`, `ProgramData`, `USERPROFILE`… của chính mình.
    ///
    /// Thư mục hồ sơ chung (`C:\Users`) = thư mục cha của hồ sơ người dùng. Không đọc HKLM
    /// `ProfileList\ProfilesDirectory`: hàm đọc registry của `util` là nội bộ, và thư mục cha của hồ sơ thật
    /// (cũng lấy từ HKLM `ProfileList\<SID>`) cho cùng kết quả trên máy thường.
    ///
    /// Hồ sơ (hoặc đường thật của nó) nằm trong Windows/ProgramData/Program Files ⇒ `Err`: luật miễn trừ con
    /// trong hồ sơ sẽ mở cửa vào thư mục hệ thống.
    pub fn from_system() -> Result<Self, String> {
        let profile = util::user_profile()?;
        let users = profile
            .parent()
            .filter(|p| p.parent().is_some())
            .ok_or_else(|| format!("user profile has no parent directory: {}", profile.display()))?
            .to_path_buf();
        let system = [util::windows_dir()?, util::program_data()?, util::program_files()?, util::program_files_x86()?];
        // Thư mục bị dời bằng junction (vd `C:\Users` → `D:\Users`): chặn cả đường thật, để đi thẳng vào đích
        // (quét ổ D:) cũng bị chặn như đi qua tên cũ.
        let with_real = |dirs: &[&Path]| {
            let mut keys = Vec::new();
            for d in dirs {
                push_unique(&mut keys, key(d));
                if let Ok(real) = std::fs::canonicalize(d) {
                    push_unique(&mut keys, key(&real));
                }
            }
            keys
        };
        let system_keys = with_real(&system.iter().map(PathBuf::as_path).collect::<Vec<_>>());
        let profile_keys = with_real(&[profile.as_path()]);
        check_profiles(&system_keys, &profile_keys)?;
        let mut dirs = system_keys;
        for k in with_real(&[users.as_path()]) {
            push_unique(&mut dirs, k);
        }
        Ok(Self::from_keys(dirs, profile_keys))
    }

    /// Có bị chặn xóa không — `protect_reason(path).is_some()`.
    ///
    /// Đây chỉ là BƯỚC KIỂM, không phải khóa: giữa lúc kiểm và lúc xóa đường dẫn vẫn có thể bị tráo. Người xóa
    /// phải kiểm lại ngay trước thao tác, trong lúc giữ khóa (handle mở, không cho xóa/đổi tên) các thư mục tổ tiên.
    pub fn is_protected(&self, path: &Path) -> bool {
        self.protect_reason(path).is_some()
    }

    /// Mã lý do chặn (lõi không chứa chữ hiển thị; giao diện dịch), `None` = xóa được:
    /// - `bad_path`: hình dạng lạ — tương đối, thiếu `\` sau ổ, UNC (kể cả `\\localhost\C$`), tiền tố thiết bị
    ///   (`\\.\`, `\\?\GLOBALROOT`, `\\?\Volume{…}`), có `.`/`..`, tên kết thúc bằng `.` hoặc dấu cách (Win32 và
    ///   `\\?\` hiểu khác nhau), có `:` (luồng dữ liệu) hay ký tự đại diện;
    /// - `drive_root`, `root_special`, `user_profile_root`, `system_dir`: luật chặn đường gõ vào (xem
    ///   `is_protected_lexical`);
    /// - `link_ancestor`: BẤT KỲ thư mục tổ tiên nào (dưới gốc ổ, trên đích) là junction/symlink/mount point,
    ///   không phải thư mục, hoặc không đọc được thuộc tính. Bộ quét không đi theo link nên nút hợp lệ không bao
    ///   giờ nằm dưới link; gặp link nghĩa là cây đã bị tráo sau khi quét và lệnh xóa có thể bị dẫn đi nơi khác;
    /// - `canonical_protected`: đường thật sau `canonicalize` bị luật chặn (bản thân đích là link thì chỉ chặn vì
    ///   lý do này — xóa link không xóa đích), hoặc `canonicalize` hỏng vì lý do khác «không có».
    pub fn protect_reason(&self, path: &Path) -> Option<&'static str> {
        let Some(verbatim) = checked_verbatim(path) else {
            return Some("bad_path");
        };
        if let Some(r) = self.lexical_reason(path).or_else(|| self.lexical_reason(&verbatim)) {
            return Some(r);
        }
        if !verbatim.ancestors().skip(1).take_while(|a| a.parent().is_some()).all(plain_dir) {
            return Some("link_ancestor");
        }
        match std::fs::canonicalize(&verbatim) {
            Ok(real) if self.lexical_reason(&real).is_none() => None,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            _ => Some("canonical_protected"),
        }
    }

    /// Chỉ so chuỗi (đã chuẩn hóa `..`, `.`, hoa thường, tiền tố `\\?\`/`\\.\`, chấm/cách cuối tên) — không chạm đĩa.
    pub fn is_protected_lexical(&self, path: &Path) -> bool {
        self.lexical_reason(path).is_some()
    }

    fn lexical_reason(&self, path: &Path) -> Option<&'static str> {
        let parts = parts(path);
        // UNC (kể cả `\\?\UNC\…`, `\\.\UNC\…`): chỉ nhận ổ có chữ cái.
        if parts.first().is_some_and(|p| p.starts_with(r"\\") || p == "unc") {
            return Some("bad_path");
        }
        if parts.len() <= 1 {
            return Some("drive_root");
        }
        // Bỏ phần luồng dữ liệu (`$MFT::$DATA`) trước khi so tên.
        let top = parts[1].split(':').next().unwrap_or_default().trim_end_matches(['.', ' ']);
        if ROOT_SPECIAL.contains(&top) {
            return Some("root_special");
        }
        let k = parts.join("\\");
        if self.user_profiles.iter().any(|(p, _)| *p == k) {
            return Some("user_profile_root");
        }
        if self.user_profiles.iter().any(|(p, exempt)| *exempt && is_within(&k, p)) {
            return None;
        }
        self.system_dirs.iter().any(|d| k == *d || is_within(&k, d)).then_some("system_dir")
    }
}

/// Hồ sơ (hoặc đường thật của nó) không được là/nằm trong thư mục hệ thống (khóa đã chuẩn hóa).
fn check_profiles(system_dirs: &[String], profiles: &[String]) -> Result<(), String> {
    for p in profiles {
        if let Some(d) = system_dirs.iter().find(|d| p == *d || is_within(p, d)) {
            return Err(format!("user profile {p} is inside system directory {d}"));
        }
    }
    Ok(())
}

fn push_unique(v: &mut Vec<String>, k: String) {
    if !v.contains(&k) {
        v.push(k);
    }
}

fn is_within(k: &str, dir: &str) -> bool {
    k.len() > dir.len() && k.starts_with(dir) && k.as_bytes()[dir.len()] == b'\\'
}

/// Dạng so sánh: các phần đã chuẩn hóa nối bằng `\`.
fn key(path: &Path) -> String {
    parts(path).join("\\")
}

/// Phần đầu là gốc (`c:`; UNC giữ dạng `\\srv\share` để bị nhận ra và chặn), sau đó từng tên: bỏ `\\?\`/`\\.\`,
/// gộp `.`/`..`, chữ thường, bỏ chấm/cách cuối tên (Win32 tự bỏ khi mở, nên `C:\Windows.` chính là `C:\Windows`).
fn parts(path: &Path) -> Vec<String> {
    let s = path.to_string_lossy();
    let s = if let Some(r) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{r}")
    } else if let Some(r) = s.strip_prefix(r"\\?\").or_else(|| s.strip_prefix(r"\\.\")) {
        r.to_string()
    } else {
        s.into_owned()
    };
    let mut out: Vec<String> = Vec::new();
    for c in Path::new(&s).components() {
        match c {
            Component::Prefix(p) => out.push(p.as_os_str().to_string_lossy().to_lowercase()),
            Component::RootDir | Component::CurDir => {}
            Component::ParentDir => {
                if out.len() > 1 {
                    out.pop();
                }
            }
            Component::Normal(n) => {
                let n = n.to_string_lossy();
                let n = n.trim_end_matches(['.', ' ']);
                if !n.is_empty() {
                    out.push(n.to_lowercase());
                }
            }
        }
    }
    out
}

/// Đường dẫn tuyệt đối trên ổ có chữ cái, chỉ gồm tên thường ⇒ dạng `\\?\X:\…` (đọc được cả đường dài hơn 260
/// ký tự, không để Win32 diễn giải lại). Hình dạng lạ hoặc UNC ⇒ `None`.
fn checked_verbatim(path: &Path) -> Option<PathBuf> {
    let mut comps = path.components();
    let mut out = match comps.next() {
        Some(Component::Prefix(p)) => match p.kind() {
            Prefix::Disk(l) | Prefix::VerbatimDisk(l) => format!(r"\\?\{}:", l as char),
            _ => return None,
        },
        _ => return None,
    };
    if comps.next() != Some(Component::RootDir) {
        return None;
    }
    let mut any = false;
    for c in comps {
        let Component::Normal(n) = c else {
            return None;
        };
        let n = n.to_str()?;
        if n == "."
            || n == ".."
            || n.ends_with(['.', ' '])
            || n.contains([':', '/', '\\', '*', '?', '"', '<', '>', '|'])
        {
            return None;
        }
        out.push('\\');
        out.push_str(n);
        any = true;
    }
    if !any {
        out.push('\\');
    }
    Some(PathBuf::from(out))
}

/// Là thư mục thật, không phải junction/symlink/mount point. Mở không đi theo reparse point; không mở hoặc
/// không đọc được thuộc tính ⇒ `false` (coi là bị chặn).
fn plain_dir(dir: &Path) -> bool {
    let w = util::wide(dir);
    // SAFETY: w là chuỗi kết thúc NUL; không có security attributes/template; handle đóng ở dưới.
    let h = unsafe {
        CreateFileW(
            w.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if h == INVALID_HANDLE_VALUE {
        return false;
    }
    let mut info = FILE_ATTRIBUTE_TAG_INFO { FileAttributes: 0, ReparseTag: 0 };
    // SAFETY: h là handle hợp lệ vừa mở; info khả ghi đúng kích thước truyền vào.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            h,
            FileAttributeTagInfo,
            (&mut info as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
            std::mem::size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    };
    // SAFETY: h do CreateFileW mở ở trên, đóng đúng một lần.
    unsafe { CloseHandle(h) };
    let link = info.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 && info.ReparseTag & NAME_SURROGATE != 0;
    ok != 0 && info.FileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0 && !link
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> ProtectRules {
        ProtectRules::new(
            &[
                PathBuf::from(r"C:\Windows"),
                PathBuf::from(r"C:\Program Files"),
                PathBuf::from(r"C:\Program Files (x86)"),
                PathBuf::from(r"C:\ProgramData"),
                PathBuf::from(r"C:\Users"),
            ],
            Path::new(r"C:\Users\an"),
        )
    }

    #[test]
    fn roots_and_system_dirs_themselves_are_blocked() {
        let r = rules();
        for p in [r"C:\", r"D:\", r"C:\Windows", r"c:\windows\", r"C:\Program Files", r"C:\Program Files (x86)", r"C:\ProgramData", r"C:\Users", r"C:\Users\an"] {
            assert!(r.is_protected_lexical(Path::new(p)), "{p}");
        }
    }

    #[test]
    fn inside_system_dirs_is_blocked_but_inside_the_profile_is_allowed() {
        let r = rules();
        assert!(r.is_protected_lexical(Path::new(r"C:\Windows\System32\drivers")));
        assert!(r.is_protected_lexical(Path::new(r"C:\Program Files\Adobe")));
        assert!(r.is_protected_lexical(Path::new(r"C:\Users\Public\Videos")), "hồ sơ người khác vẫn thuộc C:\\Users");
        assert!(!r.is_protected_lexical(Path::new(r"C:\Users\an\Downloads\abc")));
        assert!(!r.is_protected_lexical(Path::new(r"C:\Users\an\AppData\Local\Temp")));
        assert!(!r.is_protected_lexical(Path::new(r"D:\Games\old")));
        assert!(!r.is_protected_lexical(Path::new(r"C:\Windows.old")), "tên bắt đầu giống nhưng không nằm trong C:\\Windows");
    }

    #[test]
    fn special_files_at_any_drive_root_are_blocked() {
        let r = rules();
        for p in [r"C:\pagefile.sys", r"D:\hiberfil.sys", r"C:\swapfile.sys", r"E:\System Volume Information", r"E:\System Volume Information\x", r"C:\$Recycle.Bin\S-1-5", r"C:\$MFT"] {
            assert!(r.is_protected_lexical(Path::new(p)), "{p}");
        }
        assert!(!r.is_protected_lexical(Path::new(r"D:\data\pagefile.sys")), "chỉ chặn ở gốc ổ");
    }

    #[test]
    fn dot_dot_and_verbatim_prefix_cannot_sneak_past() {
        let r = rules();
        assert!(r.is_protected_lexical(Path::new(r"C:\Users\an\Downloads\..\..\..\Windows\Temp")));
        assert!(r.is_protected_lexical(Path::new(r"\\?\C:\Windows\Temp")));
        assert!(r.is_protected_lexical(Path::new(r"C:\Users\an\Downloads\..")));
        // Thêm ngoài kế hoạch: tiền tố thiết bị `\\.\`, gạch chéo xuôi, dấu chấm/cách cuối tên (Win32 tự bỏ).
        assert!(r.is_protected_lexical(Path::new(r"\\.\C:\Windows\Temp")));
        assert!(r.is_protected_lexical(Path::new("C:/Windows/Temp")));
        assert!(r.is_protected_lexical(Path::new(r"C:\Windows.\Temp")));
        assert!(r.is_protected_lexical(Path::new(r"C:\Windows \Temp")));
        assert!(r.is_protected_lexical(Path::new(r"C:\pagefile.sys.")));
    }

    #[test]
    fn only_real_system_names_at_the_drive_root_are_blocked() {
        let r = rules();
        for p in [
            r"C:\$MFT", r"C:\$MFTMirr", r"C:\$LogFile", r"C:\$Volume", r"C:\$AttrDef", r"C:\$Bitmap", r"C:\$Boot",
            r"C:\$BadClus", r"C:\$Secure", r"C:\$UpCase", r"C:\$Extend\$Quota", r"C:\$Recycle.Bin", r"C:\$WinREAgent\x",
            r"C:\bootmgr", r"C:\BOOTNXT", r"C:\Boot\BCD", r"D:\Recovery", r"C:\EFI", r"C:\$MFT::$DATA",
        ] {
            assert!(r.is_protected_lexical(Path::new(p)), "{p}");
            assert!(r.protect_reason(Path::new(p)).is_some(), "{p}");
        }
        // Thư mục cập nhật/cài lại Windows để lại ở gốc ổ: dọn được, không phải metafile.
        for p in [r"C:\$WINDOWS.~BT", r"C:\$Windows.~WS\x", r"C:\$GetCurrent", r"C:\$SysReset", r"D:\data\bootmgr", r"D:\data\Boot"] {
            assert!(!r.is_protected_lexical(Path::new(p)), "{p}");
        }
    }

    #[test]
    fn unc_paths_are_always_blocked() {
        let r = rules();
        for p in [
            r"\\srv\share",
            r"\\srv\share\",
            r"\\srv\share\data",
            r"\\?\UNC\srv\share\data",
            r"\\localhost\C$\Users\an\Downloads\x",
            r"\\?\UNC\127.0.0.1\c$\Users\an\Downloads\x",
            r"\\.\UNC\srv\share\x",
        ] {
            assert!(r.is_protected_lexical(Path::new(p)), "{p}");
            assert_eq!(r.protect_reason(Path::new(p)), Some("bad_path"), "{p}");
        }
    }

    #[test]
    fn every_block_has_a_reason_code() {
        let r = rules();
        let reason = |p: &str| r.protect_reason(Path::new(p));
        assert_eq!(reason(r"C:\Windows\System32"), Some("system_dir"));
        assert_eq!(reason(r"C:\Users\Public\x"), Some("system_dir"));
        assert_eq!(reason(r"C:\Users\an"), Some("user_profile_root"));
        assert_eq!(reason(r"D:\"), Some("drive_root"));
        assert_eq!(reason(r"C:\pagefile.sys"), Some("root_special"));
        assert_eq!(reason(r"C:\Users\an\..\x"), Some("bad_path"));
        assert_eq!(reason(r"\\srv\share\x"), Some("bad_path"));
        let f = fake();
        let link = f.downloads.join("link");
        junction::create(f.sys.join("System32"), &link).unwrap();
        assert_eq!(f.rules.protect_reason(&link), Some("canonical_protected"));
        assert_eq!(f.rules.protect_reason(&link.join("x")), Some("link_ancestor"));
        assert_eq!(f.rules.protect_reason(&f.downloads), None);
        assert!(f.rules.is_protected(&link));
        assert!(!f.rules.is_protected(&f.downloads));
    }

    #[test]
    fn a_profile_inside_a_system_dir_is_not_an_exemption() {
        let r = ProtectRules::new(
            &[PathBuf::from(r"C:\Windows"), PathBuf::from(r"C:\Users")],
            Path::new(r"C:\Windows\x\prof"),
        );
        assert_eq!(r.protect_reason(Path::new(r"C:\Windows\x\prof\a")), Some("system_dir"));
        assert_eq!(r.protect_reason(Path::new(r"C:\Windows\x\prof")), Some("user_profile_root"));
        let sys = [r"c:\windows".to_string()];
        assert!(check_profiles(&sys, &[r"c:\windows\x\prof".to_string()]).is_err());
        assert!(check_profiles(&sys, &[r"c:\windows".to_string()]).is_err());
        assert!(check_profiles(&sys, &[r"c:\users\an".to_string(), r"d:\windows\an".to_string()]).is_ok());
        assert!(check_profiles(&sys, &[r"c:\users\an".to_string(), r"c:\windows\an".to_string()]).is_err(), "đường thật");
    }

    #[test]
    fn names_ending_in_a_dot_or_space_are_blocked() {
        let f = fake();
        let profile = f.downloads.parent().unwrap().to_path_buf();
        // Thư mục thật tên `an.` (chỉ tạo được qua `\\?\`). Win32 hiểu `…\an.\x` là `…\an\x`, còn `\\?\` thì không.
        let mut odd = std::ffi::OsString::from(r"\\?\");
        odd.push(profile.as_os_str());
        odd.push(".");
        let odd = PathBuf::from(odd);
        std::fs::create_dir_all(odd.join("x")).unwrap();
        let mut alias = profile.clone().into_os_string();
        alias.push(r".\x");
        assert_eq!(f.rules.protect_reason(Path::new(&alias)), Some("bad_path"));
        assert_eq!(f.rules.protect_reason(&odd.join("x")), Some("bad_path"));
        assert_eq!(f.rules.protect_reason(&f.downloads.join("a ")), Some("bad_path"));
        std::fs::remove_dir_all(&odd).unwrap();
    }

    /// Cây giả trong thư mục tạm: `<t>\Windows\System32`, `<t>\Users\an\Downloads`, luật dựng trên đó.
    struct Fake {
        t: tempfile::TempDir,
        sys: PathBuf,
        downloads: PathBuf,
        rules: ProtectRules,
    }

    fn fake() -> Fake {
        let t = tempfile::tempdir().unwrap();
        let sys = t.path().join("Windows");
        let profile = t.path().join("Users").join("an");
        let downloads = profile.join("Downloads");
        std::fs::create_dir_all(sys.join("System32")).unwrap();
        std::fs::create_dir_all(&downloads).unwrap();
        let rules = ProtectRules::new(std::slice::from_ref(&sys), &profile);
        Fake { t, sys, downloads, rules }
    }

    #[test]
    fn junction_into_a_protected_dir_is_blocked_after_canonicalize() {
        let f = fake();
        let link = f.downloads.join("link");
        junction::create(f.sys.join("System32"), &link).unwrap();
        assert!(!f.rules.is_protected_lexical(&link));
        assert!(f.rules.is_protected(&link));
        assert!(!f.rules.is_protected(&f.downloads));
        let mut verbatim = std::ffi::OsString::from(r"\\?\");
        verbatim.push(link.as_os_str());
        assert!(f.rules.is_protected(Path::new(&verbatim)), "tiền tố \\\\?\\ cũng phải bị chặn");
    }

    #[test]
    fn anything_below_a_junction_is_blocked_even_if_it_points_somewhere_harmless() {
        let f = fake();
        let harmless = f.t.path().join("harmless");
        std::fs::create_dir_all(harmless.join("sub")).unwrap();
        std::fs::write(harmless.join("sub").join("a.txt"), b"x").unwrap();
        let link = f.downloads.join("link");
        junction::create(&harmless, &link).unwrap();
        assert!(!f.rules.is_protected(&harmless.join("sub").join("a.txt")), "đường thật thì xóa được");
        assert!(f.rules.is_protected(&link.join("sub")));
        assert!(f.rules.is_protected(&link.join("sub").join("a.txt")));
        assert!(f.rules.is_protected(&link.join("khong-co")), "dưới junction thì chặn kể cả khi đích không tồn tại");
    }

    #[test]
    fn the_link_itself_is_deletable_when_it_points_somewhere_harmless() {
        let f = fake();
        let harmless = f.t.path().join("harmless");
        std::fs::create_dir_all(&harmless).unwrap();
        let link = f.downloads.join("link");
        junction::create(&harmless, &link).unwrap();
        assert!(!f.rules.is_protected(&link), "xóa link, không xóa đích");
        // Link treo (đích đã mất) cũng xóa được.
        std::fs::remove_dir(&harmless).unwrap();
        assert!(!f.rules.is_protected(&link));
    }

    #[test]
    fn a_folder_swapped_for_a_junction_after_the_scan_is_blocked() {
        let f = fake();
        let dir = f.downloads.join("cu");
        std::fs::create_dir_all(dir.join("drivers")).unwrap();
        let target = dir.join("drivers");
        assert!(!f.rules.is_protected(&target), "lúc quét: thư mục thường");
        // Kẻ xấu tráo `cu` thành junction trỏ vào thư mục hệ thống giả, giữ nguyên đường dẫn con.
        std::fs::remove_dir_all(&dir).unwrap();
        std::fs::create_dir_all(f.sys.join("System32").join("drivers")).unwrap();
        junction::create(f.sys.join("System32"), &dir).unwrap();
        assert!(f.rules.is_protected(&target));
    }

    #[test]
    fn an_ancestor_that_cannot_be_checked_blocks() {
        let f = fake();
        assert!(f.rules.is_protected(&f.downloads.join("khong-co").join("x")), "tổ tiên không có ⇒ chặn");
        let file = f.downloads.join("a.txt");
        std::fs::write(&file, b"x").unwrap();
        assert!(f.rules.is_protected(&file.join("x")), "tổ tiên là file ⇒ chặn");
        assert!(!f.rules.is_protected(&f.downloads.join("khong-co")), "đích không có thì không có gì để xóa");
    }

    #[test]
    fn odd_path_shapes_are_blocked_before_touching_the_disk() {
        let f = fake();
        let dir = f.downloads.join("a");
        std::fs::create_dir_all(dir.join("b")).unwrap();
        assert!(!f.rules.is_protected(&dir.join("b")));
        assert!(f.rules.is_protected(&dir.join("..").join("a").join("b")), "có `..` ⇒ chặn");
        assert!(f.rules.is_protected(Path::new(r"Downloads\a")), "đường tương đối ⇒ chặn");
        assert!(f.rules.is_protected(Path::new(r"C:Downloads")), "C: không có `\\` ⇒ chặn");
        assert!(f.rules.is_protected(Path::new(r"\Downloads\a")), "không có ổ ⇒ chặn");
        let mut ads = dir.join("b").into_os_string();
        ads.push("::$INDEX_ALLOCATION");
        assert!(f.rules.is_protected(Path::new(&ads)), "luồng dữ liệu (`:`) ⇒ chặn");
        for p in [r"\\?\GLOBALROOT\Device\HarddiskVolume3\x", r"\\?\Volume{00000000-0000-0000-0000-000000000000}\x", r"\\.\PhysicalDrive0"] {
            assert!(f.rules.is_protected(Path::new(p)), "{p}");
        }
    }

    #[test]
    fn rules_from_this_machine_block_windir() {
        let r = ProtectRules::from_system().unwrap();
        let windir = crate::util::windows_dir().unwrap();
        assert!(r.is_protected(&windir));
        assert!(r.is_protected(&crate::util::system_dir().unwrap()));
        assert!(r.is_protected(&crate::util::program_data().unwrap()));
        assert!(r.is_protected(&crate::util::program_files().unwrap()));
        assert!(r.is_protected(&crate::util::program_files_x86().unwrap()));
        let profile = crate::util::user_profile().unwrap();
        assert!(r.is_protected(&profile));
        assert!(r.is_protected(profile.parent().unwrap()));
        let t = tempfile::tempdir().unwrap();
        assert!(!r.is_protected(&t.path().join("khong-co-9d1f")));
    }

    const FAKE_ENV_CHILD: &str = "WFU_PROTECT_FAKE_ENV_CHILD";

    /// Chạy lại test này trong tiến trình con có SystemRoot/ProgramData/ProgramFiles/USERPROFILE… trỏ vào thư
    /// mục tạm: luật vẫn phải chặn thư mục thật và không chặn thư mục giả (không `set_var` trong tiến trình test).
    #[test]
    fn rules_from_the_system_ignore_faked_environment_variables() {
        if let Some(fake) = std::env::var_os(FAKE_ENV_CHILD) {
            let fake = PathBuf::from(fake);
            assert_eq!(std::env::var_os("SystemRoot").map(PathBuf::from), Some(fake.clone()), "env not faked");
            let r = ProtectRules::from_system().unwrap();
            let expected = std::env::var("WFU_PROTECT_EXPECTED").unwrap();
            for real in expected.split('|') {
                assert!(r.is_protected(Path::new(real)), "{real}");
            }
            let inside = fake.join("x");
            std::fs::create_dir_all(&inside).unwrap();
            assert!(!r.is_protected(&inside), "SystemRoot giả không được thành thư mục hệ thống");
            return;
        }
        let real = [
            crate::util::windows_dir().unwrap().join("System32"),
            crate::util::program_data().unwrap(),
            crate::util::program_files().unwrap(),
            crate::util::program_files_x86().unwrap(),
            crate::util::user_profile().unwrap(),
            crate::util::user_profile().unwrap().parent().unwrap().join("Public"),
        ];
        let expected = real.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("|");
        let fake = tempfile::tempdir().unwrap();
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "disk::protect::tests::rules_from_the_system_ignore_faked_environment_variables",
            "--test-threads=1",
        ])
        .env(FAKE_ENV_CHILD, fake.path())
        .env("WFU_PROTECT_EXPECTED", &expected)
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
            "USERPROFILE",
            "HOMEPATH",
            "APPDATA",
            "LOCALAPPDATA",
        ] {
            cmd.env(k, fake.path());
        }
        let out = cmd.output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
        assert!(stdout.contains("1 passed"), "{stdout}");
    }
}
