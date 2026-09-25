use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::env::Env;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Safe,
    Caution,
    Risky,
}

pub const TOP_ITEMS: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ScanResult {
    pub total_bytes: u64,
    pub file_count: u64,
    pub top_items: Vec<Item>,
    /// true khi số byte là ước tính (DISM).
    pub estimated: bool,
    /// Mã thông báo cho giao diện dịch, vd "browser_running:chrome".
    pub notices: Vec<String>,
}

impl ScanResult {
    pub fn add_file(&mut self, path: &Path, bytes: u64) {
        self.total_bytes += bytes;
        self.file_count += 1;
        if bytes == 0 {
            return;
        }
        let pos = self.top_items.partition_point(|i| i.bytes >= bytes);
        if pos >= TOP_ITEMS {
            return;
        }
        self.top_items.insert(pos, Item { path: path.display().to_string(), bytes });
        self.top_items.truncate(TOP_ITEMS);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CleanReport {
    pub bytes_freed: u64,
    pub files_deleted: u64,
    pub skipped_locked: u64,
    pub errors: Vec<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CleanOptions {
    pub dry_run: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemAction {
    Deleted,
    WouldDelete,
    SkippedLocked,
    Failed,
}

impl ItemAction {
    pub fn as_str(self) -> &'static str {
        match self {
            ItemAction::Deleted => "DELETED",
            ItemAction::WouldDelete => "WOULD_DELETE",
            ItemAction::SkippedLocked => "SKIPPED_LOCKED",
            ItemAction::Failed => "FAILED",
        }
    }
}

pub trait Progress: Send + Sync {
    fn item(&self, action: ItemAction, path: &Path, bytes: u64, detail: Option<&str>);
    fn percent(&self, pct: f32);
}

pub struct NoProgress;

impl Progress for NoProgress {
    fn item(&self, _: ItemAction, _: &Path, _: u64, _: Option<&str>) {}
    fn percent(&self, _: f32) {}
}

pub trait Cleaner: Send + Sync {
    fn id(&self) -> &'static str;
    fn risk(&self) -> RiskLevel;
    fn default_selected(&self) -> bool;
    fn allowed_roots(&self, env: &Env) -> Vec<PathBuf>;
    fn scan(&self, env: &Env, cancel: &CancelToken) -> Result<ScanResult>;
    fn clean(
        &self,
        env: &Env,
        scan: &ScanResult,
        opts: &CleanOptions,
        progress: &dyn Progress,
    ) -> Result<CleanReport>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn scan_result_keeps_twenty_largest_sorted() {
        let mut r = ScanResult::default();
        for i in 1..=30u64 {
            r.add_file(Path::new(&format!("C:/t/{i}.tmp")), i * 10);
        }
        assert_eq!(r.file_count, 30);
        assert_eq!(r.total_bytes, (1..=30u64).map(|i| i * 10).sum::<u64>());
        assert_eq!(r.top_items.len(), TOP_ITEMS);
        assert_eq!(r.top_items[0].bytes, 300);
        assert_eq!(r.top_items[19].bytes, 110);
        assert!(r.top_items.windows(2).all(|w| w[0].bytes >= w[1].bytes));
    }

    #[test]
    fn zero_byte_entries_are_counted_but_not_listed() {
        let mut r = ScanResult::default();
        r.add_file(Path::new("C:/t/link"), 0);
        assert_eq!(r.file_count, 1);
        assert!(r.top_items.is_empty());
    }

    #[test]
    fn risk_level_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&RiskLevel::Caution).unwrap(), "\"caution\"");
        assert_eq!(serde_json::to_string(&RiskLevel::Risky).unwrap(), "\"risky\"");
    }

    #[test]
    fn cancel_token_clones_share_state() {
        let a = CancelToken::new();
        let b = a.clone();
        assert!(!b.is_cancelled());
        a.cancel();
        assert!(b.is_cancelled());
    }

    #[test]
    fn item_action_labels_are_stable() {
        assert_eq!(ItemAction::Deleted.as_str(), "DELETED");
        assert_eq!(ItemAction::WouldDelete.as_str(), "WOULD_DELETE");
        assert_eq!(ItemAction::SkippedLocked.as_str(), "SKIPPED_LOCKED");
        assert_eq!(ItemAction::Failed.as_str(), "FAILED");
    }
}
