//! Cài đặt thật của SystemOps. Mọi tiến trình con chạy với CREATE_NO_WINDOW (không nháy cửa sổ đen).
//!
//! Ứng dụng luôn chạy quyền Admin nhưng thừa hưởng biến môi trường của người dùng thường, nên:
//! - đường dẫn hệ thống (Windows, System32, ổ hệ thống) lấy qua API chứ không qua `SystemRoot`/`windir`;
//! - tiến trình con (PowerShell, DISM) chạy với môi trường dựng lại từ đầu, không mang biến của người dùng.
use std::ffi::{c_void, OsStr, OsString};
use std::io::{self, Read};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::{Component, Path, PathBuf, Prefix};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows_sys::core::{BOOL, PCWSTR, PWSTR};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, ERROR_ALREADY_EXISTS, ERROR_NOT_ALL_ASSIGNED, HANDLE, INVALID_HANDLE_VALUE,
    LUID,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SetEntriesInAclW,
    EXPLICIT_ACCESS_W, GRANT_ACCESS, NO_MULTIPLE_TRUSTEE, SDDL_REVISION_1, SE_FILE_OBJECT, TRUSTEE_IS_GROUP,
    TRUSTEE_IS_SID, TRUSTEE_W,
};
use windows_sys::Win32::Security::{
    AdjustTokenPrivileges, CreateWellKnownSid, GetSecurityDescriptorControl, InitializeSecurityDescriptor,
    LookupPrivilegeValueW, SetKernelObjectSecurity, SetSecurityDescriptorControl, SetSecurityDescriptorDacl,
    SetSecurityDescriptorOwner, WinBuiltinAdministratorsSid, ACL, DACL_SECURITY_INFORMATION, LUID_AND_ATTRIBUTES,
    NO_INHERITANCE, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR,
    SE_DACL_AUTO_INHERITED, SE_DACL_PROTECTED, SE_BACKUP_NAME, SE_PRIVILEGE_ENABLED, SE_RESTORE_NAME, SE_TAKE_OWNERSHIP_NAME,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateDirectoryW, CreateFileW, FileAttributeTagInfo, GetDiskFreeSpaceExW, GetFileInformationByHandle,
    GetFileInformationByHandleEx, GetFinalPathNameByHandleW, BY_HANDLE_FILE_INFORMATION, FILE_ALL_ACCESS,
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_NAME_NORMALIZED, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING, READ_CONTROL, VOLUME_NAME_DOS, WRITE_DAC, WRITE_OWNER,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, Thread32First, Thread32Next, PROCESSENTRY32W,
    TH32CS_SNAPPROCESS, TH32CS_SNAPTHREAD, THREADENTRY32,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
    TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::SystemInformation::{GetSystemDirectoryW, GetSystemWindowsDirectoryW};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcessToken, OpenThread, ResumeThread, CREATE_SUSPENDED, THREAD_SUSPEND_RESUME,
};
use windows_sys::Win32::UI::Shell::{
    SHEmptyRecycleBinW, SHQueryRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHQUERYRBINFO,
};

use crate::env::{Env, SystemOps};
use crate::error::{io_err, CoreError, Result};
use crate::types::CancelToken;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// SHEmptyRecycleBinW trả E_UNEXPECTED khi thùng rác vốn đã trống.
const E_UNEXPECTED: i32 = 0x8000_FFFFu32 as i32;
/// DISM: 3010 = thành công, cần khởi động lại.
const DISM_OK_REBOOT: i32 = 3010;
/// Mọi script PowerShell xuất UTF-8 để thông điệp lỗi tiếng Việt không vỡ chữ.
const PS_UTF8_PREFIX: &str = "[Console]::OutputEncoding=[Text.Encoding]::UTF8;";
/// SECURITY_DESCRIPTOR_REVISION (nằm ở feature Win32_System_SystemServices, không bật).
const SD_REVISION: u32 = 1;
/// Số thông điệp lỗi giữ lại khi gộp, và độ dài tối đa mỗi thông điệp.
const MAX_SHOWN_ERRORS: usize = 5;
const MAX_ERROR_CHARS: usize = 200;
/// NT SERVICE\TrustedInstaller.
const TRUSTED_INSTALLER_SID: &str = "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464";
/// Thư mục tạm riêng cho DISM: chỉ SYSTEM và Administrators, không kế thừa gì từ cha.
const PRIVATE_DIR_SDDL: &str = "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)";

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

fn check(ok: BOOL) -> io::Result<()> {
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Đọc chuỗi rộng kết thúc NUL.
///
/// # Safety
/// `p` khác null và trỏ tới chuỗi u16 kết thúc bằng NUL còn sống trong suốt lời gọi.
unsafe fn wide_ptr_to_string(p: *const u16) -> String {
    // SAFETY: theo hợp đồng của hàm, đọc tới trước NUL là trong vùng hợp lệ.
    let n = (0..).take_while(|&i| unsafe { *p.add(i) } != 0).count();
    // SAFETY: n phần tử đầu vừa được đọc ở trên, hợp lệ.
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, n) })
}

/// Handle Win32 tự đóng khi ra khỏi phạm vi.
struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn new(h: HANDLE) -> Option<Self> {
        (!h.is_null() && h != INVALID_HANDLE_VALUE).then_some(Self(h))
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: self.0 là handle hợp lệ do OwnedHandle độc quyền sở hữu, chỉ đóng một lần ở đây.
        unsafe { CloseHandle(self.0) };
    }
}

/// Vùng nhớ do Windows cấp bằng LocalAlloc (GetSecurityInfo, SetEntriesInAclW, Convert*), tự LocalFree.
struct LocalMem(*mut c_void);

impl Drop for LocalMem {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: con trỏ do API cấp bằng LocalAlloc, chưa giải phóng, chỉ giải phóng một lần ở đây.
            unsafe { LocalFree(self.0) };
        }
    }
}

pub fn disk_free(path: &Path) -> Result<u64> {
    let w = wide(path.as_os_str());
    let mut avail: u64 = 0;
    // SAFETY: w kết thúc bằng NUL; avail là biến cục bộ khả ghi; hai con trỏ còn lại được phép null.
    let ok = unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, std::ptr::null_mut(), std::ptr::null_mut()) };
    if ok == 0 {
        return Err(io_err(path, io::Error::last_os_error()));
    }
    Ok(avail)
}

pub fn split_lines(pending: &mut String, incoming: &str) -> Vec<String> {
    pending.push_str(incoming);
    let mut out = Vec::new();
    while let Some(i) = pending.find(['\r', '\n']) {
        let line: String = pending.drain(..=i).collect();
        let line = line.trim();
        if !line.is_empty() {
            out.push(line.to_string());
        }
    }
    out
}

pub(crate) fn valid_service_name(name: &str) -> Result<()> {
    if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Ok(())
    } else {
        Err(CoreError::System(format!("invalid service name: {name:?}")))
    }
}

/// `/ResetBase` biến mọi bản cập nhật thành vĩnh viễn (không gỡ được) — không bao giờ chạy, dù ai gọi.
pub(crate) fn refuse_resetbase(args: &[&str]) -> Result<()> {
    if args.iter().any(|a| a.to_ascii_lowercase().contains("resetbase")) {
        Err(CoreError::System("refusing DISM /ResetBase: it makes installed updates permanent".into()))
    } else {
        Ok(())
    }
}

/// Mã thoát DISM âm là HRESULT — in kèm dạng hex (vd `-2146498554 (0x800F0806)`) để tra được.
fn dism_exit_text(code: Option<i32>) -> String {
    match code {
        Some(c) if c < 0 => format!("{c} (0x{:08X})", c as u32),
        Some(c) => c.to_string(),
        None => "unknown".to_string(),
    }
}

/// Đối số DISM + `/ScratchDir:` riêng (mặc định DISM bung DismHost và DLL vào %TEMP%).
fn dism_args(args: &[&str], scratch: &Path) -> Vec<OsString> {
    let mut out: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut s = OsString::from("/ScratchDir:");
    s.push(scratch.as_os_str());
    out.push(s);
    out
}

/// Script PowerShell: xuất UTF-8, và đặt lại PSModulePath ngay trong phiên (PowerShell tự chèn thêm
/// thư mục module của người dùng vào biến này lúc khởi động).
fn ps_script(script: &str, system32: &Path) -> String {
    let modules = ps_modules_dir(system32).to_string_lossy().replace('\'', "''");
    format!("{PS_UTF8_PREFIX}$env:PSModulePath='{modules}';{script}")
}

