use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use crate::error::Result;
use crate::types::CancelToken;

/// Mọi thao tác chạm hệ thống thật. Bản thật ở `sys_windows::RealSystem`, bản giả ở `testutil::FakeSys`.
pub trait SystemOps: Send + Sync {
    fn is_process_running(&self, exe_name: &str) -> bool;
    /// Ok(true) = dịch vụ đang chạy và đã dừng; Ok(false) = vốn đã dừng.
    fn stop_service(&self, name: &str) -> Result<bool>;
    fn start_service(&self, name: &str) -> Result<()>;
    /// Gọi `on_line` cho từng dòng đầu ra (tách theo `\r` hoặc `\n`, bỏ dòng rỗng). Lỗi nếu mã thoát khác 0/3010.
    fn run_dism(&self, args: &[&str], cancel: &CancelToken, on_line: &mut dyn FnMut(&str)) -> Result<()>;
    /// (tổng byte, số mục) của Thùng rác mọi ổ.
    fn recycle_bin_size(&self) -> Result<(u64, u64)>;
    fn empty_recycle_bin(&self) -> Result<()>;
    fn create_restore_point(&self, description: &str) -> Result<()>;
    fn take_ownership(&self, path: &Path) -> Result<()>;
}

#[derive(Clone)]
pub struct Env {
    pub temp: PathBuf,
    pub windir: PathBuf,
    pub local_appdata: PathBuf,
    pub program_data: PathBuf,
    /// Dạng `C:\` (có dấu gạch chéo cuối).
    pub system_drive: PathBuf,
    pub now: SystemTime,
    pub sys: Arc<dyn SystemOps>,
}
