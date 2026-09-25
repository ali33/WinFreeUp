//! Khuôn chung của bộ quét ổ đĩa và tiến độ.
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use winfreeup_core::{CancelToken, Result};

use super::tree::DiskTree;

/// Payload sự kiện `disk-scan-progress`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScanStatus {
    pub files: u64,
    pub bytes: u64,
    pub current: String,
    /// MFT biết trước tổng số bản ghi nên có %; duyệt thư mục thì không (`None`).
    pub percent: Option<f32>,
}

pub trait ScanProgress: Sync {
    fn report(&self, status: &ScanStatus);
}

impl<F: Fn(&ScanStatus) + Sync> ScanProgress for F {
    fn report(&self, status: &ScanStatus) {
        self(status)
    }
}

/// Spec 3.1 `DiskScanner`. `root` là gốc ổ dạng `C:\`.
pub trait DiskScanner: Sync {
    fn scan(&self, root: &Path, cancel: &CancelToken, progress: &dyn ScanProgress) -> Result<DiskTree>;
}

/// Giãn nhịp báo tiến độ: tối đa một lần mỗi `every`, để giao diện không bị ngập sự kiện.
pub struct Throttle {
    every: Duration,
    last: Mutex<Option<Instant>>,
}

impl Throttle {
    pub fn new(every: Duration) -> Self {
        Throttle { every, last: Mutex::new(None) }
    }

    pub fn ready(&self) -> bool {
        let Ok(mut last) = self.last.lock() else { return false };
        let now = Instant::now();
        match *last {
            Some(t) if now.duration_since(t) < self.every => false,
            _ => {
                *last = Some(now);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throttle_lets_the_first_call_through_then_waits() {
        let t = Throttle::new(Duration::from_secs(60));
        assert!(t.ready());
        assert!(!t.ready());
        let t0 = Throttle::new(Duration::ZERO);
        assert!(t0.ready() && t0.ready());
    }
}