fn ps_modules_dir(system32: &Path) -> PathBuf {
    system32.join(r"WindowsPowerShell\v1.0\Modules")
}

/// Môi trường tối thiểu cho tiến trình con, dựng hoàn toàn từ đường dẫn lấy qua API.
/// Không mang TEMP/PSModulePath/PATH… của người dùng (DLL hijack qua %TEMP%, module giả qua PSModulePath).
fn child_env(windir: &Path, system32: &Path) -> Vec<(&'static str, OsString)> {
    let temp = windir.join("Temp").into_os_string();
    let mut path = system32.as_os_str().to_os_string();
    for extra in [windir.to_path_buf(), system32.join("Wbem"), system32.join(r"WindowsPowerShell\v1.0")] {
        path.push(";");
        path.push(extra.as_os_str());
    }
    let arch = if cfg!(target_arch = "aarch64") {
        "ARM64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "AMD64"
    };
    let mut env = vec![
        ("SystemRoot", windir.as_os_str().to_os_string()),
        ("windir", windir.as_os_str().to_os_string()),
        ("TEMP", temp.clone()),
        ("TMP", temp),
        ("ComSpec", system32.join("cmd.exe").into_os_string()),
        ("PATHEXT", OsString::from(".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC")),
        ("PATH", path),
        ("PSModulePath", ps_modules_dir(system32).into_os_string()),
        ("PROCESSOR_ARCHITECTURE", OsString::from(arch)),
    ];
    if let Some(Component::Prefix(p)) = windir.components().next() {
        env.push(("SystemDrive", p.as_os_str().to_os_string()));
    }
    env
}

fn child_env_from_api() -> Result<Vec<(&'static str, OsString)>> {
    Ok(child_env(&windows_dir()?, &system_dir()?))
}

/// Đọc một thư mục hệ thống qua API kiểu `GetSystemDirectoryW` (không qua biến môi trường).
fn api_dir(f: unsafe extern "system" fn(PWSTR, u32) -> u32, what: &str) -> Result<PathBuf> {
    let mut buf = vec![0u16; 260];
    loop {
        // SAFETY: buf có đúng buf.len() phần tử u16 khả ghi; API ghi không quá độ dài được báo.
        let n = unsafe { f(buf.as_mut_ptr(), buf.len() as u32) } as usize;
        if n == 0 {
            return Err(CoreError::System(format!("{what} failed: {}", io::Error::last_os_error())));
        }
        if n < buf.len() {
            buf.truncate(n);
            return Ok(PathBuf::from(OsString::from_wide(&buf)));
        }
        // Bộ đệm thiếu: n là kích thước cần, gồm cả NUL.
        buf.resize(n, 0);
    }
}

/// Thư mục Windows dùng chung (không phải bản riêng từng phiên Terminal Server).
pub(crate) fn windows_dir() -> Result<PathBuf> {
    api_dir(GetSystemWindowsDirectoryW, "GetSystemWindowsDirectoryW")
}

pub(crate) fn system_dir() -> Result<PathBuf> {
    api_dir(GetSystemDirectoryW, "GetSystemDirectoryW")
}

/// `C:\Windows` → `C:\`.
fn drive_root(windir: &Path) -> Result<PathBuf> {
    match windir.components().next() {
        Some(Component::Prefix(p)) => {
            let mut s = p.as_os_str().to_os_string();
            s.push("\\");
            Ok(PathBuf::from(s))
        }
        _ => Err(CoreError::System(format!("windows directory has no drive: {}", windir.display()))),
    }
}

