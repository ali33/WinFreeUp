//! Duyệt cây thư mục (không đi theo reparse point) và dọn theo danh sách `Target`.
use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::env::Env;
use crate::error::{io_err, CoreError, Result};
use crate::safety::{delete_path, is_reparse_point, remove_empty_dir, DeleteOutcome, Guard};
use crate::types::{
    CancelToken, CleanOptions, CleanReport, Cleaner, ItemAction, Progress, RiskLevel, ScanResult,
};

pub const DAY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filter {
    All,
    OlderThan(Duration),
    NamePrefix(&'static str),
}

#[derive(Debug, Clone)]
pub struct Target {
    pub root: PathBuf,
    pub filter: Filter,
    pub recursive: bool,
    pub prune_empty_dirs: bool,
    pub remove_root: bool,
}

impl Target {
    pub fn all(root: PathBuf) -> Target {
        Target { root, filter: Filter::All, recursive: true, prune_empty_dirs: true, remove_root: false }
    }
    pub fn older_than(root: PathBuf, age: Duration) -> Target {
        Target { filter: Filter::OlderThan(age), ..Target::all(root) }
    }
    pub fn prefixed(root: PathBuf, prefix: &'static str) -> Target {
        Target { root, filter: Filter::NamePrefix(prefix), recursive: false, prune_empty_dirs: false, remove_root: false }
    }
}

#[derive(Debug, Clone)]
pub struct Found {
    pub path: PathBuf,
    pub bytes: u64,
    pub modified: SystemTime,
    pub is_link: bool,
}

fn is_old(modified: SystemTime, age: Duration, now: SystemTime) -> bool {
    now.duration_since(modified).map(|d| d >= age).unwrap_or(false)
}

impl Filter {
    pub fn matches(&self, f: &Found, now: SystemTime) -> bool {
        match self {
            Filter::All => true,
            Filter::OlderThan(age) => is_old(f.modified, *age, now),
            Filter::NamePrefix(p) => f
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase().starts_with(&p.to_lowercase()))
                .unwrap_or(false),
        }
    }

    fn may_prune_dir(&self, d: &Found, now: SystemTime) -> bool {
        match self {
            Filter::All => true,
            Filter::OlderThan(age) => is_old(d.modified, *age, now),
            Filter::NamePrefix(_) => false,
        }
    }
}

pub struct Walk {
    pub files: Vec<Found>,
    pub dirs: Vec<Found>,
    pub unreadable: u64,
}

fn found(path: PathBuf, meta: &fs::Metadata) -> Found {
    let is_link = is_reparse_point(meta);
    // Không đọc được giờ ⇒ coi như mới ⇒ không bị lọc "cũ hơn 24 giờ" xóa nhầm.
    let modified = meta.modified().unwrap_or_else(|_| SystemTime::now());
    // Bộ cài bung file vào %TEMP% giữ nguyên mtime cũ trong gói, nhưng creation time là lúc
    // vừa bung ra ⇒ lấy thời điểm MỚI HƠN giữa mtime và creation time làm tuổi, để không
    // coi file vừa bung là "cũ" rồi xóa nhầm. Không đọc được creation time ⇒ dùng mtime.
    let created = meta.created().unwrap_or(modified);
    Found {
        bytes: if is_link || meta.is_dir() { 0 } else { meta.len() },
        modified: modified.max(created),
        is_link,
        path,
    }
}

/// Liệt kê file (kể cả liên kết, coi như "file" 0 byte) và thư mục con. Không bao giờ đi vào reparse point.
/// Gốc là reparse point ⇒ bỏ qua cả gốc. Gốc là file ⇒ chính nó là ứng viên.
pub fn walk(root: &Path, recursive: bool, cancel: Option<&CancelToken>) -> Result<Walk> {
    let mut w = Walk { files: vec![], dirs: vec![], unreadable: 0 };
    let meta = match fs::symlink_metadata(root) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(w),
        Err(e) => return Err(io_err(root, e)),
    };
    if is_reparse_point(&meta) {
        return Ok(w);
    }
    if !meta.is_dir() {
        w.files.push(found(root.to_path_buf(), &meta));
        return Ok(w);
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if cancel.is_some_and(|c| c.is_cancelled()) {
            return Err(CoreError::Cancelled);
        }
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => {
                w.unreadable += 1;
                continue;
            }
        };
        for entry in entries {
            let Ok(entry) = entry else {
                w.unreadable += 1;
                continue;
            };
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                w.unreadable += 1;
                continue;
            };
            if is_reparse_point(&meta) {
                w.files.push(found(path, &meta));
            } else if meta.is_dir() {
                if recursive {
                    w.dirs.push(found(path.clone(), &meta));
                    stack.push(path);
                }
            } else {
                w.files.push(found(path, &meta));
            }
        }
    }
    Ok(w)
}

