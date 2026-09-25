//! Bật/tắt tác vụ theo lịch qua COM `ITaskService` (không phân tích chữ của `schtasks`, vốn đổi theo ngôn ngữ Windows).
use windows::core::BSTR;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, VARIANT_BOOL};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::System::TaskScheduler::{IRegisteredTask, ITaskService, TaskScheduler};
use windows::Win32::System::Variant::VARIANT;

/// Khởi tạo COM cho luồng hiện tại; chỉ gỡ nếu chính mình đã khởi tạo thành công.
struct Com(bool);

impl Com {
    fn init() -> Com {
        Com(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

fn not_found(e: &windows::core::Error) -> bool {
    e.code() == ERROR_FILE_NOT_FOUND.to_hresult() || e.code() == ERROR_PATH_NOT_FOUND.to_hresult()
}

/// `\A\B\Tên` ⇒ (`\A\B`, `Tên`); `\Tên` ⇒ (`\`, `Tên`).
pub fn split_task_path(path: &str) -> Result<(String, String), String> {
    match path.rsplit_once('\\') {
        Some((folder, name)) if !name.is_empty() => Ok((if folder.is_empty() { "\\".into() } else { folder.into() }, name.into())),
        _ => Err(format!("bad task path: {path}")),
    }
}

fn with_task<T>(path: &str, f: impl FnOnce(&IRegisteredTask) -> Result<T, String>) -> Result<Option<T>, String> {
    let (folder, name) = split_task_path(path)?;
    let _com = Com::init();
    let svc: ITaskService = unsafe { CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER) }.map_err(|e| format!("TaskScheduler: {}", e.message()))?;
    let empty = VARIANT::default();
    unsafe { svc.Connect(&empty, &empty, &empty, &empty) }.map_err(|e| format!("TaskScheduler: {}", e.message()))?;
    let folder = match unsafe { svc.GetFolder(&BSTR::from(folder.as_str())) } {
        Ok(f) => f,
        Err(e) if not_found(&e) => return Ok(None),
        Err(e) => return Err(format!("{path}: {}", e.message())),
    };
    let task = match unsafe { folder.GetTask(&BSTR::from(name.as_str())) } {
        Ok(t) => t,
        Err(e) if not_found(&e) => return Ok(None),
        Err(e) => return Err(format!("{path}: {}", e.message())),
    };
    f(&task).map(Some)
}

/// `Ok(None)` = tác vụ không có trên máy.
pub fn enabled(path: &str) -> Result<Option<bool>, String> {
    with_task(path, |t| unsafe { t.Enabled() }.map(|b| b.as_bool()).map_err(|e| format!("{path}: {}", e.message())))
}

pub fn set_enabled(path: &str, on: bool) -> Result<(), String> {
    match with_task(path, |t| unsafe { t.SetEnabled(VARIANT_BOOL::from(on)) }.map_err(|e| format!("{path}: {}", e.message())))? {
        Some(()) => Ok(()),
        None => Err(format!("{path}: task not found")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_paths() {
        assert_eq!(split_task_path(r"\Microsoft\Windows\A B\C").unwrap(), (r"\Microsoft\Windows\A B".into(), "C".into()));
        assert_eq!(split_task_path(r"\Top").unwrap(), (r"\".into(), "Top".into()));
        assert!(split_task_path(r"\Folder\").is_err());
    }

    #[test]
    fn reads_task_state_without_changing_anything() {
        assert_eq!(enabled(r"\WinFreeUpNoSuchFolder\X").unwrap(), None);
        assert_eq!(enabled(r"\Microsoft\Windows\WinFreeUpNoSuchTask").unwrap(), None);
        assert!(set_enabled(r"\WinFreeUpNoSuchFolder\X", true).is_err());
        // Tác vụ chống phân mảnh có trên mọi bản Windows client và Server có Desktop Experience.
        assert!(enabled(r"\Microsoft\Windows\Defrag\ScheduledDefrag").unwrap().is_some());
    }
}