/// Đường dẫn tuyệt đối dạng `\\?\` để CreateFileW không vướng giới hạn MAX_PATH (Windows.old rất sâu).
/// Dạng lạ (tương đối, có `.`/`..`, đã là `\\?\`) giữ nguyên.
fn verbatim(path: &Path) -> PathBuf {
    let mut comps = path.components();
    let mut out = match comps.next() {
        Some(Component::Prefix(p)) => match p.kind() {
            Prefix::Disk(_) => {
                let mut s = OsString::from(r"\\?\");
                s.push(p.as_os_str());
                s
            }
            Prefix::UNC(server, share) => {
                let mut s = OsString::from(r"\\?\UNC\");
                s.push(server);
                s.push("\\");
                s.push(share);
                s
            }
            _ => return path.to_path_buf(),
        },
        _ => return path.to_path_buf(),
    };
    if comps.next() != Some(Component::RootDir) {
        return path.to_path_buf();
    }
    let mut any = false;
    for c in comps {
        match c {
            Component::Normal(n) => {
                out.push("\\");
                out.push(n);
                any = true;
            }
            _ => return path.to_path_buf(),
        }
    }
    if !any {
        out.push("\\");
    }
    PathBuf::from(out)
}

/// Gộp lỗi từng mục: đếm hết, giữ vài thông điệp đầu (đã cắt ngắn).
#[derive(Default)]
struct ErrorSummary {
    count: usize,
    shown: Vec<String>,
}

impl ErrorSummary {
    fn push(&mut self, msg: String) {
        self.count += 1;
        if self.shown.len() < MAX_SHOWN_ERRORS {
            let mut m: String = msg.chars().take(MAX_ERROR_CHARS).collect();
            if m.len() < msg.len() {
                m.push('…');
            }
            self.shown.push(m);
        }
    }

    fn into_result(self, what: &str, note: Option<String>) -> Result<()> {
        if self.count == 0 {
            return Ok(());
        }
        let mut msg = format!("{what}: {} error(s): {}", self.count, self.shown.join(" | "));
        if self.count > self.shown.len() {
            msg.push_str(&format!(" (+{} more)", self.count - self.shown.len()));
        }
        if let Some(n) = note {
            msg.push_str(&format!(" [{n}]"));
        }
        Err(CoreError::System(msg))
    }
}

/// Mở handle chỉ để đọc/ghi thông tin bảo mật. FILE_FLAG_OPEN_REPARSE_POINT chỉ che thành phần CUỐI của
/// đường dẫn; thành phần giữa vẫn có thể đi qua junction — vì vậy mọi handle dùng để sửa ACL đều phải qua
/// `open_checked` (kiểm trên chính handle). CreateFileW luôn xin kèm
/// FILE_READ_ATTRIBUTES + SYNCHRONIZE: với mục mà DACL không cấp gì cho Administrators, hai quyền này chỉ có
/// được nhờ SeBackupPrivilege (bật trong lúc take_ownership).
fn open_for_security(path: &Path, access: u32) -> io::Result<OwnedHandle> {
    let w = wide(verbatim(path).as_os_str());
    // SAFETY: w kết thúc bằng NUL; security attributes và template được phép null.
    let h = unsafe {
        CreateFileW(
            w.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    OwnedHandle::new(h).ok_or_else(io::Error::last_os_error)
}

fn attribute_tag(h: &OwnedHandle) -> io::Result<u32> {
    let mut info = FILE_ATTRIBUTE_TAG_INFO::default();
    // SAFETY: info là struct đúng kiểu cho lớp FileAttributeTagInfo, kích thước truyền khớp.
    check(unsafe {
        GetFileInformationByHandleEx(
            h.0,
            FileAttributeTagInfo,
            (&mut info as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
            std::mem::size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    })?;
    Ok(info.FileAttributes)
}

fn link_count(h: &OwnedHandle) -> io::Result<u32> {
    // SAFETY: BY_HANDLE_FILE_INFORMATION là struct C thuần, toàn số 0 hợp lệ.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: h hợp lệ; info khả ghi.
    check(unsafe { GetFileInformationByHandle(h.0, &mut info) })?;
    Ok(info.nNumberOfLinks)
}

/// Đường dẫn thật của đối tượng mà handle trỏ tới (dạng `\\?\C:\…`), sau khi đã phân giải mọi junction.
fn final_path(h: &OwnedHandle) -> io::Result<Vec<u16>> {
    let mut buf = vec![0u16; 512];
    loop {
        // SAFETY: buf có đúng buf.len() phần tử khả ghi.
        let n = unsafe {
            GetFinalPathNameByHandleW(h.0, buf.as_mut_ptr(), buf.len() as u32, FILE_NAME_NORMALIZED | VOLUME_NAME_DOS)
        } as usize;
        if n == 0 {
            return Err(io::Error::last_os_error());
        }
        if n < buf.len() {
            buf.truncate(n);
            return Ok(buf);
        }
        buf.resize(n, 0);
    }
}

/// `child` là chính `root` hoặc nằm bên dưới nó (so theo ranh giới `\`, để `C:\A` không khớp `C:\AB`).
fn is_within(child: &[u16], root: &[u16]) -> bool {
    const SEP: u16 = b'\\' as u16;
    if child == root {
        return true;
    }
    child.len() > root.len()
        && child.starts_with(root)
        && (root.last() == Some(&SEP) || child[root.len()] == SEP)
}

/// Handle đã kiểm: không phải reparse point, không phải tệp nhiều hard link, và đường dẫn thật nằm trong gốc.
struct Checked {
    handle: OwnedHandle,
    is_dir: bool,
    path: PathBuf,
}

enum Rejected {
    Io(io::Error),
    Link,
    HardLink,
    Outside(PathBuf),
}

impl Rejected {
    fn describe(&self, p: &Path) -> String {
        match self {
            Rejected::Io(e) => format!("{}: {e}", p.display()),
            Rejected::Link => format!("{}: is a reparse point, skipped", p.display()),
            Rejected::HardLink => format!("{}: has several hard links, skipped", p.display()),
            Rejected::Outside(real) => {
                format!("{}: resolves outside the tree ({}), skipped", p.display(), real.display())
            }
        }
    }
}

impl From<io::Error> for Rejected {
    fn from(e: io::Error) -> Self {
        Rejected::Io(e)
    }
}

/// Mở `path` rồi kiểm trên CHÍNH handle đó. Kẻ tấn công tráo một thư mục giữa đường dẫn thành junction
/// trỏ về C:\Windows thì đường dẫn thật (final path) nằm ngoài gốc ⇒ bị từ chối trước khi đụng ACL.
/// Hard link: đổi chủ một tên là đổi chủ cả tệp ở mọi tên khác (có thể nằm ngoài cây) ⇒ bỏ qua.
fn open_checked(path: &Path, access: u32, root: &[u16]) -> std::result::Result<Checked, Rejected> {
    let handle = open_for_security(path, access | FILE_READ_ATTRIBUTES)?;
    let attrs = attribute_tag(&handle)?;
    if attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(Rejected::Link);
    }
    let is_dir = attrs & FILE_ATTRIBUTE_DIRECTORY != 0;
    if !is_dir && link_count(&handle)? > 1 {
        return Err(Rejected::HardLink);
    }
    let real = final_path(&handle)?;
    let real_path = PathBuf::from(OsString::from_wide(&real));
    if !is_within(&real, root) {
        return Err(Rejected::Outside(real_path));
    }
    Ok(Checked { handle, is_dir, path: real_path })
}

/// Chủ sở hữu của đối tượng, dạng chuỗi SID (`S-1-5-18`…).
fn owner_sid_string(h: &OwnedHandle) -> io::Result<String> {
    let mut owner: PSID = std::ptr::null_mut();
    let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: h mở với READ_CONTROL; các con trỏ ra là biến cục bộ; sd được LocalMem giải phóng.
    let err = unsafe {
        GetSecurityInfo(
            h.0,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut sd,
        )
    };
    if err != 0 {
        return Err(io::Error::from_raw_os_error(err as i32));
    }
    let _sd = LocalMem(sd);
    let mut text: PWSTR = std::ptr::null_mut();
    // SAFETY: owner trỏ vào sd còn sống; text do LocalMem giải phóng.
    check(unsafe { ConvertSidToStringSidW(owner, &mut text) })?;
    let _text = LocalMem(text.cast());
    // SAFETY: text là chuỗi kết thúc NUL do API cấp, còn sống tới hết hàm.
    Ok(unsafe { wide_ptr_to_string(text) })
}

/// Chỉ nhận cây do hệ thống sở hữu. Người dùng thường tự dựng `C:\Windows.old` (được phép tạo thư mục ở
/// gốc ổ) thì cây đó không được đổi chủ.
fn trusted_owner(sid: &str) -> bool {
    matches!(sid, "S-1-5-18" | "S-1-5-32-544" | TRUSTED_INSTALLER_SID)
}

/// Duyệt cây từ `root`, cha trước con, gọi `visit(đường dẫn thật, gốc thật, lỗi)` cho từng mục đã qua
/// `open_checked`. Reparse point và hard link bị bỏ qua (safety sẽ gỡ liên kết). Gốc là reparse point, hoặc
/// (khi `require_trusted_owner`) gốc không do SYSTEM/TrustedInstaller/Administrators sở hữu ⇒ Err, không duyệt gì.
fn walk_checked(
    root: &Path,
    require_trusted_owner: bool,
    errors: &mut ErrorSummary,
    visit: &mut dyn FnMut(&Path, &[u16], &mut ErrorSummary),
) -> io::Result<()> {
    let root_h = open_for_security(root, FILE_READ_ATTRIBUTES | READ_CONTROL)?;
    if attribute_tag(&root_h)? & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::other("refusing: the root is a reparse point"));
    }
    if require_trusted_owner {
        let owner = owner_sid_string(&root_h)?;
        if !trusted_owner(&owner) {
            return Err(io::Error::other(format!(
                "refusing: the root is owned by {owner}, not SYSTEM, TrustedInstaller or Administrators"
            )));
        }
    }
    let root_real = final_path(&root_h)?;
    drop(root_h);
    let mut stack = vec![PathBuf::from(OsString::from_wide(&root_real))];
    while let Some(p) = stack.pop() {
        let (is_dir, real) = match open_checked(&p, 0, &root_real) {
            Ok(c) => (c.is_dir, c.path),
            Err(Rejected::Link | Rejected::HardLink) => continue,
            Err(r) => {
                errors.push(r.describe(&p));
                continue;
            }
        };
        visit(&real, &root_real, errors);
        if !is_dir {
            continue;
        }
        // Liệt kê theo đường dẫn là an toàn: mỗi mục con lại được kiểm trên handle của chính nó.
        match std::fs::read_dir(&real) {
            Ok(rd) => {
                for entry in rd {
                    match entry {
                        Ok(e) => stack.push(e.path()),
                        Err(e) => errors.push(format!("{}: {e}", real.display())),
                    }
                }
            }
            Err(e) => errors.push(format!("{}: {e}", real.display())),
        }
    }
    Ok(())
}

/// SID nhóm Administrators (S-1-5-32-544), bộ đệm SECURITY_MAX_SID_SIZE = 68 byte, căn 4 byte.
struct AdminsSid([u32; 17]);

impl AdminsSid {
    fn new() -> io::Result<Self> {
        let mut buf = [0u32; 17];
        let mut cb = std::mem::size_of_val(&buf) as u32;
        // SAFETY: buf khả ghi đúng cb byte; domain SID được phép null với SID dựng sẵn.
        check(unsafe {
            CreateWellKnownSid(WinBuiltinAdministratorsSid, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut cb)
        })?;
        Ok(Self(buf))
    }

    fn psid(&self) -> PSID {
        // Các API nhận PSID không ghi vào SID; ép sang *mut chỉ vì chữ ký C.
        self.0.as_ptr() as PSID
    }
}

/// Bật một đặc quyền cho token tiến trình; trả lại trạng thái cũ khi drop.
struct PrivilegeGuard {
    token: OwnedHandle,
    previous: TOKEN_PRIVILEGES,
}

impl PrivilegeGuard {
    fn enable(name: PCWSTR) -> io::Result<Self> {
        let mut raw: HANDLE = std::ptr::null_mut();
        // SAFETY: GetCurrentProcess trả pseudo-handle luôn hợp lệ; raw là biến cục bộ khả ghi.
        check(unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut raw) })?;
        let token = OwnedHandle(raw);
        let mut luid = LUID::default();
        // SAFETY: name là hằng chuỗi rộng kết thúc NUL của windows-sys; luid khả ghi.
        check(unsafe { LookupPrivilegeValueW(std::ptr::null(), name, &mut luid) })?;
        let wanted = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }],
        };
        let mut previous = TOKEN_PRIVILEGES::default();
        let mut len = 0u32;
        // SAFETY: token mở với TOKEN_ADJUST_PRIVILEGES|TOKEN_QUERY; previous đủ chỗ cho một đặc quyền.
        check(unsafe {
            AdjustTokenPrivileges(
                token.0,
                0,
                &wanted,
                std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
                &mut previous,
                &mut len,
            )
        })?;
        // AdjustTokenPrivileges trả TRUE cả khi token không có đặc quyền; phải xem mã lỗi cuối.
        // SAFETY: chỉ đọc mã lỗi của luồng hiện tại.
        let last = unsafe { GetLastError() };
        if last == ERROR_NOT_ALL_ASSIGNED {
            return Err(io::Error::from_raw_os_error(last as i32));
        }
        Ok(Self { token, previous })
    }
}