pub fn roots_of(targets: &[Target]) -> Vec<PathBuf> {
    targets.iter().map(|t| t.root.clone()).collect()
}

pub fn scan_targets(targets: &[Target], now: SystemTime, cancel: &CancelToken) -> Result<ScanResult> {
    let mut r = ScanResult::default();
    // Guard chỉ sống trong lời gọi này (không lưu lại): xét gốc nào là reparse point hoặc có
    // tổ tiên là junction/ổ ánh xạ, để bỏ qua trước khi quét thay vì đếm nhầm byte dưới đó.
    let guard = Guard::new(&roots_of(targets));
    for t in targets {
        if guard.rejected_roots().iter().any(|p| p == &t.root) {
            r.notices.push(format!("root_rejected:{}", t.root.display()));
            continue;
        }
        let w = match walk(&t.root, t.recursive, Some(cancel)) {
            Ok(w) => w,
            // Hủy quét là tín hiệu toàn cục ⇒ dừng ngay, không chỉ bỏ qua gốc này.
            Err(CoreError::Cancelled) => return Err(CoreError::Cancelled),
            // Gốc lỗi (vd AccessDenied) không được làm hỏng cả nhóm quét, giống clean_targets.
            Err(_) => {
                r.notices.push(format!("root_unreadable:{}", t.root.display()));
                continue;
            }
        };
        for f in w.files.iter().filter(|f| t.filter.matches(f, now)) {
            r.add_file(&f.path, f.bytes);
        }
    }
    Ok(r)
}

pub fn clean_targets(targets: &[Target], now: SystemTime, opts: &CleanOptions, progress: &dyn Progress) -> CleanReport {
    // Guard chỉ sống trong lời gọi này (không lưu vào struct dài hạn): gốc bị loại thì bỏ qua cả
    // target, ghi một dòng lỗi duy nhất thay vì mỗi file dưới nó một lỗi OutsideRoots.
    let guard = Guard::new(&roots_of(targets));
    let mut rep = CleanReport { dry_run: opts.dry_run, ..Default::default() };
    for t in targets {
        if guard.rejected_roots().iter().any(|p| p == &t.root) {
            rep.errors.push(format!("root_rejected:{}", t.root.display()));
            continue;
        }
        let mut w = match walk(&t.root, t.recursive, None) {
            Ok(w) => w,
            Err(e) => {
                // Lỗi thô (vd "C:\...: Access is denied. (os error 5)") không được lộ ra giao
                // diện (lộ đường dẫn/hệ thống, người dùng không đọc được) — mã hoá bằng tiền tố
                // "root_unreadable:" giống scan_targets để catalog.ts::noticeText dịch được, còn
                // thông điệp gốc vẫn vào nhật ký qua progress (ItemAction::Failed).
                progress.item(ItemAction::Failed, &t.root, 0, Some(&e.to_string()));
                rep.errors.push(format!("root_unreadable:{}", t.root.display()));
                continue;
            }
        };
        // Thư mục/entry không đọc được (vd AccessDenied khi liệt kê) không phải là "file bị khóa"
        // (đó là DeleteOutcome::Locked ở dưới) nên không cộng vào skipped_locked — ghi số lượng
        // vào errors để không mất thông tin nhưng cũng không lẫn ý nghĩa với "locked".
        if w.unreadable > 0 {
            rep.errors.push(format!("unreadable_entries:{}", w.unreadable));
        }
        for f in w.files.iter().filter(|f| t.filter.matches(f, now)) {
            match delete_path(&guard, &f.path, opts.dry_run) {
                Ok(DeleteOutcome::Deleted(b)) => {
                    rep.bytes_freed += b;
                    rep.files_deleted += 1;
                    progress.item(ItemAction::Deleted, &f.path, b, None);
                }
                Ok(DeleteOutcome::WouldDelete(b)) => {
                    rep.bytes_freed += b;
                    rep.files_deleted += 1;
                    progress.item(ItemAction::WouldDelete, &f.path, b, None);
                }
                Ok(DeleteOutcome::Locked) => {
                    rep.skipped_locked += 1;
                    progress.item(ItemAction::SkippedLocked, &f.path, f.bytes, None);
                }
                Ok(DeleteOutcome::Vanished) => {}
                Err(e) => {
                    let msg = e.to_string();
                    progress.item(ItemAction::Failed, &f.path, 0, Some(&msg));
                    rep.errors.push(msg);
                }
            }
        }
        if opts.dry_run {
            continue;
        }
        if t.prune_empty_dirs {
            w.dirs.sort_by_key(|d| Reverse(d.path.components().count()));
            for d in w.dirs.iter().filter(|d| t.filter.may_prune_dir(d, now)) {
                remove_empty_dir(&guard, &d.path);
            }
        }
        if t.remove_root {
            remove_empty_dir(&guard, &t.root);
        }
    }
    rep
}

