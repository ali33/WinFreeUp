//! Thùng rác mọi ổ qua SHEmptyRecycleBinW (trong RealSystem). Không lấy lại được.
use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::error::Result;
use crate::types::{CancelToken, CleanOptions, CleanReport, Cleaner, ItemAction, Progress, RiskLevel, ScanResult};

const LOG_LABEL: &str = "Recycle Bin";

pub struct RecycleBin;

impl Cleaner for RecycleBin {
    fn id(&self) -> &'static str {
        "recycle_bin"
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Caution
    }
    fn default_selected(&self) -> bool {
        false
    }
    fn allowed_roots(&self, _env: &Env) -> Vec<PathBuf> {
        vec![]
    }
    fn scan(&self, env: &Env, _cancel: &CancelToken) -> Result<ScanResult> {
        let (bytes, items) = env.sys.recycle_bin_size()?;
        Ok(ScanResult { total_bytes: bytes, file_count: items, ..Default::default() })
    }
    fn clean(&self, env: &Env, scan: &ScanResult, opts: &CleanOptions, progress: &dyn Progress) -> Result<CleanReport> {
        let report = CleanReport {
            bytes_freed: scan.total_bytes,
            files_deleted: scan.file_count,
            dry_run: opts.dry_run,
            ..Default::default()
        };
        if opts.dry_run {
            progress.item(ItemAction::WouldDelete, Path::new(LOG_LABEL), scan.total_bytes, None);
            return Ok(report);
        }
        env.sys.empty_recycle_bin()?;
        progress.item(ItemAction::Deleted, Path::new(LOG_LABEL), scan.total_bytes, None);
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{fake_env, FakeSys};
    use crate::types::NoProgress;
    use std::sync::Arc;

    #[test]
    fn scan_reads_size_and_count() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys { recycle: (5000, 3), ..Default::default() }));
        let r = RecycleBin.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!((r.total_bytes, r.file_count), (5000, 3));
    }

    #[test]
    fn clean_empties_the_bin() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys::default());
        let env = fake_env(t.path(), sys.clone());
        let scan = ScanResult { total_bytes: 5000, file_count: 3, ..Default::default() };
        let rep = RecycleBin.clean(&env, &scan, &CleanOptions::default(), &NoProgress).unwrap();
        assert_eq!((rep.bytes_freed, rep.files_deleted), (5000, 3));
        assert_eq!(sys.calls(), vec!["rb_empty"]);
    }

    #[test]
    fn dry_run_does_not_empty() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys::default());
        let env = fake_env(t.path(), sys.clone());
        let scan = ScanResult { total_bytes: 10, file_count: 1, ..Default::default() };
        let rep = RecycleBin.clean(&env, &scan, &CleanOptions { dry_run: true }, &NoProgress).unwrap();
        assert_eq!(rep.bytes_freed, 10);
        assert!(sys.calls().is_empty());
    }

    #[test]
    fn metadata_matches_spec_table() {
        assert_eq!((RecycleBin.id(), RecycleBin.risk(), RecycleBin.default_selected()), ("recycle_bin", RiskLevel::Caution, false));
        assert!(RecycleBin.allowed_roots(&fake_env(tempfile::tempdir().unwrap().path(), Arc::new(FakeSys::default()))).is_empty());
    }
}