impl Drop for PrivilegeGuard {
    fn drop(&mut self) {
        // SAFETY: token còn mở (self sở hữu); previous do chính AdjustTokenPrivileges điền.
        unsafe {
            AdjustTokenPrivileges(self.token.0, 0, &self.previous, 0, std::ptr::null_mut(), std::ptr::null_mut())
        };
    }
}

fn enable_privilege(name: PCWSTR, label: &str, notes: &mut Vec<String>) -> Option<PrivilegeGuard> {
    PrivilegeGuard::enable(name).map_err(|e| notes.push(format!("{label} not enabled: {e}"))).ok()
}

/// Descriptor tuyệt đối rỗng trên stack; SetKernelObjectSecurity ghi thẳng, KHÔNG lan quyền xuống con
/// (khác SetNamedSecurityInfoW tự duyệt cây con — có thể đi theo junction).
fn empty_descriptor() -> io::Result<SECURITY_DESCRIPTOR> {
    // SAFETY: SECURITY_DESCRIPTOR là struct C thuần, toàn số 0 là giá trị hợp lệ trước khi khởi tạo.
    let mut sd: SECURITY_DESCRIPTOR = unsafe { std::mem::zeroed() };
    // SAFETY: sd là vùng nhớ khả ghi đủ lớn cho descriptor tuyệt đối.
    check(unsafe { InitializeSecurityDescriptor((&mut sd as *mut SECURITY_DESCRIPTOR).cast(), SD_REVISION) })?;
    Ok(sd)
}

fn set_owner_admins(h: &OwnedHandle, admins: &AdminsSid) -> io::Result<()> {
    let mut sd = empty_descriptor()?;
    let psd: PSECURITY_DESCRIPTOR = (&mut sd as *mut SECURITY_DESCRIPTOR).cast();
    // SAFETY: psd trỏ tới sd đã khởi tạo; SID sống tới hết hàm; h mở với WRITE_OWNER.
    unsafe {
        check(SetSecurityDescriptorOwner(psd, admins.psid(), 0))?;
        check(SetKernelObjectSecurity(h.0, OWNER_SECURITY_INFORMATION, psd))
    }
}

/// Thêm ACE "Administrators: Full Control" (không kế thừa — như `icacls /grant *S-1-5-32-544:F`),
/// giữ các ACE cũ và cờ protected/auto-inherited của DACL.
fn grant_admins_full_control(h: &OwnedHandle, admins: &AdminsSid) -> io::Result<()> {
    let mut old_dacl: *mut ACL = std::ptr::null_mut();
    let mut old_sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: h mở với READ_CONTROL; các con trỏ ra là biến cục bộ; old_sd được LocalMem giải phóng.
    let err = unsafe {
        GetSecurityInfo(
            h.0,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut old_dacl,
            std::ptr::null_mut(),
            &mut old_sd,
        )
    };
    if err != 0 {
        return Err(io::Error::from_raw_os_error(err as i32));
    }
    let _old_sd = LocalMem(old_sd);
    if old_dacl.is_null() {
        // DACL NULL = mọi người đã toàn quyền; dựng DACL mới sẽ chỉ làm hẹp quyền.
        return Ok(());
    }
    let mut old_control = 0u16;
    let mut revision = 0u32;
    // SAFETY: old_sd còn sống (LocalMem); hai con trỏ ra là biến cục bộ.
    check(unsafe { GetSecurityDescriptorControl(old_sd, &mut old_control, &mut revision) })?;

    let access = EXPLICIT_ACCESS_W {
        grfAccessPermissions: FILE_ALL_ACCESS,
        grfAccessMode: GRANT_ACCESS,
        grfInheritance: NO_INHERITANCE,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_GROUP,
            ptstrName: admins.psid().cast(),
        },
    };
    let mut new_dacl: *mut ACL = std::ptr::null_mut();
    // SAFETY: access hợp lệ, SID sống tới hết hàm; old_dacl nằm trong old_sd còn sống; new_dacl do LocalMem giải phóng.
    let err = unsafe { SetEntriesInAclW(1, &access, old_dacl, &mut new_dacl) };
    if err != 0 {
        return Err(io::Error::from_raw_os_error(err as i32));
    }
    let _new_dacl = LocalMem(new_dacl.cast());

    let mut sd = empty_descriptor()?;
    let psd: PSECURITY_DESCRIPTOR = (&mut sd as *mut SECURITY_DESCRIPTOR).cast();
    let keep = SE_DACL_PROTECTED | SE_DACL_AUTO_INHERITED;
    // SAFETY: psd trỏ tới sd đã khởi tạo; new_dacl sống tới hết hàm; h mở với WRITE_DAC.
    unsafe {
        check(SetSecurityDescriptorDacl(psd, 1, new_dacl, 0))?;
        check(SetSecurityDescriptorControl(psd, keep, old_control & keep))?;
        check(SetKernelObjectSecurity(h.0, DACL_SECURITY_INFORMATION, psd))
    }
}

/// Hai bước cho một mục, mỗi bước trên handle riêng đã qua `open_checked`. Luôn chạy cả hai.
fn own_and_grant_item(p: &Path, root: &[u16], admins: &AdminsSid, errors: &mut ErrorSummary) {
    match open_checked(p, WRITE_OWNER, root) {
        Ok(c) => {
            if let Err(e) = set_owner_admins(&c.handle, admins) {
                errors.push(format!("owner {}: {e}", c.path.display()));
            }
        }
        Err(r) => errors.push(format!("owner {}", r.describe(p))),
    }
    match open_checked(p, READ_CONTROL | WRITE_DAC, root) {
        Ok(c) => {
            if let Err(e) = grant_admins_full_control(&c.handle, admins) {
                errors.push(format!("grant {}: {e}", c.path.display()));
            }
        }
        Err(r) => errors.push(format!("grant {}", r.describe(p))),
    }
}

/// Job Object giết cả cây tiến trình (Dism.exe + DismHost.exe) khi TerminateJobObject hoặc khi handle đóng.
fn kill_on_close_job() -> io::Result<OwnedHandle> {
    // SAFETY: tham số null = job vô danh, security mặc định.
    let job = OwnedHandle::new(unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) })
        .ok_or_else(io::Error::last_os_error)?;
    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    // SAFETY: info là struct đúng kiểu cho lớp JobObjectExtendedLimitInformation, kích thước truyền khớp.
    check(unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    })?;
    Ok(job)
}

/// Chạy tiếp mọi luồng của tiến trình `pid` (vừa tạo với CREATE_SUSPENDED nên chỉ có luồng chính).
fn resume_threads(pid: u32) -> io::Result<()> {
    // SAFETY: chụp danh sách luồng; handle do OwnedHandle đóng.
    let snap = OwnedHandle::new(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) })
        .ok_or_else(io::Error::last_os_error)?;
    // SAFETY: THREADENTRY32 là struct C thuần, toàn số 0 hợp lệ; dwSize đặt ngay sau.
    let mut e: THREADENTRY32 = unsafe { std::mem::zeroed() };
    e.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
    let mut resumed = 0;
    // SAFETY: snap là snapshot hợp lệ; e có dwSize đúng.
    let mut more = unsafe { Thread32First(snap.0, &mut e) } != 0;
    while more {
        if e.th32OwnerProcessID == pid {
            // SAFETY: mở luồng theo id vừa liệt kê; handle do OwnedHandle đóng.
            let t = OwnedHandle::new(unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, e.th32ThreadID) })
                .ok_or_else(io::Error::last_os_error)?;
            // SAFETY: t mở với THREAD_SUSPEND_RESUME.
            if unsafe { ResumeThread(t.0) } == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            resumed += 1;
        }
        // SAFETY: như trên.
        more = unsafe { Thread32Next(snap.0, &mut e) } != 0;
    }
    if resumed == 0 {
        return Err(io::Error::other(format!("no thread found for process {pid}")));
    }
    Ok(())
}

