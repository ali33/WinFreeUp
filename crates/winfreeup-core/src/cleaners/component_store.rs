//! DISM /StartComponentCleanup. CẤM /ResetBase. Dung lượng quét là ước tính từ /AnalyzeComponentStore.
use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::error::{CoreError, Result};
use crate::types::{CancelToken, CleanOptions, CleanReport, Cleaner, ItemAction, Progress, RiskLevel, ScanResult};

/// `/English` để đầu ra không bị dịch theo ngôn ngữ Windows.
pub const ANALYZE_ARGS: &[&str] = &["/Online", "/English", "/Cleanup-Image", "/AnalyzeComponentStore"];
pub const CLEANUP_ARGS: &[&str] = &["/Online", "/English", "/Cleanup-Image", "/StartComponentCleanup"];
const LOG_LABEL: &str = "DISM /StartComponentCleanup";

pub fn parse_size(s: &str) -> Option<u64> {
    let mut parts = s.split_whitespace();
    let n: f64 = parts.next()?.parse().ok()?;
    let mult: f64 = match parts.next()?.to_ascii_lowercase().as_str() {
        "bytes" | "byte" => 1.0,
        "kb" => 1024.0,
        "mb" => 1024.0 * 1024.0,
        "gb" => 1024.0 * 1024.0 * 1024.0,
        "tb" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((n * mult) as u64)
}

pub fn parse_analyze(out: &str) -> Option<u64> {
    let (mut backups, mut cache, mut recommended) = (None, None, false);
    for line in out.lines() {
        let Some((k, v)) = line.split_once(" : ") else { continue };
        match k.trim() {
            "Backups and Disabled Features" => backups = parse_size(v),
            "Cache and Temporary Data" => cache = parse_size(v),
            "Component Store Cleanup Recommended" => recommended = v.trim().eq_ignore_ascii_case("yes"),
            _ => {}
        }
    }
    let (backups, cache) = (backups?, cache?);
    Some(if recommended { backups + cache } else { cache })
}

pub fn parse_percent(line: &str) -> Option<f32> {
    let end = line.find('%')?;
    let start = line[..end]
        .rfind(|c: char| !(c.is_ascii_digit() || c == '.'))
        .map(|i| i + 1)
        .unwrap_or(0);
    line[start..end].parse().ok()
}

pub struct ComponentStore;

impl Cleaner for ComponentStore {
    fn id(&self) -> &'static str {
        "component_store"
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
    fn scan(&self, env: &Env, cancel: &CancelToken) -> Result<ScanResult> {
        let mut out = String::new();
        env.sys.run_dism(ANALYZE_ARGS, cancel, &mut |l| {
            out.push_str(l);
            out.push('\n');
        })?;
        let bytes = parse_analyze(&out)
            .ok_or_else(|| CoreError::System("unrecognized DISM /AnalyzeComponentStore output".into()))?;
        Ok(ScanResult { total_bytes: bytes, estimated: true, ..Default::default() })
    }
    fn clean(&self, env: &Env, scan: &ScanResult, opts: &CleanOptions, progress: &dyn Progress) -> Result<CleanReport> {
        if opts.dry_run {
            progress.item(ItemAction::WouldDelete, Path::new(LOG_LABEL), scan.total_bytes, None);
            return Ok(CleanReport { bytes_freed: scan.total_bytes, dry_run: true, ..Default::default() });
        }
        let never = CancelToken::new();
        env.sys.run_dism(CLEANUP_ARGS, &never, &mut |l| {
            if let Some(p) = parse_percent(l) {
                progress.percent(p);
            }
        })?;
        progress.percent(100.0);
        progress.item(ItemAction::Deleted, Path::new(LOG_LABEL), scan.total_bytes, None);
        Ok(CleanReport { bytes_freed: scan.total_bytes, ..Default::default() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{fake_env, FakeSys, Recorder};
    use crate::types::NoProgress;
    use std::sync::Arc;

    const SAMPLE: &str = "Deployment Image Servicing and Management tool\r\nVersion: 10.0.26100.1\r\n\r\nImage Version: 10.0.26100.1742\r\n\r\n[==========================100.0%==========================]\r\nComponent Store (WinSxS) information:\r\n\r\nWindows Explorer Reported Size of Component Store : 8.21 GB\r\n\r\nActual Size of Component Store : 7.95 GB\r\n\r\n    Shared with Windows : 5.72 GB\r\n    Backups and Disabled Features : 2.05 GB\r\n    Cache and Temporary Data :  176.54 MB\r\n\r\nDate of Last Cleanup : 2026-08-10 12:30:12\r\n\r\nNumber of Reclaimable Packages : 3\r\nComponent Store Cleanup Recommended : Yes\r\n\r\nThe operation completed successfully.\r\n";

    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MB: f64 = 1024.0 * 1024.0;

    #[test]
    fn parses_sizes_with_units() {
        assert_eq!(parse_size("2.05 GB"), Some((2.05 * GB) as u64));
        assert_eq!(parse_size(" 176.54 MB"), Some((176.54 * MB) as u64));
        assert_eq!(parse_size("512 bytes"), Some(512));
        assert_eq!(parse_size("abc"), None);
    }

    #[test]
    fn estimate_is_backups_plus_cache_when_recommended() {
        assert_eq!(parse_analyze(SAMPLE), Some((2.05 * GB) as u64 + (176.54 * MB) as u64));
    }

    #[test]
    fn estimate_is_cache_only_when_not_recommended() {
        let s = SAMPLE.replace("Recommended : Yes", "Recommended : No");
        assert_eq!(parse_analyze(&s), Some((176.54 * MB) as u64));
    }

    #[test]
    fn unrecognised_output_is_none() {
        assert_eq!(parse_analyze("Error: 740\r\nElevated permissions are required"), None);
    }

    #[test]
    fn parses_progress_percent() {
        assert_eq!(parse_percent("[=====                      10.0%                          ]"), Some(10.0));
        assert_eq!(parse_percent("[==========================100.0%==========================]"), Some(100.0));
        assert_eq!(parse_percent("The operation completed successfully."), None);
    }

    #[test]
    fn scan_is_an_estimate_from_analyze() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys { dism_output: SAMPLE.into(), ..Default::default() });
        let env = fake_env(t.path(), sys.clone());
        let r = ComponentStore.scan(&env, &CancelToken::new()).unwrap();
        assert!(r.estimated);
        assert!(r.total_bytes > 0);
        assert_eq!(sys.calls(), vec![format!("dism:{}", ANALYZE_ARGS.join(" "))]);
    }

    #[test]
    fn scan_fails_loudly_on_unrecognised_output() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys { dism_output: "garbage".into(), ..Default::default() }));
        assert!(ComponentStore.scan(&env, &CancelToken::new()).is_err());
    }

    #[test]
    fn clean_reports_percent_and_never_uses_resetbase() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys { dism_output: "[==  10.0%  ]\r[======  55.5%  ]\rThe operation completed successfully.".into(), ..Default::default() });
        let env = fake_env(t.path(), sys.clone());
        let rec = Recorder::default();
        let scan = ScanResult { total_bytes: 1000, estimated: true, ..Default::default() };
        let rep = ComponentStore.clean(&env, &scan, &CleanOptions::default(), &rec).unwrap();
        assert_eq!(rep.bytes_freed, 1000);
        assert_eq!(*rec.percents.lock().unwrap(), vec![10.0, 55.5, 100.0]);
        let calls = sys.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].contains("/StartComponentCleanup"));
        assert!(!calls.iter().any(|c| c.to_lowercase().contains("resetbase")));
        assert!(!CLEANUP_ARGS.iter().chain(ANALYZE_ARGS).any(|a| a.to_lowercase().contains("resetbase")));
    }

    #[test]
    fn dry_run_never_calls_dism() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys::default());
        let env = fake_env(t.path(), sys.clone());
        let scan = ScanResult { total_bytes: 7, ..Default::default() };
        let rep = ComponentStore.clean(&env, &scan, &CleanOptions { dry_run: true }, &NoProgress).unwrap();
        assert_eq!((rep.bytes_freed, rep.dry_run), (7, true));
        assert!(sys.calls().is_empty());
    }

    #[test]
    fn dism_failure_is_an_error_for_the_group() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys { dism_fails: true, ..Default::default() }));
        assert!(ComponentStore.clean(&env, &ScanResult::default(), &CleanOptions::default(), &NoProgress).is_err());
    }
}
