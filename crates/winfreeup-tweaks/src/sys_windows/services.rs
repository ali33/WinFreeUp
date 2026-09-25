//! Kiểu khởi động dịch vụ qua Service Control Manager. Hoàn tác KHÔNG tự khởi động dịch vụ (spec 3.1).
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{ERROR_SERVICE_DOES_NOT_EXIST, ERROR_SERVICE_NOT_ACTIVE};
use windows::Win32::System::Services::{
    ChangeServiceConfigW, CloseServiceHandle, ControlService, OpenSCManagerW, OpenServiceW, QueryServiceConfigW, ENUM_SERVICE_TYPE,
    QUERY_SERVICE_CONFIGW, SC_HANDLE, SC_MANAGER_CONNECT, SERVICE_AUTO_START, SERVICE_CHANGE_CONFIG, SERVICE_CONTROL_STOP,
    SERVICE_DEMAND_START, SERVICE_DISABLED, SERVICE_ERROR, SERVICE_NO_CHANGE, SERVICE_QUERY_CONFIG, SERVICE_STATUS, SERVICE_STOP,
};

use crate::model::StartType;

struct Handle(SC_HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseServiceHandle(self.0);
        }
    }
}

/// `Ok(None)` = dịch vụ không tồn tại.
fn open(name: &str, access: u32) -> Result<Option<(Handle, Handle)>, String> {
    let scm = Handle(unsafe { OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT) }.map_err(|e| format!("OpenSCManager: {}", e.message()))?);
    match unsafe { OpenServiceW(scm.0, &HSTRING::from(name), access) } {
        Ok(h) => Ok(Some((scm, Handle(h)))),
        Err(e) if e.code() == ERROR_SERVICE_DOES_NOT_EXIST.to_hresult() => Ok(None),
        Err(e) => Err(format!("{name}: {}", e.message())),
    }
}

pub fn start_type(name: &str) -> Result<Option<StartType>, String> {
    let Some((_scm, svc)) = open(name, SERVICE_QUERY_CONFIG)? else { return Ok(None) };
    let mut needed = 0u32;
    let _ = unsafe { QueryServiceConfigW(svc.0, None, 0, &mut needed) };
    // u64 để vùng đệm canh lề 8 byte cho QUERY_SERVICE_CONFIGW.
    let mut buf = vec![0u64; (needed as usize).div_ceil(8).max(1)];
    let cfg = buf.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW;
    unsafe { QueryServiceConfigW(svc.0, Some(cfg), (buf.len() * 8) as u32, &mut needed) }.map_err(|e| format!("{name}: {}", e.message()))?;
    let st = unsafe { (*cfg).dwStartType };
    match st {
        SERVICE_AUTO_START => Ok(Some(StartType::Auto)),
        SERVICE_DEMAND_START => Ok(Some(StartType::Manual)),
        SERVICE_DISABLED => Ok(Some(StartType::Disabled)),
        other => Err(format!("{name}: unsupported start type {}", other.0)),
    }
}

pub fn set_start_type(name: &str, start: StartType) -> Result<(), String> {
    let Some((_scm, svc)) = open(name, SERVICE_CHANGE_CONFIG)? else { return Err(format!("{name}: service not found")) };
    let st = match start {
        StartType::Auto => SERVICE_AUTO_START,
        StartType::Manual => SERVICE_DEMAND_START,
        StartType::Disabled => SERVICE_DISABLED,
    };
    unsafe {
        ChangeServiceConfigW(
            svc.0,
            ENUM_SERVICE_TYPE(SERVICE_NO_CHANGE),
            st,
            SERVICE_ERROR(SERVICE_NO_CHANGE),
            PCWSTR::null(),
            PCWSTR::null(),
            None,
            PCWSTR::null(),
            PCWSTR::null(),
            PCWSTR::null(),
            PCWSTR::null(),
        )
    }
    .map_err(|e| format!("{name}: {}", e.message()))
}

/// Dịch vụ vốn đã dừng hoặc không tồn tại ⇒ `Ok(())`. Không chờ dừng hẳn.
pub fn stop(name: &str) -> Result<(), String> {
    let Some((_scm, svc)) = open(name, SERVICE_STOP)? else { return Ok(()) };
    let mut status = SERVICE_STATUS::default();
    match unsafe { ControlService(svc.0, SERVICE_CONTROL_STOP, &mut status) } {
        Ok(()) => Ok(()),
        Err(e) if e.code() == ERROR_SERVICE_NOT_ACTIVE.to_hresult() => Ok(()),
        Err(e) => Err(format!("{name}: {}", e.message())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_start_type_without_changing_anything() {
        // Winmgmt (WMI) có trên mọi bản Windows, kể cả runner CI; đọc cấu hình không cần Admin.
        assert!(start_type("Winmgmt").unwrap().is_some());
        assert_eq!(start_type("WinFreeUpNoSuchService").unwrap(), None);
        assert!(stop("WinFreeUpNoSuchService").is_ok());
        assert!(set_start_type("WinFreeUpNoSuchService", StartType::Manual).is_err());
    }
}
