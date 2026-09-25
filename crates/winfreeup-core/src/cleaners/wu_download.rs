use std::path::PathBuf;

use crate::env::Env;
use crate::error::Result;
use crate::fsclean::{clean_targets, roots_of, scan_targets, Target};
use crate::service::ServiceGuard;
use crate::types::{CancelToken, CleanOptions, CleanReport, Cleaner, Progress, RiskLevel, ScanResult};

pub const SERVICE: &str = "wuauserv";

pub fn targets(env: &Env) -> Vec<Target> {
    vec![Target::all(env.windir.join(r"SoftwareDistribution\Download"))]
}

pub struct WuDownload;

impl Cleaner for WuDownload {
    fn id(&self) -> &'static str {
        "wu_download"
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Caution
    }
    fn default_selected(&self) -> bool {
        false
    }
    fn allowed_roots(&self, env: &Env) -> Vec<PathBuf> {
        roots_of(&targets(env))
    }
    fn scan(&self, env: &Env, cancel: &CancelToken) -> Result<ScanResult> {
        scan_targets(&targets(env), env.now, cancel)
    }
    fn clean(&self, env: &Env, _scan: &ScanResult, opts: &CleanOptions, progress: &dyn Progress) -> Result<CleanReport> {
        if opts.dry_run {
            return Ok(clean_targets(&targets(env), env.now, opts, progress));
        }
        let guard = ServiceGuard::stop(env.sys.as_ref(), SERVICE)?;
        let mut report = clean_targets(&targets(env), env.now, opts, progress);
        if let Err(e) = guard.finish() {
            report.errors.push(format!("could not restart {SERVICE}: {e}"));
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{fake_env, write_file, FakeSys};
    use crate::types::{CancelToken, NoProgress};
    use std::sync::Arc;

    fn setup(sys: FakeSys) -> (tempfile::TempDir, Arc<FakeSys>, Env, std::path::PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(sys);
        let env = fake_env(t.path(), sys.clone());
        let f = write_file(&env.windir.join(r"SoftwareDistribution\Download\abc\update.cab"), 70);
        (t, sys, env, f)
    }

    #[test]
    fn stops_cleans_and_restarts_wuauserv() {
        let (_t, sys, env, f) = setup(FakeSys { service_was_running: true, ..Default::default() });
        let scan = WuDownload.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan.total_bytes, 70);
        let rep = WuDownload.clean(&env, &scan, &CleanOptions::default(), &NoProgress).unwrap();
        assert_eq!(rep.bytes_freed, 70);
        assert!(!f.exists());
        assert_eq!(sys.calls(), vec!["stop:wuauserv", "start:wuauserv"]);
    }

    #[test]
    fn restart_failure_is_reported_not_swallowed() {
        let (_t, _sys, env, _f) = setup(FakeSys { service_was_running: true, start_fails: true, ..Default::default() });
        let rep = WuDownload.clean(&env, &ScanResult::default(), &CleanOptions::default(), &NoProgress).unwrap();
        assert_eq!(rep.errors.len(), 1);
        assert!(rep.errors[0].contains("wuauserv"), "{:?}", rep.errors);
    }

    #[test]
    fn dry_run_touches_neither_service_nor_files() {
        let (_t, sys, env, f) = setup(FakeSys { service_was_running: true, ..Default::default() });
        let rep = WuDownload.clean(&env, &ScanResult::default(), &CleanOptions { dry_run: true }, &NoProgress).unwrap();
        assert_eq!(rep.bytes_freed, 70);
        assert!(f.exists());
        assert!(sys.calls().is_empty());
    }

    #[test]
    fn metadata_matches_spec_table() {
        assert_eq!((WuDownload.id(), WuDownload.risk(), WuDownload.default_selected()), ("wu_download", RiskLevel::Caution, false));
    }
}