/// Tạo tiến trình ở trạng thái treo, cho vào job KILL_ON_JOB_CLOSE rồi mới chạy — tiến trình cháu
/// (DismHost) không thể sinh ra ngoài job. Không cho vào job được thì giết tiến trình và báo lỗi.
fn spawn_in_job(cmd: &mut Command) -> io::Result<(Child, OwnedHandle)> {
    let job = kill_on_close_job().map_err(|e| io::Error::new(e.kind(), format!("job object: {e}")))?;
    let mut child = cmd.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED).spawn()?;
    // SAFETY: job và handle tiến trình con đều còn sống trong phạm vi này.
    let assigned = check(unsafe { AssignProcessToJobObject(job.0, child.as_raw_handle() as HANDLE) });
    if let Err(e) = assigned.and_then(|()| resume_threads(child.id())) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(io::Error::new(e.kind(), format!("job object: {e}")));
    }
    Ok((child, job))
}

/// Thư mục tạm riêng (DACL chỉ SYSTEM + Administrators), tên mới mỗi lần; tự xóa khi drop.
/// CreateDirectoryW thất bại nếu tên đã có ⇒ không bao giờ dùng lại thư mục (hay junction) ai đó dựng sẵn.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn create(parent: &Path) -> io::Result<Self> {
        let sddl = wide(OsStr::new(PRIVATE_DIR_SDDL));
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: sddl kết thúc NUL; sd do LocalMem giải phóng; con trỏ kích thước được phép null.
        check(unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut sd,
                std::ptr::null_mut(),
            )
        })?;
        let _sd = LocalMem(sd);
        let sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd,
            bInheritHandle: 0,
        };
        for attempt in 0..16u32 {
            let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
            let dir = parent.join(format!("WinFreeUp-{}-{nanos}-{attempt}", std::process::id()));
            let w = wide(verbatim(&dir).as_os_str());
            // SAFETY: w kết thúc NUL; sa trỏ tới descriptor còn sống tới hết hàm.
            if unsafe { CreateDirectoryW(w.as_ptr(), &sa) } != 0 {
                return Ok(Self(dir));
            }
            let e = io::Error::last_os_error();
            if e.raw_os_error() != Some(ERROR_ALREADY_EXISTS as i32) {
                return Err(e);
            }
        }
        Err(io::Error::other("could not create a unique scratch directory"))
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        // remove_dir_all không đi theo junction; nếu không liệt kê được (vd tiến trình không phải Admin)
        // thì thử xóa thư mục rỗng.
        if std::fs::remove_dir_all(&self.0).is_err() {
            let _ = std::fs::remove_dir(&self.0);
        }
    }
}

fn run_capture(exe: &Path, args: &[&OsStr], env: &[(&'static str, OsString)]) -> Result<String> {
    let out = Command::new(exe)
        .args(args)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| CoreError::System(format!("{}: {e}", exe.display())))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        Err(CoreError::System(
            format!("{} exit {}: {} {}", exe.display(), out.status.code().unwrap_or(-1), stdout, stderr)
                .trim()
                .to_string(),
        ))
    }
}

pub struct RealSystem {
    pub system32: PathBuf,
}

impl RealSystem {
    fn powershell(&self, script: &str) -> Result<String> {
        let exe = self.system32.join(r"WindowsPowerShell\v1.0\powershell.exe");
        let env = child_env_from_api()?;
        let script = ps_script(script, &system_dir()?);
        let args: Vec<&OsStr> = ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script]
            .into_iter()
            .map(OsStr::new)
            .collect();
        run_capture(&exe, &args, &env)
    }
}

impl SystemOps for RealSystem {
    fn is_process_running(&self, exe_name: &str) -> bool {
        // SAFETY: gọi API chụp danh sách tiến trình; handle do OwnedHandle đóng.
        let Some(snap) = OwnedHandle::new(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
            return false;
        };
        // SAFETY: PROCESSENTRY32W là struct C thuần, toàn số 0 hợp lệ; dwSize đặt ngay sau.
        let mut e: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        // SAFETY: snap là snapshot hợp lệ; e có dwSize đúng.
        let mut more = unsafe { Process32FirstW(snap.0, &mut e) } != 0;
        while more {
            let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            if String::from_utf16_lossy(&e.szExeFile[..len]).eq_ignore_ascii_case(exe_name) {
                return true;
            }
            // SAFETY: như trên.
            more = unsafe { Process32NextW(snap.0, &mut e) } != 0;
        }
        false
    }

    fn stop_service(&self, name: &str) -> Result<bool> {
        valid_service_name(name)?;
        let out = self.powershell(&format!(
            "$s = Get-Service -Name '{name}' -ErrorAction Stop; if ($s.Status -eq 'Running') {{ Stop-Service -Name '{name}' -Force -ErrorAction Stop; 'STOPPED' }} else {{ 'ALREADY' }}"
        ))?;
        Ok(out.ends_with("STOPPED"))
    }

    fn start_service(&self, name: &str) -> Result<()> {
        valid_service_name(name)?;
        self.powershell(&format!("Start-Service -Name '{name}' -ErrorAction Stop")).map(|_| ())
    }

