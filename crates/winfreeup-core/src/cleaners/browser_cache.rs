//! Chỉ thư mục cache của từng profile. Không đụng cookie, lịch sử, mật khẩu.
use std::fs;
use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::error::Result;
use crate::fsclean::{clean_targets, roots_of, scan_targets, Target};
use crate::safety::is_reparse_point;
use crate::types::{CancelToken, CleanOptions, CleanReport, Cleaner, Progress, RiskLevel, ScanResult};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Chromium,
    Firefox,
}

const CHROMIUM_CACHE_DIRS: &[&str] = &["Cache", "Code Cache", "GPUCache"];

/// (khoá, tên tiến trình, loại, thư mục chứa các profile)
fn browsers(env: &Env) -> Vec<(&'static str, &'static str, Kind, PathBuf)> {
    let la = &env.local_appdata;
    vec![
        ("chrome", "chrome.exe", Kind::Chromium, la.join(r"Google\Chrome\User Data")),
        ("edge", "msedge.exe", Kind::Chromium, la.join(r"Microsoft\Edge\User Data")),
        ("coccoc", r"CocCoc\Browser\Application\browser.exe", Kind::Chromium, la.join(r"CocCoc\Browser\User Data")),
        ("firefox", "firefox.exe", Kind::Firefox, la.join(r"Mozilla\Firefox\Profiles")),
    ]
}

fn profile_dirs(data_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(data_dir) else { return vec![] };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .filter(|e| fs::symlink_metadata(e.path()).map(|m| m.is_dir() && !is_reparse_point(&m)).unwrap_or(false))
        .map(|e| e.path())
        .collect();
    dirs.sort();
    dirs
}

pub fn targets(env: &Env) -> Vec<Target> {
    let mut out = Vec::new();
    for (_, _, kind, data_dir) in browsers(env) {
        for profile in profile_dirs(&data_dir) {
            match kind {
                Kind::Chromium => {
                    for d in CHROMIUM_CACHE_DIRS {
                        out.push(Target::all(profile.join(d)));
                    }
                }
                Kind::Firefox => out.push(Target::all(profile.join("cache2"))),
            }
        }
    }
    out
}

pub struct BrowserCache;

impl Cleaner for BrowserCache {
    fn id(&self) -> &'static str {
        "browser_cache"
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Safe
    }
    fn default_selected(&self) -> bool {
        true
    }
    fn allowed_roots(&self, env: &Env) -> Vec<PathBuf> {
        roots_of(&targets(env))
    }
    fn scan(&self, env: &Env, cancel: &CancelToken) -> Result<ScanResult> {
        let mut r = scan_targets(&targets(env), env.now, cancel)?;
        for (key, exe, _, data_dir) in browsers(env) {
            if data_dir.exists() && env.sys.is_process_running(exe) {
                r.notices.push(format!("browser_running:{key}"));
            }
        }
        Ok(r)
    }
    fn clean(&self, env: &Env, _scan: &ScanResult, opts: &CleanOptions, progress: &dyn Progress) -> Result<CleanReport> {
        Ok(clean_targets(&targets(env), env.now, opts, progress))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{fake_env, write_file, FakeSys};
    use crate::types::{CancelToken, CleanOptions, NoProgress};
    use std::sync::Arc;

    #[test]
    fn cleans_cache_dirs_of_every_profile_and_keeps_user_data() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys::default()));
        let chrome = env.local_appdata.join(r"Google\Chrome\User Data");
        let c1 = write_file(&chrome.join(r"Default\Cache\Cache_Data\f_000001"), 10);
        let c2 = write_file(&chrome.join(r"Profile 1\Code Cache\js\a"), 20);
        let c3 = write_file(&chrome.join(r"Default\GPUCache\data_1"), 30);
        let cookies = write_file(&chrome.join(r"Default\Network\Cookies"), 5);
        let history = write_file(&chrome.join(r"Default\History"), 5);
        let logins = write_file(&chrome.join(r"Default\Login Data"), 5);
        let coccoc = write_file(&env.local_appdata.join(r"CocCoc\Browser\User Data\Default\Cache\x"), 40);
        let edge = write_file(&env.local_appdata.join(r"Microsoft\Edge\User Data\Default\Cache\y"), 50);
        let ff = env.local_appdata.join(r"Mozilla\Firefox\Profiles\abc.default-release");
        let ff_cache = write_file(&ff.join(r"cache2\entries\E1"), 60);
        let ff_places = write_file(&ff.join("places.sqlite"), 5);

        let c = BrowserCache;
        let scan = c.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan.total_bytes, 10 + 20 + 30 + 40 + 50 + 60);
        c.clean(&env, &scan, &CleanOptions::default(), &NoProgress).unwrap();
        for gone in [&c1, &c2, &c3, &coccoc, &edge, &ff_cache] {
            assert!(!gone.exists(), "{}", gone.display());
        }
        for kept in [&cookies, &history, &logins, &ff_places] {
            assert!(kept.exists(), "{}", kept.display());
        }
        assert!(chrome.join(r"Default\Cache").exists(), "thư mục gốc cache được giữ");
    }

    #[test]
    fn running_installed_browser_produces_notice() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys { running: vec!["chrome.exe".into(), "firefox.exe".into()], ..Default::default() });
        let env = fake_env(t.path(), sys);
        write_file(&env.local_appdata.join(r"Google\Chrome\User Data\Default\Cache\x"), 1);
        let scan = BrowserCache.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan.notices, vec!["browser_running:chrome".to_string()]);
    }

    #[test]
    fn coccoc_is_detected_by_image_path_not_by_bare_browser_exe() {
        let t = tempfile::tempdir().unwrap();
        let data = r"CocCoc\Browser\User Data\Default\Cache\x";
        // Một "browser.exe" bất kỳ (không phải Cốc Cốc) không được coi là Cốc Cốc đang mở.
        let env = fake_env(t.path(), Arc::new(FakeSys { running: vec!["browser.exe".into()], ..Default::default() }));
        write_file(&env.local_appdata.join(data), 1);
        assert!(BrowserCache.scan(&env, &CancelToken::new()).unwrap().notices.is_empty());
        let running = vec![r"CocCoc\Browser\Application\browser.exe".into()];
        let env = fake_env(t.path(), Arc::new(FakeSys { running, ..Default::default() }));
        let scan = BrowserCache.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan.notices, vec!["browser_running:coccoc".to_string()]);
    }

    // Review Focus 4
    #[test]
    fn no_browser_installed_scans_zero_without_notice() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys::default()));
        let scan = BrowserCache.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan, ScanResult::default());
    }

    #[test]
    fn metadata_matches_spec_table() {
        assert_eq!((BrowserCache.id(), BrowserCache.risk(), BrowserCache.default_selected()), ("browser_cache", RiskLevel::Safe, true));
    }
}
