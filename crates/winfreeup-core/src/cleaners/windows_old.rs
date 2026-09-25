use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::error::Result;
use crate::fsclean::{clean_targets, scan_targets, Target};
use crate::types::{CancelToken, CleanOptions, CleanReport, Cleaner, Progress, RiskLevel, ScanResult};

pub fn target(env: &Env) -> Target {
    Target { remove_root: true, ..Target::all(env.system_drive.join("Windows.old")) }
}

pub struct WindowsOld;

/// `take_ownership` chỉ trả lỗi GỘP TỪNG MỤC (`"take_ownership <gốc>: N error(s): …"`, tạo bởi
/// `ErrorSummary::into_result` trong sys_windows.rs) sau khi đã kiểm xong gốc và duyệt hết cây —
/// khi đó gốc đáng tin, dọn tiếp phần còn lại là đúng. Mọi dạng khác (gốc là liên kết, tổ tiên là
/// liên kết, chủ sở hữu gốc không phải SYSTEM/TrustedInstaller/Administrators, không mở được gốc,
/// không lấy được đặc quyền…) đều là lỗi MỨC GỐC ⇒ không được xóa gì. Nhận diện theo cách chặt:
/// đúng tiền tố có đường dẫn gốc, rồi một số nguyên, rồi đúng `" error(s): "`; không khớp ⇒ chặn.
fn is_per_item_summary(msg: &str, root: &Path) -> bool {
    let Some(rest) = msg.strip_prefix(&format!("take_ownership {}: ", root.display())) else {
        return false;
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && rest[digits..].starts_with(" error(s): ")
}

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
                let msg = e.to_string();
                // Lỗi mức gốc: Windows.old có thể do người dùng thường tự tạo (chủ là user) — xóa nó
                // bằng quyền Admin là xóa dữ liệu người dùng. Trả lỗi cho cả nhóm, không dọn gì.
                if !is_per_item_summary(&msg, &t.root) {
                    return Err(e);
                }
                pre_errors.push(msg);
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
    fn root_level_ownership_refusal_blocks_the_whole_group_and_deletes_nothing() {
        let t = tempfile::tempdir().unwrap();
        let old = t.path().join("Windows.old");
        let sys = Arc::new(FakeSys {
            own_error: Some(format!(
                "take_ownership {}: refusing: the root is owned by S-1-5-21-1111111111-2222222222-3333333333-1001, \
                 not SYSTEM, TrustedInstaller or Administrators",
                old.display()
            )),
            ..Default::default()
        });
        let env = fake_env(t.path(), sys.clone());
        let a = write_file(&old.join(r"Users\me\Documents\luan-van.docx"), 100);
        let b = write_file(&old.join("anh.jpg"), 7);
        let err = WindowsOld.clean(&env, &ScanResult::default(), &CleanOptions::default(), &NoProgress).unwrap_err();
        assert!(err.to_string().contains("refusing"), "{err}");
        assert!(a.exists() && b.exists(), "không được xóa gì khi gốc bị từ chối");
        assert_eq!(sys.calls(), vec![format!("own:{}", old.display())]);
    }

    #[test]
    fn any_unrecognised_ownership_error_also_blocks() {
        let t = tempfile::tempdir().unwrap();
        let old = t.path().join("Windows.old");
        for msg in [
            "refusing: the root is owned by S-1-5-21-1-2-3-1001".to_string(),
            format!("take_ownership {}: refusing: the root is a reparse point", old.display()),
            format!("take_ownership {}: thread impersonation: Access is denied. (os error 5)", old.display()),
            format!("take_ownership {}: error(s): x", old.display()),
            format!("take_ownership {}\\khac: 2 error(s): x", old.display()),
        ] {
            let sys = Arc::new(FakeSys { own_error: Some(msg.clone()), ..Default::default() });
            let env = fake_env(t.path(), sys);
            let f = write_file(&old.join("x.bin"), 5);
            assert!(WindowsOld.clean(&env, &ScanResult::default(), &CleanOptions::default(), &NoProgress).is_err(), "{msg}");
            assert!(f.exists(), "{msg}");
        }
    }

    #[test]
    fn per_item_ownership_errors_still_clean_the_rest() {
        let t = tempfile::tempdir().unwrap();
        let old = t.path().join("Windows.old");
        let summary = format!("take_ownership {}: 2 error(s): a: denied | b: denied [SeBackupPrivilege]", old.display());
        let sys = Arc::new(FakeSys { own_error: Some(summary.clone()), ..Default::default() });
        let env = fake_env(t.path(), sys);
        write_file(&old.join(r"Windows\a.dll"), 10);
        let rep = WindowsOld.clean(&env, &ScanResult::default(), &CleanOptions::default(), &NoProgress).unwrap();
        assert_eq!(rep.bytes_freed, 10);
        assert!(!old.exists());
        assert_eq!(rep.errors.first(), Some(&summary));
    }

    #[test]
    fn metadata_matches_spec_table() {
        assert_eq!((WindowsOld.id(), WindowsOld.risk(), WindowsOld.default_selected()), ("windows_old", RiskLevel::Risky, false));
    }
}
