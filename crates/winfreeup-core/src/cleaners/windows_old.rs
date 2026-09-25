use std::path::PathBuf;

use crate::env::Env;
use crate::error::Result;
use crate::fsclean::{clean_targets, scan_targets, Target};
use crate::types::{CancelToken, CleanOptions, CleanReport, Cleaner, Progress, RiskLevel, ScanResult};

pub fn target(env: &Env) -> Target {
    Target { remove_root: true, ..Target::all(env.system_drive.join("Windows.old")) }
}

pub struct WindowsOld;

impl Cleaner for WindowsOld {
    fn id(&self) -> &'static str {
        "windows_old"
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Risky
    }
    fn default_selected(&self) -> bool {
        false
    }
    fn allowed_roots(&self, env: &Env) -> Vec<PathBuf> {
        vec![target(env).root]
    }
    fn scan(&self, env: &Env, cancel: &CancelToken) -> Result<ScanResult> {
        scan_targets(&[target(env)], env.now, cancel)
    }
    fn clean(&self, env: &Env, _scan: &ScanResult, opts: &CleanOptions, progress: &dyn Progress) -> Result<CleanReport> {
        let t = target(env);
        let mut pre_errors = Vec::new();
        if !opts.dry_run && t.root.exists() {
            if let Err(e) = env.sys.take_ownership(&t.root) {
                pre_errors.push(e.to_string());
            }
        }
        let mut report = clean_targets(&[t], env.now, opts, progress);
        report.errors.splice(0..0, pre_errors);
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{fake_env, write_file, FakeSys};
    use crate::types::{CancelToken, NoProgress};
    use std::sync::Arc;

    #[test]
    fn removes_windows_old_entirely_without_following_junctions() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys::default());
        let env = fake_env(t.path(), sys.clone());
        let old = env.system_drive.join("Windows.old");
        write_file(&old.join(r"Windows\System32\kernel32.dll"), 100);
        write_file(&old.join(r"Program Files\app\a.exe"), 50);
        let users = t.path().join("Users");
        let precious = write_file(&users.join(r"me\Documents\anh-cuoi.jpg"), 9);
        std::fs::create_dir_all(old.join("Documents and Settings").parent().unwrap()).unwrap();
        junction::create(&users, old.join("Documents and Settings")).unwrap();

        let scan = WindowsOld.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan.total_bytes, 150);
        let rep = WindowsOld.clean(&env, &scan, &CleanOptions::default(), &NoProgress).unwrap();
        assert_eq!(rep.bytes_freed, 150);
        assert!(!old.exists(), "Windows.old phải biến mất");
        assert!(precious.exists(), "đích của junction không được đụng");
        assert_eq!(sys.calls(), vec![format!("own:{}", old.display())]);
    }

    // Review Focus 4
    #[test]
    fn missing_windows_old_scans_zero_and_never_takes_ownership() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys::default());
        let env = fake_env(t.path(), sys.clone());
        assert_eq!(WindowsOld.scan(&env, &CancelToken::new()).unwrap().total_bytes, 0);
        let rep = WindowsOld.clean(&env, &ScanResult::default(), &CleanOptions::default(), &NoProgress).unwrap();
        assert!(rep.errors.is_empty());
        assert!(sys.calls().is_empty());
    }

    #[test]
    fn dry_run_keeps_everything_and_skips_ownership() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys::default());
        let env = fake_env(t.path(), sys.clone());
        let f = write_file(&env.system_drive.join(r"Windows.old\x.bin"), 5);
        let rep = WindowsOld.clean(&env, &ScanResult::default(), &CleanOptions { dry_run: true }, &NoProgress).unwrap();
        assert_eq!(rep.bytes_freed, 5);
        assert!(f.exists());
        assert!(sys.calls().is_empty());
    }

    #[test]
    fn metadata_matches_spec_table() {
        assert_eq!((WindowsOld.id(), WindowsOld.risk(), WindowsOld.default_selected()), ("windows_old", RiskLevel::Risky, false));
    }
}