/// Nhóm dọn chỉ gồm file trong các `Target` (không cần dịch vụ hay lệnh hệ thống).
pub struct FileCleaner {
    pub id: &'static str,
    pub risk: RiskLevel,
    pub default_selected: bool,
    pub targets: fn(&Env) -> Vec<Target>,
}

impl Cleaner for FileCleaner {
    fn id(&self) -> &'static str {
        self.id
    }
    fn risk(&self) -> RiskLevel {
        self.risk
    }
    fn default_selected(&self) -> bool {
        self.default_selected
    }
    fn allowed_roots(&self, env: &Env) -> Vec<PathBuf> {
        roots_of(&(self.targets)(env))
    }
    fn scan(&self, env: &Env, cancel: &CancelToken) -> Result<ScanResult> {
        scan_targets(&(self.targets)(env), env.now, cancel)
    }
    fn clean(&self, env: &Env, _scan: &ScanResult, opts: &CleanOptions, progress: &dyn Progress) -> Result<CleanReport> {
        Ok(clean_targets(&(self.targets)(env), env.now, opts, progress))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{age, fake_env, write_file, FakeSys, Recorder};
    use crate::types::NoProgress;
    use std::fs;
    use std::os::windows::fs::OpenOptionsExt;
    use std::sync::Arc;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn scan_counts_only_files_older_than_the_limit() {
        let t = tmp();
        let root = t.path().join("Temp");
        let old = write_file(&root.join("old.tmp"), 100);
        write_file(&root.join("new.tmp"), 50);
        age(&old, 48);
        let r = scan_targets(&[Target::older_than(root, DAY)], SystemTime::now(), &CancelToken::new()).unwrap();
        assert_eq!((r.total_bytes, r.file_count), (100, 1));
        assert_eq!(r.top_items.len(), 1);
    }

    // Review Focus 1 (mục 1): bộ cài bung file giữ mtime cũ trong gói, nhưng file vừa được tạo
    // (creation time mới) ⇒ KHÔNG được coi là cũ, dù mtime đã lùi qua ngưỡng older_than. Cố ý
    // KHÔNG dùng testutil::age ở đây (nó lùi cả creation time) mà chỉ lùi mtime bằng filetime
    // trực tiếp, để tái tạo đúng tình huống "vừa bung file, mtime cũ nhưng creation time mới".
    #[test]
    fn freshly_created_file_with_stale_mtime_is_not_matched_as_old() {
        let t = tmp();
        let root = t.path().join("Temp");
        let f = write_file(&root.join("just-unpacked.tmp"), 10);
        let old_mtime = SystemTime::now() - Duration::from_secs(48 * 3600);
        filetime::set_file_mtime(&f, filetime::FileTime::from_system_time(old_mtime)).unwrap();
        let meta = fs::symlink_metadata(&f).unwrap();
        let found = found(f.clone(), &meta);
        assert!(!Filter::OlderThan(DAY).matches(&found, SystemTime::now()));
        let r = scan_targets(&[Target::older_than(root, DAY)], SystemTime::now(), &CancelToken::new()).unwrap();
        assert_eq!((r.total_bytes, r.file_count), (0, 0));
    }

    // Review Focus 4
    #[test]
    fn missing_root_scans_as_empty_not_error() {
        let t = tmp();
        let r = scan_targets(&[Target::all(t.path().join("khong-co"))], SystemTime::now(), &CancelToken::new()).unwrap();
        assert_eq!(r, ScanResult::default());
        let rep = clean_targets(&[Target::all(t.path().join("khong-co"))], SystemTime::now(), &CleanOptions::default(), &NoProgress);
        assert!(rep.errors.is_empty());
    }

    #[test]
    fn scan_stops_when_cancelled() {
        let t = tmp();
        write_file(&t.path().join("r").join("a"), 1);
        let c = CancelToken::new();
        c.cancel();
        let err = scan_targets(&[Target::all(t.path().join("r"))], SystemTime::now(), &c).unwrap_err();
        assert!(matches!(err, CoreError::Cancelled));
    }

    #[test]
    fn clean_deletes_old_files_prunes_old_empty_dirs_keeps_fresh_ones_and_root() {
        let t = tmp();
        let root = t.path().join("Temp");
        let old_file = write_file(&root.join("old_dir").join("a.tmp"), 10);
        let new_file = write_file(&root.join("new.tmp"), 20);
        let old_in_fresh = write_file(&root.join("fresh_dir").join("b.tmp"), 30);
        age(&old_file, 48);
        age(&old_in_fresh, 48);
        age(&root.join("old_dir"), 48);
        let rec = Recorder::default();
        let rep = clean_targets(&[Target::older_than(root.clone(), DAY)], SystemTime::now(), &CleanOptions::default(), &rec);
        assert_eq!(rep.bytes_freed, 40);
        assert_eq!(rep.files_deleted, 2);
        assert!(!old_file.exists() && !old_in_fresh.exists());
        assert!(new_file.exists());
        assert!(!root.join("old_dir").exists());
        assert!(root.join("fresh_dir").exists());
        assert!(root.exists());
        assert_eq!(rec.actions(), vec![ItemAction::Deleted, ItemAction::Deleted]);
    }

    #[test]
    fn dry_run_deletes_nothing_but_reports_what_it_would_free() {
        let t = tmp();
        let root = t.path().join("c");
        let a = write_file(&root.join("sub").join("a"), 10);
        let b = write_file(&root.join("b"), 5);
        let rec = Recorder::default();
        let rep = clean_targets(&[Target::all(root.clone())], SystemTime::now(), &CleanOptions { dry_run: true }, &rec);
        assert!(rep.dry_run);
        assert_eq!((rep.bytes_freed, rep.files_deleted), (15, 2));
        assert!(a.exists() && b.exists() && root.join("sub").exists());
        assert!(rec.actions().iter().all(|x| *x == ItemAction::WouldDelete));
    }

    #[test]
    fn junction_inside_root_is_not_traversed() {
        let t = tmp();
        let root = t.path().join("cache");
        let outside = t.path().join("Documents");
        let precious = write_file(&outside.join("luan-van.docx"), 999);
        fs::create_dir_all(&root).unwrap();
        junction::create(&outside, root.join("link")).unwrap();
        let scan = scan_targets(&[Target::all(root.clone())], SystemTime::now(), &CancelToken::new()).unwrap();
        assert_eq!(scan.total_bytes, 0);
        clean_targets(&[Target::all(root.clone())], SystemTime::now(), &CleanOptions::default(), &NoProgress);
        assert!(precious.exists());
        assert!(fs::symlink_metadata(root.join("link")).is_err());
    }

    // Gốc bản thân là junction (vd AppData chuyển ổ bằng junction): Guard::new loại nó ngay khi
    // dựng, nên scan_targets phải bỏ qua trước khi quét (không đếm byte dưới đích junction) và
    // phát đúng 1 notice để giao diện dịch cho người dùng.
    #[test]
    fn root_that_is_a_junction_is_skipped_by_scan_with_a_notice() {
        let t = tmp();
        let outside = t.path().join("that-that");
        write_file(&outside.join("secret.txt"), 999);
        let fake_root = t.path().join("Temp");
        junction::create(&outside, &fake_root).unwrap();
        let r = scan_targets(&[Target::all(fake_root.clone())], SystemTime::now(), &CancelToken::new()).unwrap();
        assert_eq!((r.total_bytes, r.file_count), (0, 0));
        assert_eq!(r.notices, vec![format!("root_rejected:{}", fake_root.display())]);
    }

    #[test]
    fn root_that_is_a_junction_is_skipped_by_clean_with_one_error_line_and_deletes_nothing() {
        let t = tmp();
        let outside = t.path().join("that-that");
        let secret = write_file(&outside.join("secret.txt"), 999);
        let fake_root = t.path().join("Temp");
        junction::create(&outside, &fake_root).unwrap();
        let rep = clean_targets(&[Target::all(fake_root.clone())], SystemTime::now(), &CleanOptions::default(), &NoProgress);
        assert_eq!(rep.errors, vec![format!("root_rejected:{}", fake_root.display())]);
        assert_eq!((rep.bytes_freed, rep.files_deleted), (0, 0));
        assert!(secret.exists());
    }

    #[test]
    fn locked_file_is_counted_and_the_rest_still_cleaned() {
        let t = tmp();
        let root = t.path().join("c");
        let busy = write_file(&root.join("busy"), 5);
        let free = write_file(&root.join("free"), 7);
        let _h = fs::OpenOptions::new().read(true).share_mode(0).open(&busy).unwrap();
        let rep = clean_targets(&[Target::all(root)], SystemTime::now(), &CleanOptions::default(), &NoProgress);
        assert_eq!(rep.skipped_locked, 1);
        assert_eq!(rep.bytes_freed, 7);
        assert!(busy.exists() && !free.exists());
        assert!(rep.errors.is_empty());
    }

    #[test]
    fn file_root_is_treated_as_a_single_candidate() {
        let t = tmp();
        let dump = write_file(&t.path().join("MEMORY.DMP"), 64);
        let rep = clean_targets(&[Target::all(dump.clone())], SystemTime::now(), &CleanOptions::default(), &NoProgress);
        assert_eq!(rep.bytes_freed, 64);
        assert!(!dump.exists());
    }

    #[test]
    fn remove_root_deletes_the_emptied_root() {
        let t = tmp();
        let root = t.path().join("Windows.old");
        write_file(&root.join("a").join("b").join("c"), 3);
        let target = Target { remove_root: true, ..Target::all(root.clone()) };
        clean_targets(&[target], SystemTime::now(), &CleanOptions::default(), &NoProgress);
        assert!(!root.exists());
    }

    #[test]
    fn prefixed_target_matches_prefix_only_and_does_not_recurse() {
        let t = tmp();
        let root = t.path().join("Explorer");
        let thumb = write_file(&root.join("thumbcache_256.db"), 8);
        let icon = write_file(&root.join("iconcache_16.db"), 8);
        let nested = write_file(&root.join("sub").join("thumbcache_1.db"), 8);
        clean_targets(&[Target::prefixed(root, "thumbcache_")], SystemTime::now(), &CleanOptions::default(), &NoProgress);
        assert!(!thumb.exists());
        assert!(icon.exists() && nested.exists());
    }

    // Review Focus 3
    #[test]
    fn vanished_file_between_scan_and_clean_is_not_an_error() {
        let t = tmp();
        let sys = Arc::new(FakeSys::default());
        let env = fake_env(t.path(), sys);
        let f = write_file(&env.temp.join("x.tmp"), 9);
        age(&f, 48);
        let c = FileCleaner { id: "t", risk: RiskLevel::Safe, default_selected: true, targets: |e| vec![Target::older_than(e.temp.clone(), DAY)] };
        let scan = c.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan.total_bytes, 9);
        fs::remove_file(&f).unwrap();
        let rep = c.clean(&env, &scan, &CleanOptions::default(), &NoProgress).unwrap();
        assert!(rep.errors.is_empty());
        assert_eq!(rep.files_deleted, 0);
    }

    // Review Focus 2 (mục 2): gốc lỗi khác NotFound không được làm hỏng cả nhóm quét — chỉ gốc
    // đó bị bỏ qua và ghi vào notices, Target khác vẫn quét bình thường, giống clean_targets đã
    // ghi lỗi rồi đi tiếp. Mô phỏng bằng tên đường dẫn không hợp lệ (ký tự `?*|`) vì đó là cách
    // đáng tin cậy và không cần đổi ACL để tạo lỗi khác NotFound (đổi ACL có rủi ro để lại thư
    // mục bị khóa nếu assert thất bại giữa chừng); đã xác nhận kind() != NotFound trước khi quét.
    //
    // Từ khi scan_targets dựng Guard cho các gốc (mục "gốc bị loại"), Guard mở gốc này cũng lỗi
    // (không phải "vanished") nên bị xếp vào rejected_roots TRƯỚC KHI walk() chạy tới nhánh lỗi
    // của riêng nó ⇒ notice bây giờ là "root_rejected" thay vì "root_unreadable". Bất biến cần
    // giữ (gốc lỗi không làm hỏng cả nhóm quét, gốc khác vẫn quét bình thường) không đổi.
    #[test]
    fn root_with_unreadable_metadata_is_skipped_with_a_notice_other_targets_still_scan() {
        let t = tmp();
        let bad_root = t.path().join("ten-khong-hop-le-?-*-|");
        assert_ne!(fs::symlink_metadata(&bad_root).unwrap_err().kind(), std::io::ErrorKind::NotFound);
        let ok_root = t.path().join("Temp");
        write_file(&ok_root.join("keep.tmp"), 5);
        let r = scan_targets(&[Target::all(bad_root.clone()), Target::all(ok_root)], SystemTime::now(), &CancelToken::new()).unwrap();
        assert_eq!((r.total_bytes, r.file_count), (5, 1));
        assert!(
            r.notices.iter().any(|n| *n == format!("root_rejected:{}", bad_root.display())),
            "notices: {:?}",
            r.notices
        );
    }
}