    fn run_dism(&self, args: &[&str], cancel: &CancelToken, on_line: &mut dyn FnMut(&str)) -> Result<()> {
        refuse_resetbase(args)?;
        let exe = self.system32.join("Dism.exe");
        let env = child_env_from_api()?;
        let scratch = ScratchDir::create(&windows_dir()?.join("Temp"))
            .map_err(|e| CoreError::System(format!("DISM scratch directory: {e}")))?;
        let mut cmd = Command::new(&exe);
        cmd.args(dism_args(args, &scratch.0))
            .env_clear()
            .envs(env.iter().map(|(k, v)| (k, v)))
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let (mut child, job) =
            spawn_in_job(&mut cmd).map_err(|e| CoreError::System(format!("{}: {e}", exe.display())))?;
        let mut stdout = child.stdout.take().expect("stdout is piped");
        let (tx, rx) = mpsc::channel::<String>();
        let reader = std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut pending = String::new();
            loop {
                match stdout.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        for line in split_lines(&mut pending, &String::from_utf8_lossy(&buf[..n])) {
                            if tx.send(line).is_err() {
                                return;
                            }
                        }
                    }
                }
            }
            if !pending.trim().is_empty() {
                let _ = tx.send(pending.trim().to_string());
            }
        });
        let mut tail: Vec<String> = Vec::new();
        loop {
            if cancel.is_cancelled() {
                // SAFETY: job là handle job hợp lệ còn sống; giết cả Dism.exe lẫn DismHost.exe.
                unsafe { TerminateJobObject(job.0, 1) };
                let _ = child.wait();
                // Không join luồng đọc: nếu còn tiến trình nào giữ đầu ghi pipe thì join sẽ treo.
                // Luồng tự kết thúc khi pipe đóng; rx bị drop nên nó không gửi thêm được gì.
                drop(reader);
                return Err(CoreError::Cancelled);
            }
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(line) => {
                    on_line(&line);
                    tail.push(line);
                    if tail.len() > 5 {
                        tail.remove(0);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        // Kênh đã ngắt nghĩa là luồng đọc đã thoát: join không chờ.
        let _ = reader.join();
        let status = child.wait().map_err(|e| CoreError::System(format!("{}: {e}", exe.display())))?;
        match status.code() {
            Some(0) | Some(DISM_OK_REBOOT) => Ok(()),
            code => Err(CoreError::System(format!("DISM exit code {}: {}", dism_exit_text(code), tail.join(" | ")))),
        }
    }

    fn recycle_bin_size(&self) -> Result<(u64, u64)> {
        // SAFETY: SHQUERYRBINFO là struct C thuần, toàn số 0 hợp lệ; cbSize đặt ngay sau.
        let mut info: SHQUERYRBINFO = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHQUERYRBINFO>() as u32;
        // SAFETY: null = mọi ổ; info khả ghi với cbSize đúng.
        let hr = unsafe { SHQueryRecycleBinW(std::ptr::null(), &mut info) };
        if hr < 0 {
            return Err(CoreError::System(format!("SHQueryRecycleBinW failed: 0x{:08X}", hr as u32)));
        }
        let size = info.i64Size;
        let items = info.i64NumItems;
        Ok((size.max(0) as u64, items.max(0) as u64))
    }

    fn empty_recycle_bin(&self) -> Result<()> {
        let flags = SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND;
        // SAFETY: hwnd null và root null (= mọi ổ) là hợp lệ.
        let hr = unsafe { SHEmptyRecycleBinW(std::ptr::null_mut(), std::ptr::null(), flags) };
        if hr >= 0 || hr == E_UNEXPECTED {
            Ok(())
        } else {
            Err(CoreError::System(format!("SHEmptyRecycleBinW failed: 0x{:08X}", hr as u32)))
        }
    }

    fn create_restore_point(&self, description: &str) -> Result<()> {
        let d = description.replace('\'', "''");
        self.powershell(&format!(
            "Checkpoint-Computer -Description '{d}' -RestorePointType 'MODIFY_SETTINGS' -WarningAction SilentlyContinue -WarningVariable w -ErrorAction Stop; if ($w) {{ Write-Output ($w -join ' '); exit 2 }}"
        ))
        .map(|_| ())
    }

    /// Đổi chủ sở hữu sang Administrators rồi cấp Administrators Full Control cho từng mục trong cây.
    /// Từ chối cả cây nếu gốc là liên kết hoặc không do SYSTEM/TrustedInstaller/Administrators sở hữu.
    /// Mỗi mục được kiểm trên chính handle sẽ sửa (không liên kết, không hard link, đường dẫn thật trong gốc).
    /// Làm đủ cả hai bước cho mọi mục; lỗi gộp lại sau khi chạy hết.
    fn take_ownership(&self, path: &Path) -> Result<()> {
        let what = format!("take_ownership {}", path.display());
        let admins = AdminsSid::new().map_err(|e| CoreError::System(format!("{what}: Administrators SID: {e}")))?;
        let mut notes = Vec::new();
        let take = enable_privilege(SE_TAKE_OWNERSHIP_NAME, "SeTakeOwnershipPrivilege", &mut notes);
        let restore = enable_privilege(SE_RESTORE_NAME, "SeRestorePrivilege", &mut notes);
        let backup = enable_privilege(SE_BACKUP_NAME, "SeBackupPrivilege", &mut notes);
        let mut errors = ErrorSummary::default();
        let walked = walk_checked(path, true, &mut errors, &mut |p, root, errors| {
            own_and_grant_item(p, root, &admins, errors)
        });
        drop(backup);
        drop(restore);
        drop(take);
        walked.map_err(|e| CoreError::System(format!("{what}: {e}")))?;
        errors.into_result(&what, (!notes.is_empty()).then(|| notes.join("; ")))
    }
}

impl Env {
    pub fn from_system() -> Result<Env> {
        fn var(name: &str) -> Result<PathBuf> {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .ok_or_else(|| CoreError::System(format!("environment variable {name} is not set")))
        }
        let windir = windows_dir()?;
        Ok(Env {
            temp: std::env::temp_dir(),
            system_drive: drive_root(&windir)?,
            windir,
            local_appdata: var("LOCALAPPDATA")?,
            program_data: var("ProgramData")?,
            now: SystemTime::now(),
            sys: Arc::new(RealSystem { system32: system_dir()? }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, GetNamedSecurityInfoW, SetNamedSecurityInfoW,
    };
    use windows_sys::Win32::Security::GetSecurityDescriptorDacl;

    #[test]
    fn split_lines_handles_carriage_returns_and_partial_chunks() {
        let mut pending = String::new();
        assert_eq!(split_lines(&mut pending, "[==  10.0%  ]\r[===  2"), vec!["[==  10.0%  ]"]);
        assert_eq!(split_lines(&mut pending, "0.0%  ]\r\nDone.\r\n"), vec!["[===  20.0%  ]", "Done."]);
        assert!(pending.is_empty());
    }

    #[test]
    fn disk_free_of_temp_dir_is_positive() {
        assert!(disk_free(&std::env::temp_dir()).unwrap() > 0);
    }

    #[test]
    fn disk_free_of_missing_drive_is_an_error() {
        assert!(disk_free(Path::new(r"\\?\Volume{00000000-0000-0000-0000-000000000000}\")).is_err());
    }

    #[test]
    fn env_from_system_points_at_existing_folders() {
        let env = Env::from_system().unwrap();
        for p in [&env.temp, &env.windir, &env.local_appdata, &env.program_data, &env.system_drive] {
            assert!(p.exists(), "{}", p.display());
        }
        assert!(env.system_drive.to_string_lossy().ends_with('\\'));
        assert!(env.windir.starts_with(&env.system_drive));
    }

    const FAKE_ENV_CHILD: &str = "WFU_FAKE_ENV_CHILD";

    /// Chạy lại chính test này trong tiến trình con có SystemRoot/windir/SystemDrive giả — không `set_var`
    /// trong tiến trình test (các test khác chạy song song đọc chung môi trường).
    #[test]
    fn system_dirs_come_from_the_api_not_from_environment_variables() {
        if let Some(fake) = std::env::var_os(FAKE_ENV_CHILD) {
            let fake = PathBuf::from(fake);
            assert_eq!(std::env::var_os("windir"), Some(fake.clone().into_os_string()), "env not faked");
            let windir = windows_dir().unwrap();
            let system32 = system_dir().unwrap();
            let env = Env::from_system().unwrap();
            assert_ne!(windir, fake);
            assert_eq!(env.windir, windir);
            assert!(!env.system_drive.starts_with(&fake));
            assert!(system32.starts_with(&windir), "{}", system32.display());
            assert!(system32.join("Dism.exe").is_file());
            let child = child_env(&windir, &system32);
            assert!(child.iter().all(|(_, v)| !Path::new(v).starts_with(&fake)));
            return;
        }
        let fake = tempfile::tempdir().unwrap();
        let out = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "sys_windows::tests::system_dirs_come_from_the_api_not_from_environment_variables",
                "--test-threads=1",
            ])
            .env(FAKE_ENV_CHILD, fake.path())
            .env("SystemRoot", fake.path())
            .env("windir", fake.path())
            .env("SystemDrive", "Z:")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
        assert!(stdout.contains("1 passed"), "{stdout}");
    }

    #[test]
    fn child_processes_get_a_minimal_environment_built_from_system_paths() {
        let env = child_env(Path::new(r"C:\Windows"), Path::new(r"C:\Windows\System32"));
        let get = |k: &str| env.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string_lossy().into_owned());
        assert_eq!(get("SystemRoot").as_deref(), Some(r"C:\Windows"));
        assert_eq!(get("windir").as_deref(), Some(r"C:\Windows"));
        assert_eq!(get("SystemDrive").as_deref(), Some("C:"));
        assert_eq!(get("TEMP").as_deref(), Some(r"C:\Windows\Temp"));
        assert_eq!(get("TMP").as_deref(), Some(r"C:\Windows\Temp"));
        assert_eq!(get("ComSpec").as_deref(), Some(r"C:\Windows\System32\cmd.exe"));
        assert_eq!(get("PSModulePath").as_deref(), Some(r"C:\Windows\System32\WindowsPowerShell\v1.0\Modules"));
        let path = get("PATH").unwrap();
        assert!(path.starts_with(r"C:\Windows\System32;C:\Windows;"), "{path}");
        assert!(path.split(';').all(|p| p.starts_with(r"C:\Windows")), "{path}");
        for k in ["USERPROFILE", "LOCALAPPDATA", "APPDATA", "HOMEPATH"] {
            assert!(get(k).is_none(), "{k}");
        }
    }

    /// Chạy PowerShell thật với môi trường tối thiểu, chỉ đọc (không đổi gì trên máy).
    #[test]
    fn powershell_runs_with_the_minimal_environment() {
        let system32 = system_dir().unwrap();
        let windir = windows_dir().unwrap();
        let sys = RealSystem { system32: system32.clone() };
        let out = sys
            .powershell("$env:TEMP + '|' + $env:PSModulePath + '|' + (Get-Service -Name 'wuauserv').Name")
            .unwrap();
        let expected = format!("{}|{}|wuauserv", windir.join("Temp").display(), ps_modules_dir(&system32).display());
        assert!(out.eq_ignore_ascii_case(&expected), "{out}");
    }

    #[test]
    fn drive_root_and_verbatim_paths() {
        assert_eq!(drive_root(Path::new(r"C:\Windows")).unwrap(), PathBuf::from(r"C:\"));
        assert!(drive_root(Path::new(r"Windows")).is_err());
        assert_eq!(verbatim(Path::new(r"C:\Windows.old\Users")), PathBuf::from(r"\\?\C:\Windows.old\Users"));
        assert_eq!(verbatim(Path::new(r"C:\")), PathBuf::from(r"\\?\C:\"));
        assert_eq!(verbatim(Path::new(r"\\srv\share\a")), PathBuf::from(r"\\?\UNC\srv\share\a"));
        assert_eq!(verbatim(Path::new(r"\\?\C:\x")), PathBuf::from(r"\\?\C:\x"));
        assert_eq!(verbatim(Path::new(r"C:\a\..\b")), PathBuf::from(r"C:\a\..\b"));
        assert_eq!(verbatim(Path::new(r"rel\x")), PathBuf::from(r"rel\x"));
    }

    #[test]
    fn detects_the_current_test_process_but_not_a_made_up_one() {
        let sys = RealSystem { system32: PathBuf::from(r"C:\Windows\System32") };
        let me = std::env::current_exe().unwrap();
        let name = me.file_name().unwrap().to_string_lossy().to_string();
        assert!(sys.is_process_running(&name));
        assert!(!sys.is_process_running("khong-co-tien-trinh-nay-3f9a.exe"));
    }

    #[test]
    fn service_names_are_validated_before_reaching_powershell() {
        assert!(valid_service_name("wuauserv").is_ok());
        assert!(valid_service_name("x'; Remove-Item C:\\ -Recurse; '").is_err());
        assert!(valid_service_name("").is_err());
    }

    #[test]
    fn powershell_scripts_force_utf8_and_system_modules() {
        let s = ps_script("Get-Service", Path::new(r"C:\Windows\System32"));
        assert!(s.starts_with("[Console]::OutputEncoding=[Text.Encoding]::UTF8;"), "{s}");
        assert!(s.contains(r"$env:PSModulePath='C:\Windows\System32\WindowsPowerShell\v1.0\Modules';"), "{s}");
        assert!(s.ends_with(";Get-Service"), "{s}");
        assert!(ps_script("x", Path::new(r"C:\it's")).contains(r"C:\it''s\"));
    }

    #[test]
    fn run_dism_refuses_resetbase_in_any_case_before_spawning() {
        // system32 giả: dù kiểm tra có lọt thì cũng không thể chạy DISM thật.
        let fake = tempfile::tempdir().unwrap();
        let sys = RealSystem { system32: fake.path().to_path_buf() };
        let cancel = CancelToken::new();
        for arg in ["/ResetBase", "/resetbase", "/RESETBASE"] {
            let err = sys
                .run_dism(&["/Online", "/Cleanup-Image", "/StartComponentCleanup", arg], &cancel, &mut |_| {})
                .unwrap_err();
            assert!(matches!(&err, CoreError::System(m) if m.contains("ResetBase")), "{err}");
        }
        assert!(refuse_resetbase(&["/Online", "/Cleanup-Image", "/StartComponentCleanup"]).is_ok());
    }

    #[test]
    fn dism_gets_a_private_scratch_dir_argument() {
        let args = dism_args(&["/Online", "/Cleanup-Image"], Path::new(r"C:\Windows\Temp\WinFreeUp-1"));
        assert_eq!(args, vec![
            OsString::from("/Online"),
            OsString::from("/Cleanup-Image"),
            OsString::from(r"/ScratchDir:C:\Windows\Temp\WinFreeUp-1"),
        ]);
    }

    #[test]
    fn scratch_dir_is_fresh_private_and_removed_on_drop() {
        let parent = tempfile::tempdir().unwrap();
        let a = ScratchDir::create(parent.path()).unwrap();
        let b = ScratchDir::create(parent.path()).unwrap();
        assert_ne!(a.0, b.0);
        assert!(a.0.starts_with(parent.path()) && a.0.is_dir());
        let s = sddl(&a.0);
        assert!(s.contains("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)"), "{s}");
        assert!(std::fs::write(a.0.join("x"), b"x").is_err() || s.starts_with("O:BA"), "{s}");
        // Không nâng quyền thì chính người chạy test cũng không xóa được (đúng ý đồ): mở DACL rồi mới dọn.
        unlock_for_test(&a.0);
        unlock_for_test(&b.0);
        let path = a.0.clone();
        drop(a);
        assert!(!path.exists());
    }

    /// Chủ sở hữu luôn có WRITE_DAC ngầm định: thay DACL bằng Everyone Full Control để dọn được.
    /// Chỉ dùng cho thư mục RỖNG trong thư mục tạm (SetNamedSecurityInfoW lan quyền xuống con).
    fn unlock_for_test(path: &Path) {
        let name = wide(path.as_os_str());
        let text = wide(OsStr::new("D:(A;OICI;FA;;;WD)"));
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        let mut dacl: *mut ACL = std::ptr::null_mut();
        let (mut present, mut defaulted) = (0, 0);
        // SAFETY: chuỗi kết thúc NUL; sd do LocalMem giải phóng; dacl trỏ vào sd còn sống.
        unsafe {
            check(ConvertStringSecurityDescriptorToSecurityDescriptorW(
                text.as_ptr(),
                SDDL_REVISION_1,
                &mut sd,
                std::ptr::null_mut(),
            ))
            .unwrap();
            let _sd = LocalMem(sd);
            check(GetSecurityDescriptorDacl(sd, &mut present, &mut dacl, &mut defaulted)).unwrap();
            let err = SetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                dacl,
                std::ptr::null(),
            );
            assert_eq!(err, 0, "{}", path.display());
        }
    }

    #[test]
    fn dism_negative_exit_codes_are_shown_in_hex() {
        assert_eq!(dism_exit_text(Some(0x800F0806u32 as i32)), "-2146498554 (0x800F0806)");
        assert_eq!(dism_exit_text(Some(87)), "87");
        assert_eq!(dism_exit_text(None), "unknown");
    }

    fn cmd(args: &[&str]) -> Command {
        let mut c = Command::new(system_dir().unwrap().join("cmd.exe"));
        c.args(args).stdin(Stdio::null()).stderr(Stdio::null());
        c
    }

    #[test]
    fn spawn_in_job_resumes_the_suspended_process() {
        let (mut child, _job) = spawn_in_job(cmd(&["/c", "exit 7"]).stdout(Stdio::null())).unwrap();
        assert_eq!(child.wait().unwrap().code(), Some(7));
    }

    /// Mô phỏng DismHost: tiến trình cháu (ping) giữ đầu ghi pipe. Kết thúc job phải đóng được pipe.
    #[test]
    fn terminating_the_job_kills_grandchildren_holding_the_pipe() {
        let (mut child, job) = spawn_in_job(cmd(&["/c", "ping -n 30 127.0.0.1"]).stdout(Stdio::piped())).unwrap();
        let mut out = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = out.read_to_end(&mut sink);
            let _ = tx.send(());
        });
        std::thread::sleep(Duration::from_millis(300));
        // SAFETY: job là handle job hợp lệ còn sống.
        unsafe { TerminateJobObject(job.0, 1) };
        child.wait().unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(10)).is_ok(), "pipe still open after job termination");
    }

    #[test]
    fn error_summary_counts_everything_but_keeps_few_short_messages() {
        let mut s = ErrorSummary::default();
        for i in 0..8 {
            s.push(format!("{i}:{}", "x".repeat(500)));
        }
        let Err(CoreError::System(m)) = s.into_result("op", Some("note".into())) else { panic!("expected Err") };
        assert!(m.starts_with("op: 8 error(s): 0:"));
        assert!(m.contains("(+3 more)") && m.ends_with("[note]"));
        assert!(m.len() < 5 * (MAX_ERROR_CHARS + 10) + 100);
        assert!(ErrorSummary::default().into_result("op", Some("n".into())).is_ok());
    }

    fn w(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn is_within_respects_component_boundaries() {
        let root = w(r"\\?\C:\Windows.old");
        assert!(is_within(&w(r"\\?\C:\Windows.old"), &root));
        assert!(is_within(&w(r"\\?\C:\Windows.old\Users\a"), &root));
        assert!(!is_within(&w(r"\\?\C:\Windows.old2\x"), &root));
        assert!(!is_within(&w(r"\\?\C:\Windows\System32"), &root));
        assert!(is_within(&w(r"\\?\D:\x"), &w(r"\\?\D:\")));
    }

    #[test]
    fn only_system_owners_are_trusted() {
        assert!(trusted_owner("S-1-5-18"));
        assert!(trusted_owner("S-1-5-32-544"));
        assert!(trusted_owner(TRUSTED_INSTALLER_SID));
        assert!(!trusted_owner("S-1-5-21-1111111111-2222222222-3333333333-1001"));
        assert!(!trusted_owner("S-1-1-0"));
    }

    /// Cây tạm: root/{a/{f.txt, b/}, link -> outside/, hl.txt = hard link tới outside/linked.txt};
    /// outside/{secret.txt, linked.txt}.
    fn tree_with_links() -> (tempfile::TempDir, tempfile::TempDir) {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join(r"a\b")).unwrap();
        std::fs::write(root.path().join(r"a\f.txt"), b"x").unwrap();
        std::fs::write(outside.path().join("secret.txt"), b"s").unwrap();
        std::fs::write(outside.path().join("linked.txt"), b"l").unwrap();
        junction::create(outside.path(), root.path().join("link")).unwrap();
        std::fs::hard_link(outside.path().join("linked.txt"), root.path().join("hl.txt")).unwrap();
        (root, outside)
    }

    fn real(p: &Path) -> Vec<u16> {
        final_path(&open_for_security(p, FILE_READ_ATTRIBUTES).unwrap()).unwrap()
    }

    fn real_path(p: &Path) -> PathBuf {
        PathBuf::from(OsString::from_wide(&real(p)))
    }

    #[test]
    fn walk_visits_parents_first_and_skips_junctions_and_hard_links() {
        let (root, outside) = tree_with_links();
        let mut seen = Vec::new();
        let mut errors = ErrorSummary::default();
        walk_checked(root.path(), false, &mut errors, &mut |p, _, _| seen.push(p.to_path_buf())).unwrap();
        assert_eq!(errors.count, 0, "{:?}", errors.shown);
        let r = real_path(root.path());
        let mut sorted = seen.clone();
        sorted.sort();
        assert_eq!(sorted, vec![r.clone(), r.join("a"), r.join(r"a\b"), r.join(r"a\f.txt")]);
        let pos = |p: &Path| seen.iter().position(|s| s == p).unwrap();
        assert!(pos(&r) < pos(&r.join("a")) && pos(&r.join("a")) < pos(&r.join(r"a\f.txt")));
        let out = real_path(outside.path());
        assert!(seen.iter().all(|p| !p.starts_with(&out)));
    }

    #[test]
    fn walk_refuses_a_root_that_is_itself_a_junction() {
        let (root, _outside) = tree_with_links();
        let mut seen = Vec::new();
        let mut errors = ErrorSummary::default();
        let err = walk_checked(&root.path().join("link"), false, &mut errors, &mut |p, _, _| {
            seen.push(p.to_path_buf())
        })
        .unwrap_err();
        assert!(err.to_string().contains("reparse point"), "{err}");
        assert!(seen.is_empty());
    }

    /// Mô phỏng tĩnh vụ tráo junction giữa lúc duyệt: đường dẫn đi qua `link` (junction ra ngoài cây)
    /// ⇒ final path nằm ngoài gốc ⇒ bị từ chối ở cả hai bước, đích không đổi một bit ACL nào.
    #[test]
    fn items_resolving_outside_the_root_are_never_modified() {
        let (root, outside) = tree_with_links();
        let secret = outside.path().join("secret.txt");
        let before = (sddl(outside.path()), sddl(&secret));
        let admins = AdminsSid::new().unwrap();
        let mut errors = ErrorSummary::default();
        let root_real = real(root.path());
        own_and_grant_item(&root.path().join(r"link\secret.txt"), &root_real, &admins, &mut errors);
        own_and_grant_item(&root.path().join("link"), &root_real, &admins, &mut errors);
        own_and_grant_item(&root.path().join("hl.txt"), &root_real, &admins, &mut errors);
        assert_eq!(errors.count, 6, "{:?}", errors.shown);
        assert!(errors.shown.iter().take(2).all(|m| m.contains("outside the tree")), "{:?}", errors.shown);
        assert_eq!((sddl(outside.path()), sddl(&secret)), before);
        assert!(!sddl(&outside.path().join("linked.txt")).contains("(A;;FA;;;BA)"));
    }

    #[test]
    fn take_ownership_refuses_a_tree_not_owned_by_the_system() {
        let (root, _outside) = tree_with_links();
        let h = open_for_security(root.path(), READ_CONTROL).unwrap();
        let owner = owner_sid_string(&h).unwrap();
        drop(h);
        if trusted_owner(&owner) {
            // Đang chạy nâng quyền: thư mục tạm do Administrators sở hữu — trường hợp từ chối không dựng được.
            return;
        }
        let before = sddl(root.path());
        let sys = RealSystem { system32: PathBuf::from(r"C:\nonexistent") };
        let err = sys.take_ownership(root.path()).unwrap_err().to_string();
        assert!(err.contains("refusing") && err.contains(&owner), "{err}");
        assert_eq!(sddl(root.path()), before);
    }

    /// Đọc bảo mật theo tên (GetNamedSecurityInfoW mở không kèm SYNCHRONIZE/FILE_READ_ATTRIBUTES nên đọc được
    /// cả thư mục riêng mà người chạy test chỉ có quyền chủ sở hữu).
    fn sddl(path: &Path) -> String {
        let name = wide(path.as_os_str());
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        let info = OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
        // SAFETY: name kết thúc NUL; sd được LocalMem giải phóng.
        let err = unsafe {
            GetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                info,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut sd,
            )
        };
        assert_eq!(err, 0);
        let _sd = LocalMem(sd);
        let mut text: PWSTR = std::ptr::null_mut();
        let mut len = 0u32;
        // SAFETY: sd hợp lệ; text do LocalMem giải phóng; len nhận số ký tự.
        check(unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(sd, SDDL_REVISION_1, info, &mut text, &mut len)
        })
        .unwrap();
        let _text = LocalMem(text.cast());
        // SAFETY: text là chuỗi kết thúc NUL do API cấp, còn sống tới hết hàm.
        unsafe { wide_ptr_to_string(text) }
    }

    /// Chạy ACL thật nhưng CHỈ trên cây tạm (bỏ kiểm chủ gốc). Không Admin thì bước đổi chủ lỗi (được gộp),
    /// bước cấp quyền vẫn phải chạy cho mọi mục; junction và hard link ra ngoài cây không bị đụng.
    #[test]
    fn owning_the_tree_grants_every_item_and_leaves_link_targets_alone() {
        let (root, outside) = tree_with_links();
        let before_outside =
            [outside.path().to_path_buf(), outside.path().join("secret.txt"), outside.path().join("linked.txt")]
                .map(|p| sddl(&p));
        let file = root.path().join(r"a\f.txt");
        assert!(!sddl(&file).contains("(A;;FA;;;BA)"));

        let admins = AdminsSid::new().unwrap();
        let mut errors = ErrorSummary::default();
        walk_checked(root.path(), false, &mut errors, &mut |p, r, errors| own_and_grant_item(p, r, &admins, errors))
            .unwrap();

        let elevated = errors.count == 0;
        for p in [root.path().to_path_buf(), root.path().join("a"), root.path().join(r"a\b"), file] {
            let s = sddl(&p);
            assert!(s.contains("(A;;FA;;;BA)"), "{}: {s}", p.display());
            if elevated {
                assert!(s.starts_with("O:BA"), "{s}");
            }
        }
        if !elevated {
            // Chỉ chấp nhận lỗi đổi chủ (thiếu quyền Admin), đếm đủ 4 mục — không dừng ở mục đầu.
            assert_eq!(errors.count, 4, "{:?}", errors.shown);
            assert!(errors.shown.iter().all(|m| m.starts_with("owner ")), "{:?}", errors.shown);
        }
        let after_outside =
            [outside.path().to_path_buf(), outside.path().join("secret.txt"), outside.path().join("linked.txt")]
                .map(|p| sddl(&p));
        assert_eq!(after_outside, before_outside);
    }
}
