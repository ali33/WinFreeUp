//! Điều phối: quét song song, quyết định điểm khôi phục, dọn tuần tự có nhật ký và sự kiện.
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use chrono::Local;
use serde::Serialize;

use crate::env::Env;
use crate::error::Result;
use crate::log::CleanLog;
use crate::types::{CancelToken, CleanOptions, CleanReport, Cleaner, ItemAction, Progress, RiskLevel, ScanResult};

#[derive(Debug, Clone, Serialize)]
pub struct GroupScan {
    pub id: String,
    pub risk: RiskLevel,
    pub default_selected: bool,
    pub result: Option<ScanResult>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupClean {
    pub id: String,
    pub report: Option<CleanReport>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CleanEvent {
    Started { id: String },
    Percent { id: String, percent: f32 },
    Finished { result: GroupClean },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RestorePointStatus {
    NotNeeded,
    Created,
    Skipped,
    Failed { message: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanSummary {
    pub groups: Vec<GroupClean>,
    pub log_path: String,
    pub dry_run: bool,
    pub log_write_failed: bool,
}

fn panic_message(p: &(dyn Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        format!("panic: {s}")
    } else if let Some(s) = p.downcast_ref::<String>() {
        format!("panic: {s}")
    } else {
        "panic".to_string()
    }
}

pub fn scan_all(
    cleaners: &[Box<dyn Cleaner>],
    env: &Env,
    cancel: &CancelToken,
    on_done: &(dyn Fn(&GroupScan) + Sync),
) -> Vec<GroupScan> {
    std::thread::scope(|s| {
        let handles: Vec<_> = cleaners
            .iter()
            .map(|c| {
                s.spawn(move || {
                    let outcome = catch_unwind(AssertUnwindSafe(|| c.scan(env, cancel)));
                    let (result, error) = match outcome {
                        Ok(Ok(r)) => (Some(r), None),
                        Ok(Err(e)) => (None, Some(e.to_string())),
                        Err(p) => (None, Some(panic_message(p.as_ref()))),
                    };
                    let g = GroupScan {
                        id: c.id().to_string(),
                        risk: c.risk(),
                        default_selected: c.default_selected(),
                        result,
                        error,
                    };
                    on_done(&g);
                    g
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("scan thread")).collect()
    })
}

pub fn needs_restore_point(cleaners: &[Box<dyn Cleaner>], ids: &[String]) -> bool {
    ids.iter().any(|id| {
        id != "recycle_bin"
            && cleaners.iter().find(|c| c.id() == id).is_some_and(|c| c.risk() != RiskLevel::Safe)
    })
}

pub fn prepare_restore_point(
    cleaners: &[Box<dyn Cleaner>],
    env: &Env,
    ids: &[String],
    dry_run: bool,
) -> RestorePointStatus {
    if !needs_restore_point(cleaners, ids) {
        return RestorePointStatus::NotNeeded;
    }
    if dry_run {
        return RestorePointStatus::Skipped;
    }
    match env.sys.create_restore_point("WinFreeUp") {
        Ok(()) => RestorePointStatus::Created,
        Err(e) => RestorePointStatus::Failed { message: e.to_string() },
    }
}

struct GroupProgress<'a> {
    log: &'a CleanLog,
    id: &'a str,
    on_event: &'a (dyn Fn(CleanEvent) + Sync),
}

impl Progress for GroupProgress<'_> {
    fn item(&self, action: ItemAction, path: &Path, bytes: u64, detail: Option<&str>) {
        let tail = detail.map(|d| format!(" -- {d}")).unwrap_or_default();
        self.log.line(&format!("[{}] {} {} {}{}", self.id, action.as_str(), bytes, path.display(), tail));
    }
    fn percent(&self, pct: f32) {
        (self.on_event)(CleanEvent::Percent { id: self.id.to_string(), percent: pct });
    }
}

pub fn run_clean(
    cleaners: &[Box<dyn Cleaner>],
    env: &Env,
    ids: &[String],
    scans: &HashMap<String, ScanResult>,
    opts: &CleanOptions,
    log_dir: &Path,
    on_event: &(dyn Fn(CleanEvent) + Sync),
) -> Result<CleanSummary> {
    let log = CleanLog::create(log_dir, Local::now())?;
    log.line(&format!("START dry_run={} ids={}", opts.dry_run, ids.join(",")));
    let mut seen = HashSet::new();
    let mut groups = Vec::new();
    let empty = ScanResult::default();
    for id in ids.iter().filter(|id| seen.insert(id.as_str())) {
        on_event(CleanEvent::Started { id: id.clone() });
        log.line(&format!("GROUP {id} START"));
        let g = match cleaners.iter().find(|c| c.id() == id) {
            None => GroupClean { id: id.clone(), report: None, error: Some(format!("unknown cleaner id: {id}")) },
            Some(c) => {
                let progress = GroupProgress { log: &log, id: id.as_str(), on_event };
                let scan = scans.get(id).unwrap_or(&empty);
                match catch_unwind(AssertUnwindSafe(|| c.clean(env, scan, opts, &progress))) {
                    Ok(Ok(r)) => GroupClean { id: id.clone(), report: Some(r), error: None },
                    Ok(Err(e)) => GroupClean { id: id.clone(), report: None, error: Some(e.to_string()) },
                    Err(p) => GroupClean { id: id.clone(), report: None, error: Some(panic_message(p.as_ref())) },
                }
            }
        };
        match (&g.report, &g.error) {
            (Some(r), _) => {
                log.line(&format!(
                    "GROUP {id} END bytes={} deleted={} skipped_locked={} errors={}",
                    r.bytes_freed,
                    r.files_deleted,
                    r.skipped_locked,
                    r.errors.len()
                ));
                for e in &r.errors {
                    log.line(&format!("GROUP {id} ITEM_ERROR {e}"));
                }
            }
            (None, Some(e)) => log.line(&format!("GROUP {id} ERROR {e}")),
            (None, None) => {}
        }
        on_event(CleanEvent::Finished { result: g.clone() });
        groups.push(g);
    }
    log.line("END");
    Ok(CleanSummary {
        groups,
        log_path: log.path().display().to_string(),
        dry_run: opts.dry_run,
        log_write_failed: log.write_failed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{fake_env, FakeCleaner, FakeSys};
    use crate::types::RiskLevel::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    fn env_with(sys: FakeSys) -> (tempfile::TempDir, Arc<FakeSys>, Env) {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(sys);
        let env = fake_env(t.path(), sys.clone());
        (t, sys, env)
    }

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn scan_all_reports_every_group_in_order_and_streams_each() {
        let (_t, _s, env) = env_with(FakeSys::default());
        let cs = vec![FakeCleaner::ok("a", Safe, 1), FakeCleaner::ok("b", Caution, 2), FakeCleaner::ok("c", Risky, 3)];
        let seen = Mutex::new(Vec::new());
        let out = scan_all(&cs, &env, &CancelToken::new(), &|g| seen.lock().unwrap().push(g.id.clone()));
        assert_eq!(out.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(), vec!["a", "b", "c"]);
        assert_eq!(out[1].result.as_ref().unwrap().total_bytes, 2);
        assert!(out[0].default_selected && !out[1].default_selected);
        let mut s = seen.into_inner().unwrap();
        s.sort();
        assert_eq!(s, vec!["a", "b", "c"]);
    }

    #[test]
    fn one_failing_or_panicking_group_does_not_break_the_others() {
        let (_t, _s, env) = env_with(FakeSys::default());
        let cs: Vec<Box<dyn Cleaner>> = vec![
            FakeCleaner::ok("a", Safe, 1),
            Box::new(FakeCleaner { id: "b", risk: Safe, bytes: 0, fail: Some("disk error"), panics: false }),
            Box::new(FakeCleaner { id: "c", risk: Safe, bytes: 0, fail: None, panics: true }),
        ];
        let out = scan_all(&cs, &env, &CancelToken::new(), &|_| {});
        assert!(out[0].result.is_some() && out[0].error.is_none());
        assert_eq!(out[1].error.as_deref(), Some("disk error"));
        assert!(out[2].error.as_deref().unwrap().contains("boom in c"));
    }

    #[test]
    fn cancelled_scan_marks_groups_cancelled() {
        let (_t, _s, env) = env_with(FakeSys::default());
        let c = CancelToken::new();
        c.cancel();
        let out = scan_all(&[FakeCleaner::ok("a", Safe, 1)], &env, &c, &|_| {});
        assert_eq!(out[0].error.as_deref(), Some("cancelled"));
    }

    fn table() -> Vec<Box<dyn Cleaner>> {
        vec![
            FakeCleaner::ok("user_temp", Safe, 1),
            FakeCleaner::ok("browser_cache", Safe, 1),
            FakeCleaner::ok("recycle_bin", Caution, 1),
            FakeCleaner::ok("wu_download", Caution, 1),
            FakeCleaner::ok("windows_old", Risky, 1),
        ]
    }

    #[test]
    fn restore_point_needed_only_for_caution_or_risky_except_recycle_bin() {
        let cs = table();
        assert!(!needs_restore_point(&cs, &ids(&["user_temp", "browser_cache"])));
        assert!(!needs_restore_point(&cs, &ids(&["recycle_bin"])));
        assert!(!needs_restore_point(&cs, &ids(&["recycle_bin", "user_temp"])));
        assert!(needs_restore_point(&cs, &ids(&["wu_download"])));
        assert!(needs_restore_point(&cs, &ids(&["windows_old"])));
        assert!(!needs_restore_point(&cs, &ids(&["khong_co"])));
    }

    #[test]
    fn prepare_restore_point_outcomes() {
        let cs = table();
        let (_t, sys, env) = env_with(FakeSys::default());
        assert_eq!(prepare_restore_point(&cs, &env, &ids(&["user_temp"]), false), RestorePointStatus::NotNeeded);
        assert_eq!(prepare_restore_point(&cs, &env, &ids(&["wu_download"]), true), RestorePointStatus::Skipped);
        assert!(sys.calls().is_empty(), "không tạo điểm khôi phục khi chạy thử");
        assert_eq!(prepare_restore_point(&cs, &env, &ids(&["wu_download"]), false), RestorePointStatus::Created);
        let (_t2, _s2, env2) = env_with(FakeSys { restore_error: Some("System Protection is off".into()), ..Default::default() });
        assert_eq!(
            prepare_restore_point(&cs, &env2, &ids(&["windows_old"]), false),
            RestorePointStatus::Failed { message: "System Protection is off".into() }
        );
    }

    #[test]
    fn run_clean_logs_every_group_emits_events_and_isolates_failures() {
        let (t, _s, env) = env_with(FakeSys::default());
        let cs: Vec<Box<dyn Cleaner>> = vec![
            FakeCleaner::ok("a", Safe, 10),
            Box::new(FakeCleaner { id: "b", risk: Safe, bytes: 0, fail: None, panics: true }),
        ];
        let events = Mutex::new(Vec::new());
        let dir = t.path().join("logs");
        let s = run_clean(&cs, &env, &ids(&["a", "b", "zzz", "a"]), &HashMap::new(), &CleanOptions { dry_run: true }, &dir, &|e| {
            events.lock().unwrap().push(serde_json::to_value(&e).unwrap())
        })
        .unwrap();
        assert!(s.dry_run);
        assert_eq!(s.groups.len(), 3, "trùng id chỉ dọn một lần");
        assert_eq!(s.groups[0].report.as_ref().unwrap().bytes_freed, 10);
        assert!(s.groups[1].error.as_deref().unwrap().contains("boom in b"));
        assert_eq!(s.groups[2].error.as_deref(), Some("unknown cleaner id: zzz"));
        let kinds: Vec<String> = events.lock().unwrap().iter().map(|v| v["kind"].as_str().unwrap().to_string()).collect();
        assert_eq!(kinds, vec!["started", "percent", "finished", "started", "finished", "started", "finished"]);
        let log = std::fs::read_to_string(&s.log_path).unwrap();
        assert!(log.contains("START dry_run=true"));
        assert!(log.contains("[a] DELETED 10 a"));
        assert!(log.contains("GROUP b ERROR"));
        assert!(log.contains("END"));
    }

    /// Đếm số lời gọi `clean` đang chạy đồng thời (dựng cục bộ trong module test này, không
    /// dùng chung `FakeCleaner`): `enter` tăng rồi ghi lại đỉnh cao nhất từng thấy, `exit` giảm.
    #[derive(Default)]
    struct ConcurrencyProbe {
        current: AtomicUsize,
        max_seen: AtomicUsize,
    }

    impl ConcurrencyProbe {
        fn enter(&self) {
            let now = self.current.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_seen.fetch_max(now, Ordering::SeqCst);
        }
        fn exit(&self) {
            self.current.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Cleaner giả có `clean` cố tình chậm (ngủ 20ms): nếu `run_clean` từng dọn song song
    /// (như `scan_all` dùng `thread::scope`), phép ngủ này đủ để hai lời gọi chồng lên nhau và
    /// `ConcurrencyProbe` bắt được đỉnh > 1.
    struct SlowCleaner {
        id: &'static str,
        probe: Arc<ConcurrencyProbe>,
    }

    impl Cleaner for SlowCleaner {
        fn id(&self) -> &'static str {
            self.id
        }
        fn risk(&self) -> RiskLevel {
            Safe
        }
        fn default_selected(&self) -> bool {
            true
        }
        fn allowed_roots(&self, _env: &Env) -> Vec<std::path::PathBuf> {
            vec![]
        }
        fn scan(&self, _env: &Env, _cancel: &CancelToken) -> Result<ScanResult> {
            Ok(ScanResult::default())
        }
        fn clean(&self, _env: &Env, _scan: &ScanResult, _opts: &CleanOptions, _progress: &dyn Progress) -> Result<CleanReport> {
            self.probe.enter();
            thread::sleep(Duration::from_millis(20));
            self.probe.exit();
            Ok(CleanReport::default())
        }
    }

    // Không đổi mã engine: `run_clean` đã là một vòng `for` tuần tự (không `thread::scope` như
    // `scan_all`). Test này chỉ chứng minh điều đó bằng bằng chứng thời gian thực, để một lần
    // sửa sau này lỡ đổi sang chạy song song sẽ bị bắt ngay.
    #[test]
    fn run_clean_runs_groups_sequentially_never_two_clean_calls_overlap() {
        let (t, _s, env) = env_with(FakeSys::default());
        let probe = Arc::new(ConcurrencyProbe::default());
        let cs: Vec<Box<dyn Cleaner>> = vec![
            Box::new(SlowCleaner { id: "a", probe: probe.clone() }),
            Box::new(SlowCleaner { id: "b", probe: probe.clone() }),
            Box::new(SlowCleaner { id: "c", probe: probe.clone() }),
        ];
        let dir = t.path().join("logs");
        let s = run_clean(&cs, &env, &ids(&["a", "b", "c"]), &HashMap::new(), &CleanOptions::default(), &dir, &|_| {}).unwrap();
        assert_eq!(s.groups.len(), 3);
        assert_eq!(probe.max_seen.load(Ordering::SeqCst), 1, "clean phải chạy tuần tự, không đồng thời");
    }

    #[test]
    fn events_serialize_to_the_shape_the_ui_expects() {
        let v = serde_json::to_value(CleanEvent::Percent { id: "component_store".into(), percent: 42.5 }).unwrap();
        assert_eq!(v, serde_json::json!({"kind": "percent", "id": "component_store", "percent": 42.5}));
        let v = serde_json::to_value(RestorePointStatus::Failed { message: "x".into() }).unwrap();
        assert_eq!(v, serde_json::json!({"status": "failed", "message": "x"}));
        let v = serde_json::to_value(RestorePointStatus::NotNeeded).unwrap();
        assert_eq!(v, serde_json::json!({"status": "not_needed"}));
    }
}
