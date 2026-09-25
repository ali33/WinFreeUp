//! Cài đặt thật của SystemOps. Mọi tiến trình con chạy với CREATE_NO_WINDOW (không nháy cửa sổ đen).
//!
//! Ứng dụng luôn chạy quyền Admin nhưng thừa hưởng biến môi trường của người dùng thường, nên đường dẫn
//! hệ thống (Windows, System32, ổ hệ thống) lấy qua API chứ không qua `SystemRoot`/`windir`.
use std::ffi::{c_void, OsStr, OsString};
use std::io::{self, Read};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::{Component, Path, PathBuf, Prefix};
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc};
use std::time::{Duration, SystemTime};

use windows_sys::core::{BOOL, PCWSTR, PWSTR};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, ERROR_NOT_ALL_ASSIGNED, HANDLE, INVALID_HANDLE_VALUE, LUID,
};
use windows_sys::Win32::Security::Authorization::{
    GetSecurityInfo, SetEntriesInAclW, EXPLICIT_ACCESS_W, GRANT_ACCESS, NO_MULTIPLE_TRUSTEE, SE_FILE_OBJECT,
    TRUSTEE_IS_GROUP, TRUSTEE_IS_SID, TRUSTEE_W,
};
use windows_sys::Win32::Security::{
    AdjustTokenPrivileges, CreateWellKnownSid, GetSecurityDescriptorControl, InitializeSecurityDescriptor,
    LookupPrivilegeValueW, SetKernelObjectSecurity, SetSecurityDescriptorControl, SetSecurityDescriptorDacl,
    SetSecurityDescriptorOwner, WinBuiltinAdministratorsSid, ACL, DACL_SECURITY_INFORMATION, LUID_AND_ATTRIBUTES,
    NO_INHERITANCE, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, SECURITY_DESCRIPTOR,
    SE_DACL_AUTO_INHERITED, SE_DACL_PROTECTED, SE_PRIVILEGE_ENABLED, SE_TAKE_OWNERSHIP_NAME, TOKEN_ADJUST_PRIVILEGES,
    TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, GetDiskFreeSpaceExW, FILE_ALL_ACCESS, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, READ_CONTROL, WRITE_DAC, WRITE_OWNER,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
    TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::SystemInformation::{GetSystemDirectoryW, GetSystemWindowsDirectoryW};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::UI::Shell::{
    SHEmptyRecycleBinW, SHQueryRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHQUERYRBINFO,
};

use crate::env::{Env, SystemOps};
use crate::error::{io_err, CoreError, Result};
use crate::safety::is_reparse_point;
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

/// Vùng nhớ do Windows cấp bằng LocalAlloc (GetSecurityInfo, SetEntriesInAclW), tự LocalFree.
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

fn ps_script(script: &str) -> String {
    format!("{PS_UTF8_PREFIX}{script}")
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

/// Duyệt cây từ `root`, gọi `visit` cho từng mục TRƯỚC khi liệt kê con của nó (để mục đã được cấp quyền
/// rồi mới đọc được). Không bao giờ đi vào hay chạm tới reparse point (junction, symlink): Windows.old thật
/// có junction trỏ về C:\Users, C:\ProgramData. Bản thân liên kết cũng bỏ qua — safety sẽ gỡ nó.
fn walk_no_follow(root: &Path, errors: &mut ErrorSummary, visit: &mut dyn FnMut(&Path, &mut ErrorSummary)) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(p) = stack.pop() {
        let meta = match std::fs::symlink_metadata(&p) {
            Ok(m) => m,
            Err(e) => {
                errors.push(format!("{}: {e}", p.display()));
                continue;
            }
        };
        if is_reparse_point(&meta) {
            continue;
        }
        visit(&p, errors);
        if !meta.is_dir() {
            continue;
        }
        match std::fs::read_dir(&p) {
            Ok(rd) => {
                for entry in rd {
                    match entry {
                        Ok(e) => stack.push(e.path()),
                        Err(e) => errors.push(format!("{}: {e}", p.display())),
                    }
                }
            }
            Err(e) => errors.push(format!("{}: {e}", p.display())),
        }
    }
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

/// Mở handle chỉ để đọc/ghi thông tin bảo mật. FILE_FLAG_OPEN_REPARSE_POINT: nếu mục vừa bị đổi thành
/// liên kết thì handle trỏ vào bản thân liên kết, không bao giờ vào đích.
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

/// Descriptor tuyệt đối rỗng trên stack; SetKernelObjectSecurity ghi thẳng, KHÔNG lan quyền xuống con
/// (khác SetNamedSecurityInfoW tự duyệt cây con — có thể đi theo junction).
fn empty_descriptor() -> io::Result<SECURITY_DESCRIPTOR> {
    // SAFETY: SECURITY_DESCRIPTOR là struct C thuần, toàn số 0 là giá trị hợp lệ trước khi khởi tạo.
    let mut sd: SECURITY_DESCRIPTOR = unsafe { std::mem::zeroed() };
    // SAFETY: sd là vùng nhớ khả ghi đủ lớn cho descriptor tuyệt đối.
    check(unsafe { InitializeSecurityDescriptor((&mut sd as *mut SECURITY_DESCRIPTOR).cast(), SD_REVISION) })?;
    Ok(sd)
}

fn set_owner_admins(path: &Path, admins: &AdminsSid) -> io::Result<()> {
    let h = open_for_security(path, WRITE_OWNER)?;
    let mut sd = empty_descriptor()?;
    let psd: PSECURITY_DESCRIPTOR = (&mut sd as *mut SECURITY_DESCRIPTOR).cast();
    // SAFETY: psd trỏ tới sd đã khởi tạo; SID sống tới hết hàm; h là handle mở với WRITE_OWNER.
    unsafe {
        check(SetSecurityDescriptorOwner(psd, admins.psid(), 0))?;
        check(SetKernelObjectSecurity(h.0, OWNER_SECURITY_INFORMATION, psd))
    }
}

/// Thêm ACE "Administrators: Full Control" (không kế thừa — như `icacls /grant *S-1-5-32-544:F`),
/// giữ các ACE cũ và cờ protected/auto-inherited của DACL.
fn grant_admins_full_control(path: &Path, admins: &AdminsSid) -> io::Result<()> {
    let h = open_for_security(path, READ_CONTROL | WRITE_DAC)?;
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

fn run_capture(exe: &Path, args: &[&OsStr]) -> Result<String> {
    let out = Command::new(exe)
        .args(args)
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
        let script = ps_script(script);
        let args: Vec<&OsStr> = ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script]
            .into_iter()
            .map(OsStr::new)
            .collect();
        run_capture(&exe, &args)
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
        let mut child = Command::new(&exe)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| CoreError::System(format!("{}: {e}", exe.display())))?;
        // Không dựng được job thì vẫn chạy: khi hủy chỉ giết Dism.exe, và vẫn không chờ luồng đọc.
        let job = kill_on_close_job().ok().filter(|job| {
            // SAFETY: job và handle tiến trình con đều còn sống trong phạm vi này.
            unsafe { AssignProcessToJobObject(job.0, child.as_raw_handle() as HANDLE) != 0 }
        });
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
                if let Some(job) = &job {
                    // SAFETY: job là handle job hợp lệ còn sống.
                    unsafe { TerminateJobObject(job.0, 1) };
                }
                let _ = child.kill();
                let _ = child.wait();
                // Không join luồng đọc: DismHost (tiến trình cháu) có thể còn giữ đầu ghi pipe khiến join treo.
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

    /// Đổi chủ sở hữu sang Administrators rồi cấp Administrators Full Control cho từng mục trong cây,
    /// không đi theo reparse point. Làm đủ cả hai bước cho mọi mục; lỗi gộp lại sau khi chạy hết.
    fn take_ownership(&self, path: &Path) -> Result<()> {
        let what = format!("take_ownership {}", path.display());
        let admins = AdminsSid::new().map_err(|e| CoreError::System(format!("{what}: Administrators SID: {e}")))?;
        let privilege = PrivilegeGuard::enable(SE_TAKE_OWNERSHIP_NAME);
        let note = privilege.as_ref().err().map(|e| format!("SeTakeOwnershipPrivilege not enabled: {e}"));
        let mut errors = ErrorSummary::default();
        walk_no_follow(path, &mut errors, &mut |p, errors| {
            if let Err(e) = set_owner_admins(p, &admins) {
                errors.push(format!("owner {}: {e}", p.display()));
            }
            if let Err(e) = grant_admins_full_control(p, &admins) {
                errors.push(format!("grant {}: {e}", p.display()));
            }
        });
        drop(privilege);
        errors.into_result(&what, note)
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
        ConvertSecurityDescriptorToStringSecurityDescriptorW, SDDL_REVISION_1,
    };

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

    #[test]
    fn system_dirs_come_from_the_api_not_from_environment_variables() {
        struct Restore(Vec<(&'static str, Option<OsString>)>);
        impl Drop for Restore {
            fn drop(&mut self) {
                for (k, v) in &self.0 {
                    match v {
                        Some(v) => std::env::set_var(k, v),
                        None => std::env::remove_var(k),
                    }
                }
            }
        }
        let fake = tempfile::tempdir().unwrap();
        let names = ["SystemRoot", "windir", "SystemDrive"];
        let restore = Restore(names.iter().map(|k| (*k, std::env::var_os(k))).collect());
        for k in names {
            std::env::set_var(k, fake.path());
        }
        let windir = windows_dir().unwrap();
        let system32 = system_dir().unwrap();
        let env = Env::from_system().unwrap();
        drop(restore);

        assert_ne!(windir, fake.path());
        assert_eq!(env.windir, windir);
        assert!(!env.system_drive.starts_with(fake.path()));
        assert!(system32.starts_with(&windir), "{}", system32.display());
        assert!(system32.join("Dism.exe").is_file());
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
    fn powershell_scripts_force_utf8_output() {
        assert!(ps_script("Get-Service").starts_with("[Console]::OutputEncoding=[Text.Encoding]::UTF8;"));
        assert!(ps_script("Get-Service").ends_with("Get-Service"));
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
    fn dism_negative_exit_codes_are_shown_in_hex() {
        assert_eq!(dism_exit_text(Some(0x800F0806u32 as i32)), "-2146498554 (0x800F0806)");
        assert_eq!(dism_exit_text(Some(87)), "87");
        assert_eq!(dism_exit_text(None), "unknown");
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

    /// Cây tạm: root/{a/{f.txt, b/}, link -> outside/}; outside/secret.txt.
    fn tree_with_junction() -> (tempfile::TempDir, tempfile::TempDir) {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join(r"a\b")).unwrap();
        std::fs::write(root.path().join(r"a\f.txt"), b"x").unwrap();
        std::fs::write(outside.path().join("secret.txt"), b"s").unwrap();
        junction::create(outside.path(), root.path().join("link")).unwrap();
        (root, outside)
    }

    #[test]
    fn walk_visits_parents_first_and_never_enters_or_touches_junctions() {
        let (root, outside) = tree_with_junction();
        let mut seen = Vec::new();
        let mut errors = ErrorSummary::default();
        walk_no_follow(root.path(), &mut errors, &mut |p, _| seen.push(p.to_path_buf()));
        assert_eq!(errors.count, 0, "{:?}", errors.shown);
        let r = root.path();
        let mut sorted = seen.clone();
        sorted.sort();
        assert_eq!(sorted, vec![r.to_path_buf(), r.join("a"), r.join(r"a\b"), r.join(r"a\f.txt")]);
        let pos = |p: &Path| seen.iter().position(|s| s == p).unwrap();
        assert!(pos(r) < pos(&r.join("a")) && pos(&r.join("a")) < pos(&r.join(r"a\f.txt")));
        assert!(seen.iter().all(|p| !p.starts_with(outside.path()) && !p.starts_with(r.join("link"))));
    }

    #[test]
    fn walk_skips_a_root_that_is_itself_a_junction() {
        let (root, _outside) = tree_with_junction();
        let mut seen = Vec::new();
        let mut errors = ErrorSummary::default();
        walk_no_follow(&root.path().join("link"), &mut errors, &mut |p, _| seen.push(p.to_path_buf()));
        assert!(seen.is_empty());
        assert_eq!(errors.count, 0);
    }

    fn sddl(path: &Path) -> String {
        let h = open_for_security(path, READ_CONTROL).unwrap();
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        let info = OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
        // SAFETY: h mở với READ_CONTROL; sd được LocalMem giải phóng.
        let err = unsafe {
            GetSecurityInfo(
                h.0,
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
        // SAFETY: text trỏ tới chuỗi rộng kết thúc NUL do API cấp; đọc tới trước NUL.
        let n = (0..).take_while(|&i| unsafe { *text.add(i) } != 0).count();
        // SAFETY: n phần tử đầu của text hợp lệ (vừa đọc ở trên).
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, n) })
    }

    /// Chạy ACL thật nhưng CHỈ trên cây tạm. Không Admin thì bước đổi chủ lỗi (được gộp), bước cấp quyền
    /// vẫn phải chạy cho mọi mục; đích của junction không bao giờ bị đụng.
    #[test]
    fn take_ownership_grants_every_item_and_leaves_junction_targets_alone() {
        let (root, outside) = tree_with_junction();
        let before_outside = (sddl(outside.path()), sddl(&outside.path().join("secret.txt")));
        let file = root.path().join(r"a\f.txt");
        assert!(!sddl(&file).contains("(A;;FA;;;BA)"));

        let sys = RealSystem { system32: PathBuf::from(r"C:\nonexistent") };
        let result = sys.take_ownership(root.path());

        for p in [root.path().to_path_buf(), root.path().join("a"), root.path().join(r"a\b"), file] {
            let s = sddl(&p);
            assert!(s.contains("(A;;FA;;;BA)"), "{}: {s}", p.display());
            if result.is_ok() {
                assert!(s.starts_with("O:BA"), "{s}");
            }
        }
        if let Err(e) = &result {
            // Chỉ chấp nhận lỗi đổi chủ (thiếu quyền Admin), đếm đủ 4 mục — không dừng ở mục đầu.
            let m = e.to_string();
            assert!(m.contains("4 error(s)") && m.contains("owner ") && !m.contains("grant "), "{m}");
        }
        assert_eq!((sddl(outside.path()), sddl(&outside.path().join("secret.txt"))), before_outside);
    }
}
