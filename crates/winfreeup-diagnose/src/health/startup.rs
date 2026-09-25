//! Danh sách app khởi động cùng máy (spec 4): khóa `Run` của HKCU/HKLM (cả nhánh 32 bit) và hai thư mục
//! Startup. Bật/tắt ghi vào `Explorer\StartupApproved\{Run,Run32,StartupFolder}` — đúng cơ chế Task Manager:
//! chỉ đổi bit trạng thái và dấu thời gian của giá trị StartupApproved, không bao giờ đụng tới giá trị `Run`
//! hay file trong thư mục Startup, nên bật lại được bất cứ lúc nào. Mỗi lần đổi ghi nhật ký TRƯỚC khi ghi
//! registry: không ghi được nhật ký thì không đổi.
use std::os::windows::fs::FileTypeExt;
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegEnumValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE, KEY_WOW64_64KEY, REG_BINARY, REG_EXPAND_SZ, REG_OPTION_NON_VOLATILE, REG_SZ,
};

use crate::actlog::ActionLog;
use crate::util::{from_wide, wide};

pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const RUN32_KEY: &str = r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";
pub const APPROVED_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Hive {
    CurrentUser,
    LocalMachine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupSource {
    HkcuRun,
    HklmRun,
    HklmRun32,
    UserFolder,
    CommonFolder,
}

pub const ALL_SOURCES: [StartupSource; 5] =
    [StartupSource::HkcuRun, StartupSource::HklmRun, StartupSource::HklmRun32, StartupSource::UserFolder, StartupSource::CommonFolder];

impl StartupSource {
    pub fn code(self) -> &'static str {
        match self {
            StartupSource::HkcuRun => "hkcu_run",
            StartupSource::HklmRun => "hklm_run",
            StartupSource::HklmRun32 => "hklm_run32",
            StartupSource::UserFolder => "user_folder",
            StartupSource::CommonFolder => "common_folder",
        }
    }

    /// Khóa StartupApproved tương ứng, đúng như Task Manager dùng.
    pub fn approved(self) -> (Hive, String) {
        let (hive, sub) = match self {
            StartupSource::HkcuRun => (Hive::CurrentUser, "Run"),
            StartupSource::HklmRun => (Hive::LocalMachine, "Run"),
            StartupSource::HklmRun32 => (Hive::LocalMachine, "Run32"),
            StartupSource::UserFolder => (Hive::CurrentUser, "StartupFolder"),
            StartupSource::CommonFolder => (Hive::LocalMachine, "StartupFolder"),
        };
        (hive, format!(r"{APPROVED_KEY}\{sub}"))
    }
}

/// Byte đầu chẵn (02, 06) = bật, lẻ (03, 07) = tắt. Không có giá trị = bật (chưa ai tắt lần nào).
pub fn is_enabled(approved: Option<&[u8]>) -> bool {
    approved.and_then(|d| d.first()).is_none_or(|b| b & 1 == 0)
}

/// Giá trị 12 byte Task Manager ghi: 4 byte cờ + FILETIME lúc tắt (bật thì 0).
pub fn approved_bytes(enabled: bool, now_filetime: u64) -> [u8; 12] {
    let mut v = [0u8; 12];
    v[0] = if enabled { 0x02 } else { 0x03 };
    if !enabled {
        v[4..12].copy_from_slice(&now_filetime.to_le_bytes());
    }
    v
}

/// Giá trị mới khi bật/tắt: giá trị cũ đúng khuôn (12 byte, byte đầu 02/03/06/07) ⇒ chỉ lật bit trạng thái,
/// giữ các bit/byte cờ khác, đặt dấu thời gian như Task Manager (tắt = FILETIME lúc tắt, bật = 0). Chưa có
/// giá trị hoặc giá trị lạ ⇒ đúng khuôn `approved_bytes`.
pub fn updated_approved(old: Option<&[u8]>, enabled: bool, now_filetime: u64) -> [u8; 12] {
    let mut v = approved_bytes(enabled, now_filetime);
    if let Some(old) = old.filter(|o| o.len() == 12 && matches!(o[0], 0x02 | 0x03 | 0x06 | 0x07)) {
        v[0] = (old[0] & !1) | u8::from(!enabled);
        v[1..4].copy_from_slice(&old[1..4]);
    }
    v
}

fn filetime_now() -> u64 {
    let unix_100ns = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() / 100).unwrap_or(0) as u64;
    unix_100ns + 116_444_736_000_000_000
}

/// Tên đưa vào nhật ký: ký tự điều khiển (xuống dòng…) thay bằng `?` để tên giá trị registry do app khác đặt
/// không giả được dòng nhật ký.
fn log_safe(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { '?' } else { c }).collect()
}

/// Giá trị `GetDriveTypeW` (WindowsProgramming — feature chưa bật trong Cargo.toml nên khai tại chỗ).
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;

