//! Cài đặt thật của SystemOps. Mọi tiến trình con chạy với CREATE_NO_WINDOW (không nháy cửa sổ đen).
use std::ffi::OsStr;
use std::io::Read;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc};
use std::time::{Duration, SystemTime};

use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
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
const ADMINS_SID: &str = "*S-1-5-32-544";

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

pub fn disk_free(path: &Path) -> Result<u64> {
    let w = wide(path.as_os_str());
    let mut avail: u64 = 0;
    let ok = unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, std::ptr::null_mut(), std::ptr::null_mut()) };
    if ok == 0 {
        return Err(io_err(path, std::io::Error::last_os_error()));
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
        let args: Vec<&OsStr> = ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script]
            .into_iter()
            .map(OsStr::new)
            .collect();
        run_capture(&exe, &args)
    }
}

impl SystemOps for RealSystem {
    fn is_process_running(&self, exe_name: &str) -> bool {
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return false;
            }
            let mut e: PROCESSENTRY32W = std::mem::zeroed();
            e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut found = false;
            if Process32FirstW(snap, &mut e) != 0 {
                loop {
                    let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                    if String::from_utf16_lossy(&e.szExeFile[..len]).eq_ignore_ascii_case(exe_name) {
                        found = true;
                        break;
                    }
                    if Process32NextW(snap, &mut e) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snap);
            found
        }
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
        let exe = self.system32.join("Dism.exe");
        let mut child = Command::new(&exe)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| CoreError::System(format!("{}: {e}", exe.display())))?;
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
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
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
        let _ = reader.join();
        let status = child.wait().map_err(|e| CoreError::System(format!("{}: {e}", exe.display())))?;
        match status.code() {
            Some(0) | Some(DISM_OK_REBOOT) => Ok(()),
            code => Err(CoreError::System(format!("DISM exit code {}: {}", code.unwrap_or(-1), tail.join(" | ")))),
        }
    }

    fn recycle_bin_size(&self) -> Result<(u64, u64)> {
        let mut info: SHQUERYRBINFO = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHQUERYRBINFO>() as u32;
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

    fn take_ownership(&self, path: &Path) -> Result<()> {
        let icacls = self.system32.join("icacls.exe");
        let p = path.as_os_str();
        let own: Vec<&OsStr> = vec![p, OsStr::new("/setowner"), OsStr::new(ADMINS_SID), OsStr::new("/T"), OsStr::new("/C"), OsStr::new("/L"), OsStr::new("/Q")];
        run_capture(&icacls, &own)?;
        let grant = format!("{ADMINS_SID}:F");
        let g: Vec<&OsStr> = vec![p, OsStr::new("/grant"), OsStr::new(&grant), OsStr::new("/T"), OsStr::new("/C"), OsStr::new("/L"), OsStr::new("/Q")];
        run_capture(&icacls, &g).map(|_| ())
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
        let windir = var("SystemRoot").or_else(|_| var("windir"))?;
        let mut drive = var("SystemDrive")?.into_os_string();
        drive.push("\\");
        Ok(Env {
            temp: std::env::temp_dir(),
            windir: windir.clone(),
            local_appdata: var("LOCALAPPDATA")?,
            program_data: var("ProgramData")?,
            system_drive: PathBuf::from(drive),
            now: SystemTime::now(),
            sys: Arc::new(RealSystem { system32: windir.join("System32") }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