/// Tiến trình Admin chỉ đọc thư mục Startup nằm trên ổ cục bộ (ổ cứng / ổ rời có chữ cái): đường UNC, đường
/// thiết bị, ổ mạng gắn chữ cái… ⇒ từ chối trước khi chạm tới, để không bao giờ tự kết nối SMB ra ngoài.
fn local_folder_check(dir: &Path, drive_type: impl Fn(&Path) -> u32) -> Result<(), String> {
    let letter = match dir.components().next() {
        Some(Component::Prefix(p)) => match p.kind() {
            Prefix::Disk(l) | Prefix::VerbatimDisk(l) => Some(l.to_ascii_uppercase()),
            _ => None,
        },
        _ => None,
    };
    let Some(letter) = letter else {
        return Err(format!("{}: not on a local drive, not read", dir.display()));
    };
    match drive_type(&PathBuf::from(format!("{}:\\", letter as char))) {
        DRIVE_FIXED | DRIVE_REMOVABLE => Ok(()),
        t => Err(format!("{}: drive type {t} is not a local disk, not read", dir.display())),
    }
}

fn real_drive_type(root: &Path) -> u32 {
    let w = wide(root);
    // SAFETY: w là chuỗi kết thúc NUL còn sống trong suốt lời gọi.
    unsafe { GetDriveTypeW(w.as_ptr()) }
}

/// Thao tác registry cần cho danh sách khởi động — bản thật `WinRegistry`, test dùng bản giả trong bộ nhớ.
pub trait Registry: Send + Sync {
    /// Các giá trị chuỗi (REG_SZ/REG_EXPAND_SZ, chưa mở rộng biến) của một khóa. Khóa không có ⇒ danh sách rỗng.
    fn string_values(&self, hive: Hive, key: &str) -> Result<Vec<(String, String)>, String>;
    /// Khóa hoặc giá trị không có ⇒ `None`.
    fn binary_value(&self, hive: Hive, key: &str, name: &str) -> Result<Option<Vec<u8>>, String>;
    /// Ghi REG_BINARY, tạo khóa nếu chưa có.
    fn set_binary(&self, hive: Hive, key: &str, name: &str, data: &[u8]) -> Result<(), String>;
}

pub struct WinRegistry;

fn root(h: Hive) -> HKEY {
    match h {
        Hive::CurrentUser => HKEY_CURRENT_USER,
        Hive::LocalMachine => HKEY_LOCAL_MACHINE,
    }
}

fn hive_name(h: Hive) -> &'static str {
    match h {
        Hive::CurrentUser => "HKCU",
        Hive::LocalMachine => "HKLM",
    }
}

fn reg_err(what: &str, hive: Hive, key: &str, code: u32) -> String {
    format!(r"{what} {}\{key}: {}", hive_name(hive), std::io::Error::from_raw_os_error(code as i32))
}

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: self.0 là khóa do RegOpenKeyExW/RegCreateKeyExW mở thành công, đóng đúng một lần.
        unsafe { RegCloseKey(self.0) };
    }
}

fn open_read(hive: Hive, key: &str) -> Result<Option<Key>, String> {
    let w = wide(key);
    let mut h: HKEY = std::ptr::null_mut();
    // SAFETY: chuỗi kết thúc NUL; h là biến cục bộ khả ghi.
    let rc = unsafe { RegOpenKeyExW(root(hive), w.as_ptr(), 0, KEY_READ | KEY_WOW64_64KEY, &mut h) };
    match rc {
        ERROR_SUCCESS => Ok(Some(Key(h))),
        ERROR_FILE_NOT_FOUND => Ok(None),
        rc => Err(reg_err("RegOpenKeyExW", hive, key, rc)),
    }
}

impl Registry for WinRegistry {
    fn string_values(&self, hive: Hive, key: &str) -> Result<Vec<(String, String)>, String> {
        let Some(k) = open_read(hive, key)? else { return Ok(Vec::new()) };
        let mut out = Vec::new();
        // Tên giá trị tối đa 16 383 ký tự; dữ liệu thiếu chỗ ⇒ ERROR_MORE_DATA kèm kích thước cần, thử lại.
        let mut name = vec![0u16; 16_384];
        let mut data = vec![0u8; 64 * 1024];
        let mut i = 0u32;
        loop {
            let mut name_len = name.len() as u32;
            let mut ty = 0u32;
            let mut data_len = data.len() as u32;
            // SAFETY: name/data khả ghi đúng độ dài truyền vào; các con trỏ còn lại là biến cục bộ hoặc null.
            let rc = unsafe {
                RegEnumValueW(k.0, i, name.as_mut_ptr(), &mut name_len, std::ptr::null(), &mut ty, data.as_mut_ptr(), &mut data_len)
            };
            match rc {
                ERROR_NO_MORE_ITEMS => break,
                ERROR_MORE_DATA if data_len as usize > data.len() => {
                    data.resize(data_len as usize, 0);
                    continue;
                }
                ERROR_SUCCESS => {}
                rc => return Err(reg_err("RegEnumValueW", hive, key, rc)),
            }
            if ty == REG_SZ || ty == REG_EXPAND_SZ {
                let units: Vec<u16> = data[..data_len as usize].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                out.push((String::from_utf16_lossy(&name[..name_len as usize]), from_wide(&units)));
            }
            i += 1;
        }
        Ok(out)
    }

    fn binary_value(&self, hive: Hive, key: &str, name: &str) -> Result<Option<Vec<u8>>, String> {
        let Some(k) = open_read(hive, key)? else { return Ok(None) };
        let n = wide(name);
        let mut data = vec![0u8; 256];
        loop {
            let mut len = data.len() as u32;
            // SAFETY: n kết thúc NUL; data khả ghi đúng len byte.
            let rc = unsafe { RegQueryValueExW(k.0, n.as_ptr(), std::ptr::null(), std::ptr::null_mut(), data.as_mut_ptr(), &mut len) };
            match rc {
                ERROR_SUCCESS => {
                    data.truncate(len as usize);
                    return Ok(Some(data));
                }
                ERROR_MORE_DATA if len as usize > data.len() => data.resize(len as usize, 0),
                ERROR_FILE_NOT_FOUND => return Ok(None),
                rc => return Err(reg_err("RegQueryValueExW", hive, key, rc)),
            }
        }
    }

    fn set_binary(&self, hive: Hive, key: &str, name: &str, data: &[u8]) -> Result<(), String> {
        let w = wide(key);
        let mut h: HKEY = std::ptr::null_mut();
        // SAFETY: chuỗi kết thúc NUL; h là biến cục bộ khả ghi; lớp/bảo mật/disposition không dùng (null).
        let rc = unsafe {
            RegCreateKeyExW(
                root(hive),
                w.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE | KEY_WOW64_64KEY,
                std::ptr::null(),
                &mut h,
                std::ptr::null_mut(),
            )
        };
        if rc != ERROR_SUCCESS {
            return Err(reg_err("RegCreateKeyExW", hive, key, rc));
        }
        let k = Key(h);
        let n = wide(name);
        // SAFETY: n kết thúc NUL; data còn sống, đúng data.len() byte.
        let rc = unsafe { RegSetValueExW(k.0, n.as_ptr(), 0, REG_BINARY, data.as_ptr(), data.len() as u32) };
        if rc != ERROR_SUCCESS {
            return Err(reg_err("RegSetValueExW", hive, key, rc));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StartupEntry {
    /// `<nguồn>:<tên>` — duy nhất trong danh sách.
    pub id: String,
    pub source: StartupSource,
    pub name: String,
    /// Dòng lệnh (khóa Run) hoặc đường dẫn file (thư mục Startup).
    pub command: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StartupList {
    pub entries: Vec<StartupEntry>,
    /// Nguồn không đọc được (nguyên văn) — các nguồn khác vẫn liệt kê.
    pub errors: Vec<String>,
}

pub struct Startup {
    reg: Box<dyn Registry>,
    /// `Err` = không xác định được thư mục; chỉ nguồn đó báo lỗi, các nguồn khác vẫn liệt kê.
    user_folder: Result<PathBuf, String>,
    common_folder: Result<PathBuf, String>,
    log: Arc<ActionLog>,
}

impl Startup {
    pub fn new(reg: Box<dyn Registry>, user_folder: PathBuf, common_folder: PathBuf, log: Arc<ActionLog>) -> Self {
        Startup { reg, user_folder: Ok(user_folder), common_folder: Ok(common_folder), log }
    }

    /// Registry thật và hai thư mục Startup lấy qua `util` (registry/API), không qua biến môi trường.
    /// Luôn `Ok`: không xác định được thư mục nào thì lỗi đó nằm trong `list().errors` của nguồn tương ứng.
    pub fn from_system(log: Arc<ActionLog>) -> Result<Self, String> {
        Ok(Startup {
            reg: Box::new(WinRegistry),
            user_folder: crate::util::user_startup(),
            common_folder: crate::util::common_startup(),
            log,
        })
    }

    fn raw(&self, source: StartupSource) -> Result<Vec<(String, String)>, String> {
        match source {
            StartupSource::HkcuRun => self.reg.string_values(Hive::CurrentUser, RUN_KEY),
            StartupSource::HklmRun => self.reg.string_values(Hive::LocalMachine, RUN_KEY),
            StartupSource::HklmRun32 => self.reg.string_values(Hive::LocalMachine, RUN32_KEY),
            StartupSource::UserFolder | StartupSource::CommonFolder => {
                let dir = if source == StartupSource::UserFolder { &self.user_folder } else { &self.common_folder };
                let dir = dir.as_ref().map_err(Clone::clone)?;
                local_folder_check(dir, real_drive_type)?;
                match std::fs::symlink_metadata(dir) {
                    Ok(m) if m.file_type().is_symlink() => {
                        return Err(format!("{}: is a symlink or junction (reparse point), not followed", dir.display()))
                    }
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
                    Err(e) => return Err(format!("{}: {e}", dir.display())),
                }
                let rd = match std::fs::read_dir(dir) {
                    Ok(rd) => rd,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
                    Err(e) => return Err(format!("{}: {e}", dir.display())),
                };
                let mut v = Vec::new();
                for e in rd {
                    let e = e.map_err(|e| format!("{}: {e}", dir.display()))?;
                    let name = e.file_name().to_string_lossy().to_string();
                    // File thường hoặc symlink file (không đi theo đích); thư mục và junction/symlink thư mục thì bỏ.
                    let is_file = e.file_type().map(|t| t.is_file() || t.is_symlink_file()).unwrap_or(false);
                    if is_file && !name.eq_ignore_ascii_case("desktop.ini") {
                        v.push((name, e.path().display().to_string()));
                    }
                }
                v.sort();
                Ok(v)
            }
        }
    }

    pub fn list(&self) -> StartupList {
        let mut out = StartupList { entries: Vec::new(), errors: Vec::new() };
        for source in ALL_SOURCES {
            let (hive, key) = source.approved();
            match self.raw(source) {
                Err(e) => out.errors.push(e),
                Ok(items) => {
                    for (name, command) in items {
                        // Không đọc được trạng thái ⇒ báo lỗi (một lần mỗi khóa), không đoán «đang bật» trong im lặng.
                        let approved = self.reg.binary_value(hive, &key, &name).unwrap_or_else(|e| {
                            if !out.errors.contains(&e) {
                                out.errors.push(e);
                            }
                            None
                        });
                        out.entries.push(StartupEntry {
                            id: format!("{}:{name}", source.code()),
                            source,
                            name,
                            command,
                            enabled: is_enabled(approved.as_deref()),
                        });
                    }
                }
            }
        }
        out
    }

    /// Số app đang bật (cho dòng `startup_apps` của khám nhanh). Nguồn nào lỗi thì cả số là «không đo được».
    pub fn enabled_count(&self) -> Result<u32, String> {
        let l = self.list();
        if let Some(e) = l.errors.first() {
            return Err(e.clone());
        }
        Ok(l.entries.iter().filter(|e| e.enabled).count() as u32)
    }

    /// Bật/tắt một mục theo `id` lấy từ `list()` (tên thật đọc lại từ registry/thư mục, không tin chuỗi giao
    /// diện gửi lên). Đã đúng trạng thái ⇒ không ghi gì. Ghi nhật ký trước; không ghi được nhật ký ⇒ không đổi.
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<StartupEntry, String> {
        let entry = self.list().entries.into_iter().find(|e| e.id == id).ok_or("unknown_entry")?;
        let (hive, key) = entry.source.approved();
        let old = self.reg.binary_value(hive, &key, &entry.name)?;
        if is_enabled(old.as_deref()) == enabled {
            return Ok(StartupEntry { enabled, ..entry });
        }
        let action = if enabled { "STARTUP_ENABLE" } else { "STARTUP_DISABLE" };
        let what = format!("{action} {} {}", entry.source.code(), log_safe(&entry.name));
        self.log.line(&what)?;
        if let Err(e) = self.reg.set_binary(hive, &key, &entry.name, &updated_approved(old.as_deref(), enabled, filetime_now())) {
            // Dòng trên đã ghi ý định; ghi thêm thất bại để nhật ký không nói sai. Lỗi gốc vẫn trả về cho giao diện.
            let _ = self.log.line(&format!("{what} FAILED: {}", log_safe(&e)));
            return Err(e);
        }
        Ok(StartupEntry { enabled, ..entry })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeReg {
        strings: HashMap<(Hive, String), Vec<(String, String)>>,
        binaries: Mutex<HashMap<(Hive, String, String), Vec<u8>>>,
        fail_key: Option<String>,
        fail_binary_key: Option<String>,
        writes: Mutex<u32>,
    }

    impl Registry for FakeReg {
        fn string_values(&self, hive: Hive, key: &str) -> Result<Vec<(String, String)>, String> {
            if self.fail_key.as_deref() == Some(key) {
                return Err(format!("RegOpenKeyExW {key}: Access is denied. (os error 5)"));
            }
            Ok(self.strings.get(&(hive, key.to_string())).cloned().unwrap_or_default())
        }
        fn binary_value(&self, hive: Hive, key: &str, name: &str) -> Result<Option<Vec<u8>>, String> {
            if self.fail_binary_key.as_deref() == Some(key) {
                return Err(format!("RegOpenKeyExW {key}: Access is denied. (os error 5)"));
            }
            Ok(self.binaries.lock().unwrap().get(&(hive, key.to_string(), name.to_string())).cloned())
        }
        fn set_binary(&self, hive: Hive, key: &str, name: &str, data: &[u8]) -> Result<(), String> {
            *self.writes.lock().unwrap() += 1;
            self.binaries.lock().unwrap().insert((hive, key.to_string(), name.to_string()), data.to_vec());
            Ok(())
        }
    }

    /// Bản giả dùng chung dữ liệu với test (để test nhìn thấy byte đã ghi sau khi `Startup` giữ bản giả).
    struct Shared(Arc<FakeReg>);

    impl Registry for Shared {
        fn string_values(&self, hive: Hive, key: &str) -> Result<Vec<(String, String)>, String> {
            self.0.string_values(hive, key)
        }
        fn binary_value(&self, hive: Hive, key: &str, name: &str) -> Result<Option<Vec<u8>>, String> {
            self.0.binary_value(hive, key, name)
        }
        fn set_binary(&self, hive: Hive, key: &str, name: &str, data: &[u8]) -> Result<(), String> {
            self.0.set_binary(hive, key, name, data)
        }
    }

    fn setup_with_log(reg: Box<dyn Registry>, log_dir: impl FnOnce(&Path) -> PathBuf) -> (tempfile::TempDir, Startup) {
        let t = tempfile::tempdir().unwrap();
        let user = t.path().join("user");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(user.join("Zalo.lnk"), "x").unwrap();
        std::fs::write(user.join("desktop.ini"), "x").unwrap();
        let logs = log_dir(t.path());
        let s = Startup::new(reg, user, t.path().join("common-khong-co"), Arc::new(ActionLog::new(logs)));
        (t, s)
    }

    fn setup(reg: FakeReg) -> (tempfile::TempDir, Startup) {
        setup_with_log(Box::new(reg), |t| t.join("logs"))
    }

    fn reg_with_run() -> FakeReg {
        let mut r = FakeReg::default();
        r.strings.insert((Hive::CurrentUser, RUN_KEY.into()), vec![("OneDrive".into(), r#""C:\OneDrive.exe" /background"#.into())]);
        r.strings.insert((Hive::LocalMachine, RUN32_KEY.into()), vec![("Unikey".into(), r"C:\Unikey\UniKeyNT.exe".into())]);
        r.binaries
            .lock()
            .unwrap()
            .insert((Hive::LocalMachine, format!(r"{APPROVED_KEY}\Run32"), "Unikey".into()), vec![3, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]);
        r
    }

    fn read_log(dir: &Path) -> String {
        let log = std::fs::read_dir(dir).unwrap().next().unwrap().unwrap().path();
        std::fs::read_to_string(log).unwrap()
    }

    /// FILETIME của 2020-01-01 — mốc 1601, để phân biệt với giây Unix.
    const FT_2020: u64 = 132_223_104_000_000_000;

    #[test]
    fn approved_flag_parity_decides_enabled() {
        assert!(is_enabled(None));
        assert!(is_enabled(Some(&[])));
        assert!(is_enabled(Some(&[2, 0, 0, 0])));
        assert!(is_enabled(Some(&[6])));
        assert!(!is_enabled(Some(&[3])));
        assert!(!is_enabled(Some(&[7])));
        let off = approved_bytes(false, 0x0102_0304_0506_0708);
        assert_eq!(off[0], 3);
        assert_eq!(&off[4..], &0x0102_0304_0506_0708u64.to_le_bytes());
        assert_eq!(approved_bytes(true, 99), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn toggling_changes_only_the_state_bit_and_the_timestamp() {
        let now = 0x01DC_0000_1111_2222u64;
        // Chưa có giá trị / giá trị lạ ⇒ đúng khuôn Task Manager.
        assert_eq!(updated_approved(None, false, now), approved_bytes(false, now));
        assert_eq!(updated_approved(Some(&[]), true, now), approved_bytes(true, now));
        assert_eq!(updated_approved(Some(&[0x03, 0, 0]), true, now), approved_bytes(true, now));
        assert_eq!(updated_approved(Some(&[0x05; 12]), false, now), approved_bytes(false, now));
        // Có sẵn 12 byte: giữ bit khác của byte trạng thái (06 ↔ 07) và ba byte cờ còn lại.
        let old = [0x06, 0xAA, 0xBB, 0xCC, 9, 9, 9, 9, 9, 9, 9, 9];
        let off = updated_approved(Some(&old), false, now);
        assert_eq!(&off[..4], &[0x07, 0xAA, 0xBB, 0xCC]);
        assert_eq!(&off[4..], &now.to_le_bytes(), "tắt ⇒ FILETIME lúc tắt");
        let on = updated_approved(Some(&off), true, now);
        assert_eq!(on, [0x06, 0xAA, 0xBB, 0xCC, 0, 0, 0, 0, 0, 0, 0, 0], "bật ⇒ dấu thời gian về 0 như Task Manager");
        assert!(is_enabled(Some(&on)) && !is_enabled(Some(&off)));
        let now_ft = filetime_now();
        assert!(now_ft > FT_2020, "{now_ft}");
    }

    #[test]
    fn lists_every_source_with_its_state_and_skips_desktop_ini() {
        let (_t, s) = setup(reg_with_run());
        let l = s.list();
        assert!(l.errors.is_empty());
        let ids: Vec<_> = l.entries.iter().map(|e| (e.id.as_str(), e.enabled)).collect();
        assert_eq!(ids, vec![("hkcu_run:OneDrive", true), ("hklm_run32:Unikey", false), ("user_folder:Zalo.lnk", true)]);
        assert_eq!(s.enabled_count().unwrap(), 2);
    }

    #[test]
    fn toggling_writes_startup_approved_like_task_manager_and_logs() {
        let (t, s) = setup(reg_with_run());
        let e = s.set_enabled("hkcu_run:OneDrive", false).unwrap();
        assert!(!e.enabled);
        assert!(!s.list().entries.iter().find(|e| e.id == "hkcu_run:OneDrive").unwrap().enabled);
        s.set_enabled("user_folder:Zalo.lnk", false).unwrap();
        s.set_enabled("hkcu_run:OneDrive", true).unwrap();
        assert_eq!(s.enabled_count().unwrap(), 1);
        let text = read_log(&t.path().join("logs"));
        assert!(text.contains("STARTUP_DISABLE hkcu_run OneDrive"));
        assert!(text.contains("STARTUP_ENABLE hkcu_run OneDrive"));
        assert!(text.contains("STARTUP_DISABLE user_folder Zalo.lnk"));
        assert_eq!(s.set_enabled("hkcu_run:KhongCo", false).unwrap_err(), "unknown_entry");
    }

    #[test]
    fn toggling_writes_only_startup_approved_never_the_run_value() {
        let reg = Arc::new(reg_with_run());
        let (_t, s) = setup_with_log(Box::new(Shared(reg.clone())), |t| t.join("logs"));
        s.set_enabled("hklm_run32:Unikey", true).unwrap();
        {
            let b = reg.binaries.lock().unwrap();
            assert_eq!(b.len(), 1, "chỉ đúng một giá trị StartupApproved: {b:?}");
            assert_eq!(b[&(Hive::LocalMachine, format!(r"{APPROVED_KEY}\Run32"), "Unikey".into())], approved_bytes(true, 0));
        }
        assert_eq!(reg.strings[&(Hive::LocalMachine, RUN32_KEY.to_string())].len(), 1, "giá trị Run gốc còn nguyên");
        s.set_enabled("hkcu_run:OneDrive", false).unwrap();
        let b = reg.binaries.lock().unwrap();
        let off = &b[&(Hive::CurrentUser, format!(r"{APPROVED_KEY}\Run"), "OneDrive".into())];
        assert_eq!(&off[..4], &[3, 0, 0, 0]);
        let ft = u64::from_le_bytes(off[4..12].try_into().unwrap());
        assert!(ft > FT_2020, "FILETIME lúc tắt: {ft}");
    }

    #[test]
    fn setting_the_current_state_writes_and_logs_nothing() {
        let reg = Arc::new(reg_with_run());
        let (t, s) = setup_with_log(Box::new(Shared(reg.clone())), |t| t.join("logs"));
        assert!(s.set_enabled("hkcu_run:OneDrive", true).unwrap().enabled);
        assert!(!s.set_enabled("hklm_run32:Unikey", false).unwrap().enabled);
        assert_eq!(*reg.writes.lock().unwrap(), 0);
        assert!(!t.path().join("logs").exists(), "không đổi gì thì không có nhật ký");
    }

    #[test]
    fn no_change_without_a_log_line() {
        let reg = Arc::new(reg_with_run());
        // Thư mục nhật ký thực ra là một file ⇒ không ghi được nhật ký.
        let (_t, s) = setup_with_log(Box::new(Shared(reg.clone())), |t| {
            let p = t.join("logs-la-file");
            std::fs::write(&p, "x").unwrap();
            p
        });
        let err = s.set_enabled("hkcu_run:OneDrive", false).unwrap_err();
        assert!(err.contains("logs-la-file"), "{err}");
        assert_eq!(*reg.writes.lock().unwrap(), 0, "không ghi được nhật ký thì không được đổi registry");
        assert!(s.list().entries.iter().find(|e| e.id == "hkcu_run:OneDrive").unwrap().enabled);
    }

    #[test]
    fn control_characters_in_names_cannot_forge_log_lines() {
        let mut r = FakeReg::default();
        let evil = "Evil\r\n2026-01-01 00:00:00 STARTUP_ENABLE x";
        r.strings.insert((Hive::CurrentUser, RUN_KEY.into()), vec![(evil.into(), "a.exe".into())]);
        let (t, s) = setup(r);
        s.set_enabled(&format!("hkcu_run:{evil}"), false).unwrap();
        let text = read_log(&t.path().join("logs"));
        assert_eq!(text.lines().count(), 1, "tên không được tách thành dòng nhật ký thứ hai: {text:?}");
        assert!(text.contains("STARTUP_DISABLE hkcu_run Evil??2026-01-01"), "{text:?}");
    }

    #[test]
    fn one_unreadable_source_does_not_hide_the_others() {
        let mut r = reg_with_run();
        r.fail_key = Some(RUN32_KEY.into());
        let (_t, s) = setup(r);
        let l = s.list();
        assert_eq!(l.errors.len(), 1);
        assert_eq!(l.entries.len(), 2);
        assert!(s.enabled_count().is_err(), "đếm thiếu thì phải là «không đo được», không phải một con số sai");
    }

    #[test]
    fn unreadable_startup_approved_is_reported_not_guessed() {
        let mut r = reg_with_run();
        r.fail_binary_key = Some(format!(r"{APPROVED_KEY}\Run"));
        let (_t, s) = setup(r);
        let l = s.list();
        assert_eq!(l.entries.len(), 3, "vẫn liệt kê đủ");
        assert_eq!(l.errors.len(), 1, "{:?}", l.errors);
        assert!(l.errors[0].contains("StartupApproved"), "{:?}", l.errors);
        assert!(s.enabled_count().is_err());
        let err = s.set_enabled("hkcu_run:OneDrive", false).unwrap_err();
        assert!(err.contains("Access is denied"), "không đọc được trạng thái cũ thì không ghi đè: {err}");
    }

    #[test]
    fn startup_folders_off_the_local_disks_are_never_read() {
        let asked = std::cell::RefCell::new(Vec::new());
        let types = |root: &Path| {
            asked.borrow_mut().push(root.to_path_buf());
            match root.to_string_lossy().as_ref() {
                r"C:\" => 3, // DRIVE_FIXED
                r"E:\" => 2, // DRIVE_REMOVABLE
                r"Z:\" => 4, // DRIVE_REMOTE (ổ mạng gắn chữ cái)
                _ => 5,      // DRIVE_CDROM
            }
        };
        assert_eq!(local_folder_check(Path::new(r"C:\Users\an\Startup"), types), Ok(()));
        assert_eq!(local_folder_check(Path::new(r"\\?\C:\Users\an\Startup"), types), Ok(()));
        assert_eq!(local_folder_check(Path::new(r"e:\Startup"), types), Ok(()));
        assert_eq!(asked.borrow().last().unwrap(), Path::new(r"E:\"));
        let n = asked.borrow().len();
        for p in [
            r"\\may-chu\share\Startup",
            r"\\?\UNC\may-chu\share\Startup",
            r"\\.\pipe\x",
            r"\\?\Volume{12345678-0000-0000-0000-000000000000}\Startup",
            r"Startup",
            r"\Users\an\Startup",
        ] {
            let err = local_folder_check(Path::new(p), types).unwrap_err();
            assert!(err.contains(p) && err.contains("not on a local drive"), "{err}");
        }
        assert_eq!(asked.borrow().len(), n, "đường UNC/thiết bị bị từ chối trước khi hỏi loại ổ");
        for p in [r"Z:\Startup", r"D:\Startup"] {
            let err = local_folder_check(Path::new(p), types).unwrap_err();
            assert!(err.contains(p) && err.contains("drive type"), "{err}");
        }
    }

    #[test]
    fn a_network_startup_folder_is_an_error_of_that_source_only() {
        let t = tempfile::tempdir().unwrap();
        let s = Startup {
            reg: Box::new(reg_with_run()),
            user_folder: Ok(PathBuf::from(r"\\may-chu-khong-co.invalid\share\Startup")),
            common_folder: Ok(t.path().join("khong-co")),
            log: Arc::new(ActionLog::new(t.path().join("logs"))),
        };
        let l = s.list();
        assert_eq!(l.errors.len(), 1, "{:?}", l.errors);
        assert!(l.errors[0].contains(r"\\may-chu-khong-co.invalid"), "{:?}", l.errors);
        assert_eq!(l.entries.len(), 2);
    }

    /// Symlink file trong thư mục Startup vẫn được liệt kê (không đi theo nó); symlink/junction thư mục thì không.
    /// Tạo symlink file cần SeCreateSymbolicLink hoặc Developer Mode — không có thì bỏ qua phần đó (in lý do).
    #[test]
    fn file_symlinks_are_listed_without_being_followed_and_dir_links_are_not() {
        let (t, s) = setup(FakeReg::default());
        let user = t.path().join("user");
        junction::create(t.path(), user.join("ThuMucNoi")).unwrap();
        std::fs::create_dir(user.join("ThuMucThat")).unwrap();
        if let Err(e) = std::os::windows::fs::symlink_file(r"\\may-chu-khong-co.invalid\x\app.exe", user.join("Mang.lnk")) {
            eprintln!("bỏ qua phần symlink file: không tạo được symlink không cần quyền ({e})");
            let names: Vec<_> = s.list().entries.into_iter().map(|e| e.name).collect();
            assert_eq!(names, vec!["Zalo.lnk"]);
            return;
        }
        let names: Vec<_> = s.list().entries.into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["Mang.lnk", "Zalo.lnk"]);
    }

    #[test]
    fn a_startup_folder_that_is_itself_a_junction_is_not_followed() {
        let t = tempfile::tempdir().unwrap();
        let target = t.path().join("dich");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("An.lnk"), "x").unwrap();
        let link = t.path().join("Startup");
        junction::create(&target, &link).unwrap();
        let log = Arc::new(ActionLog::new(t.path().join("logs")));
        let s = Startup::new(Box::new(FakeReg::default()), link, t.path().join("khong-co"), log);
        let l = s.list();
        assert!(l.entries.is_empty(), "{:?}", l.entries);
        assert_eq!(l.errors.len(), 1, "{:?}", l.errors);
        assert!(l.errors[0].contains("reparse point"), "{:?}", l.errors);
    }

    #[test]
    fn an_unreadable_startup_folder_is_one_error_not_a_total_failure() {
        let t = tempfile::tempdir().unwrap();
        let s = Startup {
            reg: Box::new(reg_with_run()),
            user_folder: Err(r"HKCU\...\Startup: Access is denied. (os error 5)".into()),
            common_folder: Ok(t.path().join("khong-co")),
            log: Arc::new(ActionLog::new(t.path().join("logs"))),
        };
        let l = s.list();
        assert_eq!(l.errors, vec![r"HKCU\...\Startup: Access is denied. (os error 5)".to_string()]);
        assert_eq!(l.entries.len(), 2);
    }

    #[test]
    fn real_registry_listing_reads_without_error() {
        let log = Arc::new(ActionLog::new(std::env::temp_dir().join("wfu-khong-ghi")));
        let s = Startup::from_system(log).unwrap();
        assert_eq!(s.user_folder.as_ref().unwrap(), &crate::util::user_startup().unwrap());
        assert_eq!(s.common_folder.as_ref().unwrap(), &crate::util::common_startup().unwrap());
        let l = s.list();
        assert!(l.errors.is_empty(), "{:?}", l.errors);
        assert!(l.entries.iter().all(|e| !e.name.is_empty()));
    }

    const TEST_ROOT: &str = r"Software\WinFreeUpTest";

    /// Khóa thử `HKCU\Software\WinFreeUpTest\<tên duy nhất>`; xóa cả cây khi guard bị drop (kể cả khi test hỏng).
    struct TestKey(String);

    impl TestKey {
        fn new(tag: &str) -> TestKey {
            let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            TestKey(format!(r"{TEST_ROOT}\startup-{tag}-{}-{nanos}", std::process::id()))
        }

        fn set(&self, name: &str, ty: u32, data: &[u8]) {
            let w = wide(&self.0);
            let mut h: HKEY = std::ptr::null_mut();
            // SAFETY: chuỗi kết thúc NUL; h là biến cục bộ, đóng qua `Key`.
            let rc = unsafe {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    w.as_ptr(),
                    0,
                    std::ptr::null(),
                    REG_OPTION_NON_VOLATILE,
                    KEY_SET_VALUE,
                    std::ptr::null(),
                    &mut h,
                    std::ptr::null_mut(),
                )
            };
            assert_eq!(rc, ERROR_SUCCESS);
            let k = Key(h);
            let n = wide(name);
            // SAFETY: data còn sống, đúng data.len() byte.
            let rc = unsafe { RegSetValueExW(k.0, n.as_ptr(), 0, ty, data.as_ptr(), data.len() as u32) };
            assert_eq!(rc, ERROR_SUCCESS);
        }

        fn set_str(&self, name: &str, ty: u32, value: &str) {
            let bytes: Vec<u8> = wide(value).iter().flat_map(|u| u.to_le_bytes()).collect();
            self.set(name, ty, &bytes);
        }
    }

    impl Drop for TestKey {
        fn drop(&mut self) {
            use windows_sys::Win32::System::Registry::{RegDeleteKeyW, RegDeleteTreeW};
            if !self.0.starts_with(&format!(r"{TEST_ROOT}\")) {
                return;
            }
            let w = wide(&self.0);
            let root = wide(TEST_ROOT);
            // SAFETY: chuỗi kết thúc NUL, khóa gốc HKCU luôn mở. Khóa cha chỉ bị xóa khi đã rỗng
            // (RegDeleteKeyW từ chối khóa còn khóa con — test khác đang chạy song song vẫn giữ khóa của nó).
            unsafe {
                RegDeleteTreeW(HKEY_CURRENT_USER, w.as_ptr());
                RegDeleteKeyW(HKEY_CURRENT_USER, w.as_ptr());
                RegDeleteKeyW(HKEY_CURRENT_USER, root.as_ptr());
            }
        }
    }

    #[test]
    fn win_registry_reads_strings_and_round_trips_binaries_in_a_test_key() {
        use windows_sys::Win32::System::Registry::REG_DWORD;
        let k = TestKey::new("rw");
        let long = "x".repeat(40_000); // 80 KB UTF-16: vượt bộ đệm 64 KB ban đầu của RegEnumValueW
        k.set_str("OneDrive", REG_SZ, r#""C:\OneDrive.exe" /background"#);
        k.set_str("Khởi động", REG_EXPAND_SZ, r"%ProgramFiles%\Tải về\a.exe");
        k.set_str("Dai", REG_SZ, &long);
        k.set("So", REG_DWORD, &1u32.to_le_bytes());
        let mut v = WinRegistry.string_values(Hive::CurrentUser, &k.0).unwrap();
        v.sort();
        assert_eq!(
            v,
            vec![
                ("Dai".to_string(), long.clone()),
                ("Khởi động".to_string(), r"%ProgramFiles%\Tải về\a.exe".to_string()),
                ("OneDrive".to_string(), r#""C:\OneDrive.exe" /background"#.to_string()),
            ]
        );
        let sub = format!(r"{}\StartupApproved\Run", k.0);
        assert_eq!(WinRegistry.string_values(Hive::CurrentUser, &sub).unwrap(), vec![], "khóa không có ⇒ rỗng");
        assert_eq!(WinRegistry.binary_value(Hive::CurrentUser, &sub, "OneDrive").unwrap(), None);
        let off = approved_bytes(false, filetime_now());
        WinRegistry.set_binary(Hive::CurrentUser, &sub, "OneDrive", &off).unwrap();
        assert_eq!(WinRegistry.binary_value(Hive::CurrentUser, &sub, "OneDrive").unwrap(), Some(off.to_vec()));
        assert_eq!(WinRegistry.binary_value(Hive::CurrentUser, &sub, "KhongCo").unwrap(), None);
        let big = vec![7u8; 1000];
        WinRegistry.set_binary(Hive::CurrentUser, &sub, "To", &big).unwrap();
        assert_eq!(WinRegistry.binary_value(Hive::CurrentUser, &sub, "To").unwrap(), Some(big));
    }
}
