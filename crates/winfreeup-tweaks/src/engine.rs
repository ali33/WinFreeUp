//! Đọc trạng thái, áp dụng, hoàn tác (spec mục 3.3–3.4). Mỗi thao tác hỏng độc lập; mọi thao tác
//! có ảnh chụp được lưu xuống đĩa TRƯỚC khi chạm hệ thống.
use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::blocklist::is_blocked_package;
use crate::model::{default_data, to_reg_data, Group, Level, Op, Restart, Risk, StartType, Tweak};
use crate::ops::{packages_of, SystemInfo, TweakOps};
use crate::state::{op_state, supported, tweak_status, OpState, TweakStatus};
use crate::undo::{Snapshot, UndoStore};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TweakView {
    pub id: String,
    pub group: Group,
    pub level: Level,
    pub risk: Risk,
    pub needs_restart: Restart,
    #[serde(flatten)]
    pub status: TweakStatus,
    pub has_undo: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadResult {
    pub system: SystemInfo,
    pub tweaks: Vec<TweakView>,
    /// Mã thông báo cho giao diện: `undo_corrupt:<chi tiết>`.
    pub notices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TweakOutcome {
    pub id: String,
    /// Trạng thái đọc lại từ máy sau khi chạy.
    #[serde(flatten)]
    pub status: TweakStatus,
    /// Lỗi nguyên văn, dạng `<id>#<chỉ số thao tác>: <thông điệp>`.
    pub errors: Vec<String>,
    /// ProductId đã mở trang Store (hoàn tác app đã gỡ).
    pub store_opened: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunReport {
    pub outcomes: Vec<TweakOutcome>,
    /// Mức khởi động lại nặng nhất trong các mục đã thay đổi máy.
    pub restart: Restart,
    pub notices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TweakEvent {
    Started { id: String, index: usize, total: usize },
    Finished { outcome: TweakOutcome },
}

/// Một lượt áp dụng/hoàn tác tại một thời điểm (cùng file ảnh chụp, cùng máy). Chỉ trong MỘT tiến
/// trình — hai tiến trình WinFreeUp vẫn chồng nhau được, vỏ Tauri (Task 12) phải bảo đảm chạy một bản.
static RUN_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Id đưa vào lỗi/nhật ký: id lạ (không có trong danh mục) chỉ được echo nếu đúng dạng id danh mục.
fn shown_id(id: &str) -> String {
    let ok = (1..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if ok { id.to_string() } else { "<invalid>".into() }
}

fn load_undo(path: &Path, notices: &mut Vec<String>) -> UndoStore {
    let (u, warn) = UndoStore::load(path);
    if let Some(w) = warn {
        notices.push(format!("undo_corrupt:{w}"));
    }
    u
}

pub fn read_all(catalog: &[Tweak], ops: &dyn TweakOps, undo_path: &Path) -> Result<ReadResult, String> {
    let system = ops.system_info()?;
    let mut notices = Vec::new();
    let undo = load_undo(undo_path, &mut notices);
    let tweaks = catalog
        .iter()
        .map(|t| {
            let (status, errors) = tweak_status(t, &system, ops, &undo);
            TweakView {
                id: t.id.clone(),
                group: t.group,
                level: t.level,
                risk: t.risk,
                needs_restart: t.needs_restart,
                status,
                has_undo: undo.has_any(&t.id),
                errors,
            }
        })
        .collect();
    Ok(ReadResult { system, tweaks, notices })
}

/// Ghi ảnh chụp (nếu chưa có) xuống đĩa. Không lưu được ⇒ `Err` và thao tác KHÔNG được chạy.
/// `Ok(true)` = vừa ghi mới; `Ok(false)` = đã có từ trước (giữ bản gốc).
fn snapshot(undo: &mut UndoStore, id: &str, i: usize, snap: Snapshot) -> Result<bool, String> {
    if !undo.record_if_absent(id, i, snap) {
        return Ok(false);
    }
    if let Err(e) = undo.save() {
        undo.remove(id, i);
        return Err(format!("undo_save: {e}"));
    }
    Ok(true)
}

/// Chạy một thao tác áp dụng. `Ok(true)` = đã thay đổi máy.
fn apply_op(ops: &dyn TweakOps, undo: &mut UndoStore, id: &str, i: usize, op: &Op, all_users: bool, errors: &mut Vec<String>) -> Result<bool, String> {
    match op {
        Op::RegistrySet { path, name, value_type, value, .. } => {
            let target = to_reg_data(*value_type, value)?;
            let cur = ops.reg_read(path, name)?;
            snapshot(undo, id, i, Snapshot::Registry { data: cur.clone() })?;
            if cur.as_ref() == Some(&target) {
                return Ok(false);
            }
            ops.reg_write(path, name, &target)?;
            Ok(true)
        }
        Op::RegistryDelete { path, name } => {
            let cur = ops.reg_read(path, name)?;
            let absent = cur.is_none();
            snapshot(undo, id, i, Snapshot::Registry { data: cur })?;
            if absent {
                return Ok(false);
            }
            ops.reg_delete(path, name)?;
            Ok(true)
        }
        Op::ServiceStartup { service, start, .. } => {
            let Some(cur) = ops.service_start(service)? else { return Ok(false) };
            snapshot(undo, id, i, Snapshot::Service { start: cur })?;
            let changed = cur != *start;
            if changed {
                ops.set_service_start(service, *start)?;
            }
            if *start == StartType::Disabled {
                if let Err(e) = ops.stop_service(service) {
                    errors.push(format!("{id}#{i}: {e}"));
                }
            }
            Ok(changed)
        }
        Op::ScheduledTaskDisable { task } => {
            let Some(enabled) = ops.task_enabled(task)? else { return Ok(false) };
            snapshot(undo, id, i, Snapshot::Task { enabled })?;
            if !enabled {
                return Ok(false);
            }
            ops.set_task_enabled(task, false)?;
            Ok(true)
        }
        Op::AppxRemove { package_family, .. } => {
            if is_blocked_package(package_family) {
                return Err(format!("blocked:{package_family}"));
            }
            let mut changed = false;
            let mut blocked = false;
            // Ảnh chụp của thao tác này được ghi mới trong lượt này (không phải bản gốc từ lượt trước).
            let mut fresh = false;
            // Phòng thủ: lớp thật lọc lỏng (vd theo tên) thì không gỡ nhầm gói khác family.
            for p in packages_of(ops, package_family)? {
                if is_blocked_package(&p.family) || p.is_framework || p.non_removable {
                    blocked = true;
                    errors.push(format!("{id}#{i}: blocked:{}", p.family));
                    continue;
                }
                fresh |= snapshot(undo, id, i, Snapshot::Appx { family: p.family.clone() })?;
                match ops.remove_package(&p.full_name, all_users) {
                    Ok(()) => changed = true,
                    Err(e) => errors.push(format!("{id}#{i}: {e}")),
                }
            }
            if all_users && !blocked {
                // Gỡ khỏi ảnh cài đặt cũng cần đường quay lại (nút «Cài lại từ Store»), kể cả khi
                // tài khoản hiện tại không có gói.
                fresh |= snapshot(undo, id, i, Snapshot::Appx { family: package_family.clone() })?;
                let deprovisioned = match ops.deprovision(package_family) {
                    Ok(d) => d,
                    Err(e) => {
                        errors.push(format!("{id}#{i}: {e}"));
                        false
                    }
                };
                changed |= deprovisioned;
                // Lượt này không gỡ được gì mà ảnh chụp là mới ⇒ bỏ, kẻo mục hiện «Đã gỡ» và hoàn tác
                // mở Store cho thứ WinFreeUp chưa từng gỡ.
                if fresh && !changed {
                    undo.remove(id, i);
                    if let Err(e) = undo.save() {
                        errors.push(format!("{id}#{i}: undo_save: {e}"));
                    }
                }
            }
            Ok(changed)
        }
    }
}

enum Reverted {
    /// Đã ghi lại giá trị cũ ⇒ xoá ảnh chụp của thao tác; máy có đổi.
    Done,
    /// Không cần ghi gì ⇒ chỉ xoá ảnh chụp, máy không đổi: máy đã ở giá trị cũ sẵn (người dùng tự trả,
    /// app đã cài lại…), ảnh chụp tác vụ vốn tắt (hoàn tác không bật), hoặc registry_delete mà value vốn
    /// không có.
    Cleared,
    /// Không làm gì (không có gì để trả).
    Nothing,
    /// Đã mở trang Store; giữ ảnh chụp cho tới khi app được cài lại.
    StoreOpened(String),
}

/// `store_seen`: ProductId đã mở trang Store trong lượt này — nhiều gói cùng ProductId (vd Game Bar
/// 3 gói) chỉ mở một lần; các thao tác sau vẫn giữ ảnh chụp.
/// Không có ảnh chụp ⇒ chỉ trả `default` khi thao tác đang ở đích (`Match`); giá trị khác đích là
/// của người dùng/công cụ khác, không đụng.
fn revert_op(ops: &dyn TweakOps, snap: Option<&Snapshot>, op: &Op, store_seen: &mut BTreeSet<String>) -> Result<Reverted, String> {
    if snap.is_none() && !matches!(op, Op::RegistryDelete { .. } | Op::AppxRemove { .. }) && op_state(ops, op, false)? != OpState::Match {
        return Ok(Reverted::Nothing);
    }
    match op {
        Op::RegistrySet { path, name, value_type, default, .. } => {
            let want = match snap {
                Some(Snapshot::Registry { data }) => data.clone(),
                _ => default_data(*value_type, default)?,
            };
            if ops.reg_read(path, name)? == want {
                return Ok(Reverted::Cleared);
            }
            match want {
                Some(d) => ops.reg_write(path, name, &d)?,
                None => ops.reg_delete(path, name)?,
            }
            Ok(Reverted::Done)
        }
        Op::RegistryDelete { path, name } => match snap {
            Some(Snapshot::Registry { data: Some(d) }) => {
                if ops.reg_read(path, name)?.as_ref() == Some(d) {
                    return Ok(Reverted::Cleared);
                }
                ops.reg_write(path, name, d).map(|_| Reverted::Done)
            }
            Some(Snapshot::Registry { data: None }) => Ok(Reverted::Cleared),
            // Ảnh chụp khác loại (file bị sửa tay, danh mục đổi thứ tự thao tác) ⇒ báo, giữ ảnh chụp.
            Some(_) => Err("snapshot_mismatch".into()),
            None => Err("no_snapshot".into()),
        },
        Op::ServiceStartup { service, default, .. } => {
            let Some(cur) = ops.service_start(service)? else { return Ok(Reverted::Nothing) };
            let want = match snap {
                Some(Snapshot::Service { start }) => *start,
                _ => *default,
            };
            if cur == want {
                return Ok(Reverted::Cleared);
            }
            ops.set_service_start(service, want).map(|_| Reverted::Done)
        }
        Op::ScheduledTaskDisable { task } => {
            let Some(cur) = ops.task_enabled(task)? else { return Ok(Reverted::Nothing) };
            let want = match snap {
                Some(Snapshot::Task { enabled }) => *enabled,
                _ => true,
            };
            if !want || cur {
                return Ok(Reverted::Cleared);
            }
            ops.set_task_enabled(task, true)?;
            Ok(Reverted::Done)
        }
        Op::AppxRemove { package_family, store_product_id } => {
            if !packages_of(ops, package_family)?.is_empty() {
                return Ok(Reverted::Cleared);
            }
            if snap.is_none() || store_seen.contains(store_product_id) {
                return Ok(Reverted::Nothing);
            }
            ops.open_uri(&format!("ms-windows-store://pdp/?ProductId={store_product_id}"))?;
            store_seen.insert(store_product_id.clone());
            Ok(Reverted::StoreOpened(store_product_id.clone()))
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Apply { all_users: bool },
    Revert,
}

fn run(catalog: &[Tweak], ops: &dyn TweakOps, undo_path: &Path, ids: &[String], mode: Mode, on_event: &dyn Fn(&TweakEvent)) -> RunReport {
    // Hai lượt chồng nhau (bấm hai lần, hai cửa sổ) sẽ đọc-ghi chéo file ảnh chụp.
    let _guard = RUN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Id trùng chỉ chạy một lần (giữ thứ tự lần đầu): hoàn tác lần hai sẽ không còn ảnh chụp.
    let mut unique = BTreeSet::new();
    let ids: Vec<&String> = ids.iter().filter(|id| unique.insert(id.as_str())).collect();
    let mut notices = Vec::new();
    let mut undo = load_undo(undo_path, &mut notices);
    let mut outcomes = Vec::new();
    let mut restart = Restart::None;
    let sys = ops.system_info();
    let mut store_seen = BTreeSet::new();
    for (index, &id) in ids.iter().enumerate() {
        let tweak = catalog.iter().find(|t| &t.id == id);
        let shown = if tweak.is_some() { id.clone() } else { shown_id(id) };
        on_event(&TweakEvent::Started { id: shown.clone(), index, total: ids.len() });
        let mut errors = Vec::new();
        let mut store_opened = Vec::new();
        let outcome = match (tweak, &sys) {
            (None, _) => TweakOutcome { id: shown.clone(), status: TweakStatus::NotApplied, errors: vec![format!("{shown}: unknown_id")], store_opened },
            (_, Err(e)) => TweakOutcome { id: id.clone(), status: TweakStatus::NotApplied, errors: vec![format!("{id}: {e}")], store_opened },
            (Some(t), Ok(sys)) => {
                let (before, _) = tweak_status(t, sys, ops, &undo);
                // Mục «không hỗ trợ» (sai build/edition…) mà có ảnh chụp ⇒ chỉ cho hoàn tác, không cho
                // áp dụng lại.
                let allowed = match mode {
                    Mode::Apply { .. } => before.actionable() && supported(t, sys).is_ok(),
                    Mode::Revert => before.actionable() || (matches!(before, TweakStatus::Unsupported { .. }) && undo.has_any(id)),
                };
                if !allowed {
                    errors.push(format!("{id}: not_allowed"));
                } else {
                    let mut changed = false;
                    match mode {
                        Mode::Apply { all_users } => {
                            for (i, op) in t.ops.iter().enumerate() {
                                match apply_op(ops, &mut undo, id, i, op, all_users, &mut errors) {
                                    Ok(c) => changed |= c,
                                    Err(e) => errors.push(format!("{id}#{i}: {e}")),
                                }
                            }
                        }
                        Mode::Revert => {
                            for (i, op) in t.ops.iter().enumerate().rev() {
                                match revert_op(ops, undo.get(id, i), op, &mut store_seen) {
                                    Ok(Reverted::Done) => {
                                        changed = true;
                                        undo.remove(id, i);
                                    }
                                    Ok(Reverted::Cleared) => undo.remove(id, i),
                                    Ok(Reverted::Nothing) => {}
                                    Ok(Reverted::StoreOpened(pid)) => store_opened.push(pid),
                                    Err(e) => errors.push(format!("{id}#{i}: {e}")),
                                }
                            }
                            if let Err(e) = undo.save() {
                                errors.push(format!("{id}: undo_save: {e}"));
                            }
                        }
                    }
                    if changed {
                        restart = restart.max(t.needs_restart);
                    }
                }
                let (after, read_errors) = tweak_status(t, sys, ops, &undo);
                errors.extend(read_errors);
                TweakOutcome { id: id.clone(), status: after, errors, store_opened }
            }
        };
        on_event(&TweakEvent::Finished { outcome: outcome.clone() });
        outcomes.push(outcome);
    }
    RunReport { outcomes, restart, notices }
}

/// Áp dụng các mục `ids` (trùng ⇒ chạy một lần). `on_event` KHÔNG được gọi lại `apply`/`revert`
/// (khoá `RUN_LOCK` không vào lại được ⇒ treo).
pub fn apply(catalog: &[Tweak], ops: &dyn TweakOps, undo_path: &Path, ids: &[String], all_users: bool, on_event: &dyn Fn(&TweakEvent)) -> RunReport {
    run(catalog, ops, undo_path, ids, Mode::Apply { all_users }, on_event)
}

/// Hoàn tác các mục `ids` (trùng ⇒ chạy một lần), kể cả mục «không hỗ trợ» còn ảnh chụp. `on_event`
/// KHÔNG được gọi lại `apply`/`revert` (khoá `RUN_LOCK` không vào lại được ⇒ treo).
pub fn revert(catalog: &[Tweak], ops: &dyn TweakOps, undo_path: &Path, ids: &[String], on_event: &dyn Fn(&TweakEvent)) -> RunReport {
    run(catalog, ops, undo_path, ids, Mode::Revert, on_event)
}

/// Dòng nhật ký cho `log::CleanLog` của v0.1 (Task Tích hợp ghi ra file).
pub fn log_lines(action: &str, r: &RunReport) -> Vec<String> {
    let mut out: Vec<String> = r.notices.iter().map(|n| format!("TWEAK NOTICE {n}")).collect();
    for o in &r.outcomes {
        let status = serde_json::to_value(&o.status).ok().and_then(|v| v["status"].as_str().map(str::to_string)).unwrap_or_default();
        out.push(format!("TWEAK {action} {} {} {status}", o.id, if o.errors.is_empty() { "OK" } else { "ERRORS" }));
        out.extend(o.errors.iter().map(|e| format!("TWEAK ERROR {e}")));
        out.extend(o.store_opened.iter().map(|p| format!("TWEAK STORE {p}")));
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeOps;
    use crate::model::parse_catalog;
    use crate::ops::{PackageInfo, RegData};
    use std::sync::Mutex;

    const CAT: &str = r#"
[[tweak]]
id = "reg"
group = "privacy"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
needs_restart = "explorer"
ops = [
  { kind = "registry_set", path = 'HKCU\A', name = "x", type = "dword", value = 0, default = 1 },
  { kind = "registry_set", path = 'HKCU\A', name = "y", type = "dword", value = 0, default = "absent" },
]

[[tweak]]
id = "svc"
group = "privacy"
level = "recommended"
risk = "caution"
windows = { min_build = 19041 }
ops = [ { kind = "service_startup", service = "DiagTrack", start = "disabled", default = "auto" } ]

[[tweak]]
id = "task"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "scheduled_task_disable", task = '\T\One' } ]

[[tweak]]
id = "recall"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 26100 }
needs_restart = "reboot"
ops = [ { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\R', name = "v", type = "dword", value = 1, default = "absent" } ]

[[tweak]]
id = "app"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "A.App_1", store_product_id = "9P1J8S7CCWWT" } ]

[[tweak]]
id = "del"
group = "privacy"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "registry_delete", path = 'HKCU\D', name = "z" } ]

[[tweak]]
id = "gamebar"
group = "bloatware"
level = "recommended"
risk = "safe"
windows = { min_build = 19041 }
ops = [
  { kind = "appx_remove", package_family = "G.One_1", store_product_id = "9NZKPSTSNW4P" },
  { kind = "appx_remove", package_family = "G.Two_1", store_product_id = "9NZKPSTSNW4P" },
  { kind = "appx_remove", package_family = "G.Three_1", store_product_id = "9NZKPSTSNW4P" },
]
"#;

    struct Env {
        _dir: tempfile::TempDir,
        undo: std::path::PathBuf,
        cat: Vec<Tweak>,
    }

    fn env() -> Env {
        let d = tempfile::tempdir().unwrap();
        let undo = d.path().join("WinFreeUp").join("tweaks-undo.json");
        Env { _dir: d, undo, cat: parse_catalog(CAT).unwrap() }
    }

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn quiet(_: &TweakEvent) {}

    #[test]
    fn apply_then_revert_restores_snapshot_and_deletes_absent_values() {
        let e = env();
        let f = FakeOps::default().with_reg(r"HKCU\A", "x", RegData::Dword(7));
        let r = apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &quiet);
        assert_eq!(r.outcomes[0].status, TweakStatus::Applied);
        assert!(r.outcomes[0].errors.is_empty(), "{:?}", r.outcomes[0].errors);
        assert_eq!(r.restart, Restart::Explorer);
        let r = revert(&e.cat, &f, &e.undo, &ids(&["reg"]), &quiet);
        assert!(r.outcomes[0].errors.is_empty());
        assert_eq!(f.get(r"HKCU\A", "x"), Some(RegData::Dword(7)), "trả về ảnh chụp, không phải default = 1");
        assert_eq!(f.get(r"HKCU\A", "y"), None, "ảnh chụp absent ⇒ xoá value");
        let (u, _) = UndoStore::load(&e.undo);
        assert!(!u.has_any("reg"), "hoàn tác thành công ⇒ xoá ảnh chụp");
    }

    #[test]
    fn reapply_does_not_overwrite_original_snapshot() {
        let e = env();
        let f = FakeOps::default().with_reg(r"HKCU\A", "x", RegData::Dword(7));
        apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &quiet);
        f.reg_write(r"HKCU\A", "x", &RegData::Dword(9)).unwrap(); // người dùng đổi tay giữa hai lần
        apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &quiet);
        revert(&e.cat, &f, &e.undo, &ids(&["reg"]), &quiet);
        assert_eq!(f.get(r"HKCU\A", "x"), Some(RegData::Dword(7)));
    }

    #[test]
    fn value_of_unexpected_type_is_snapshotted_and_restored_as_is() {
        let e = env();
        // Công cụ khác từng ghi "0" dạng chuỗi thay vì DWORD.
        let f = FakeOps::default().with_reg(r"HKCU\A", "x", RegData::Sz("0".into()));
        let r = apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &quiet);
        assert_eq!(r.outcomes[0].status, TweakStatus::Applied);
        assert_eq!(f.get(r"HKCU\A", "x"), Some(RegData::Dword(0)));
        revert(&e.cat, &f, &e.undo, &ids(&["reg"]), &quiet);
        assert_eq!(f.get(r"HKCU\A", "x"), Some(RegData::Sz("0".into())));
    }

    #[test]
    fn revert_without_snapshot_uses_catalog_default() {
        let e = env();
        let f = FakeOps::default().with_reg(r"HKCU\A", "x", RegData::Dword(0)).with_reg(r"HKCU\A", "y", RegData::Dword(0));
        let r = revert(&e.cat, &f, &e.undo, &ids(&["reg"]), &quiet);
        assert!(r.outcomes[0].errors.is_empty());
        assert_eq!(f.get(r"HKCU\A", "x"), Some(RegData::Dword(1)));
        assert_eq!(f.get(r"HKCU\A", "y"), None);
    }

    #[test]
    fn corrupt_undo_file_becomes_bak_and_revert_uses_default() {
        let e = env();
        std::fs::create_dir_all(e.undo.parent().unwrap()).unwrap();
        std::fs::write(&e.undo, "garbage").unwrap();
        let f = FakeOps::default().with_reg(r"HKCU\A", "x", RegData::Dword(0));
        let r = revert(&e.cat, &f, &e.undo, &ids(&["reg"]), &quiet);
        assert!(r.notices[0].starts_with("undo_corrupt:"), "{:?}", r.notices);
        assert!(e.undo.with_extension("json.bak").exists());
        assert_eq!(f.get(r"HKCU\A", "x"), Some(RegData::Dword(1)));
    }

    #[test]
    fn one_failing_op_does_not_stop_the_run() {
        let e = env();
        let f = FakeOps::default().failing(r"reg_write:HKCU\A|x").with_task(r"\T\One", true);
        let r = apply(&e.cat, &f, &e.undo, &ids(&["reg", "task"]), false, &quiet);
        assert_eq!(r.outcomes[0].status, TweakStatus::Partial);
        assert_eq!(r.outcomes[0].errors, vec![r"reg#0: fake failure: reg_write:HKCU\A|x".to_string()]);
        assert_eq!(r.outcomes[1].status, TweakStatus::Applied);
        let (u, _) = UndoStore::load(&e.undo);
        assert!(u.get("reg", 0).is_some(), "thao tác hỏng vẫn có ảnh chụp đã lưu trước khi chạy");
    }

    #[test]
    fn op_is_not_run_when_snapshot_cannot_be_saved() {
        let e = env();
        // Cha của file ảnh chụp là một FILE ⇒ không tạo được thư mục ⇒ không lưu được.
        std::fs::write(e.undo.parent().unwrap(), "x").unwrap();
        let f = FakeOps::default();
        let r = apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &quiet);
        assert!(r.outcomes[0].errors.iter().all(|m| m.contains("undo_save")), "{:?}", r.outcomes[0].errors);
        assert!(f.calls().iter().all(|c| !c.starts_with("reg_write")), "không có ảnh chụp ⇒ không ghi registry");
        assert_eq!(r.outcomes[0].status, TweakStatus::NotApplied);
    }

    #[test]
    fn service_disabled_and_stopped_then_restored_without_starting() {
        let e = env();
        let f = FakeOps::default().with_service("DiagTrack", StartType::Auto);
        apply(&e.cat, &f, &e.undo, &ids(&["svc"]), false, &quiet);
        assert_eq!(f.calls(), vec!["set_service_start:DiagTrack:Disabled", "stop_service:DiagTrack"]);
        revert(&e.cat, &f, &e.undo, &ids(&["svc"]), &quiet);
        assert_eq!(f.service_start("DiagTrack").unwrap(), Some(StartType::Auto));
        assert!(!f.calls().iter().any(|c| c.starts_with("start_service")));
    }

    #[test]
    fn task_reenabled_only_if_it_was_enabled() {
        let e = env();
        let f = FakeOps::default().with_task(r"\T\One", false);
        apply(&e.cat, &f, &e.undo, &ids(&["task"]), false, &quiet);
        revert(&e.cat, &f, &e.undo, &ids(&["task"]), &quiet);
        assert_eq!(f.task_enabled(r"\T\One").unwrap(), Some(false), "ảnh chụp là tắt ⇒ không bật");
    }

    #[test]
    fn unsupported_or_unknown_ids_are_refused() {
        let e = env();
        let mut f = FakeOps::default();
        f.sys.build = 22631;
        let r = apply(&e.cat, &f, &e.undo, &ids(&["recall", "nope"]), false, &quiet);
        assert_eq!(r.outcomes[0].errors, vec!["recall: not_allowed".to_string()]);
        assert_eq!(r.outcomes[1].errors, vec!["nope: unknown_id".to_string()]);
        assert!(f.calls().is_empty());
        assert_eq!(r.restart, Restart::None);
    }

    #[test]
    fn app_removed_for_current_user_then_revert_opens_store_and_keeps_snapshot() {
        let e = env();
        let f = FakeOps::default().with_package("A.App_1");
        let r = apply(&e.cat, &f, &e.undo, &ids(&["app"]), false, &quiet);
        assert_eq!(f.calls(), vec!["remove_package:A.App_1!1:false"]);
        assert_eq!(r.outcomes[0].status, TweakStatus::Applied);
        let r = revert(&e.cat, &f, &e.undo, &ids(&["app"]), &quiet);
        assert_eq!(r.outcomes[0].store_opened, vec!["9P1J8S7CCWWT"]);
        assert!(f.calls().contains(&"open_uri:ms-windows-store://pdp/?ProductId=9P1J8S7CCWWT".to_string()));
        let (u, _) = UndoStore::load(&e.undo);
        assert!(u.has_any("app"), "chưa cài lại ⇒ giữ ảnh chụp để còn nút «Cài lại từ Store»");
    }

    #[test]
    fn all_users_mode_also_deprovisions() {
        let e = env();
        let f = FakeOps::default().with_package("A.App_1");
        apply(&e.cat, &f, &e.undo, &ids(&["app"]), true, &quiet);
        assert_eq!(f.calls(), vec!["remove_package:A.App_1!1:true", "deprovision:A.App_1"]);
    }

    #[test]
    fn framework_or_system_packages_are_never_removed_even_if_catalog_says_so() {
        for all_users in [false, true] {
            for (is_framework, non_removable) in [(false, true), (true, false)] {
                let e = env();
                let f = FakeOps::default();
                f.packages.lock().unwrap().push(PackageInfo { full_name: "A.App_1!1".into(), family: "A.App_1".into(), is_framework, non_removable });
                let r = apply(&e.cat, &f, &e.undo, &ids(&["app"]), all_users, &quiet);
                assert_eq!(r.outcomes[0].errors, vec!["app#0: blocked:A.App_1".to_string()]);
                assert!(f.calls().is_empty(), "all_users={all_users}: {:?} — gói bị chặn thì cũng không deprovision", f.calls());
            }
        }
    }

    #[test]
    fn all_users_snapshots_then_deprovisions_even_when_current_user_lacks_the_package() {
        let e = env();
        let f = FakeOps::default().with_package("G.One_1").with_provisioned("G.Two_1").with_provisioned("G.Three_1");
        let r = apply(&e.cat, &f, &e.undo, &ids(&["gamebar"]), true, &quiet);
        assert!(r.outcomes[0].errors.is_empty(), "{:?}", r.outcomes[0].errors);
        assert!(f.calls().contains(&"deprovision:G.Two_1".to_string()));
        let (u, _) = UndoStore::load(&e.undo);
        assert!((0..3).all(|i| u.get("gamebar", i).is_some()), "deprovision cũng phải có ảnh chụp để còn nút Cài lại");
    }

    #[test]
    fn deprovision_not_run_when_snapshot_cannot_be_saved() {
        let e = env();
        std::fs::write(e.undo.parent().unwrap(), "x").unwrap();
        let f = FakeOps::default().with_package("G.One_1");
        apply(&e.cat, &f, &e.undo, &ids(&["gamebar"]), true, &quiet);
        assert!(f.calls().is_empty(), "{:?}", f.calls());
    }

    /// FakeOps nhưng `packages()` trả MỌI gói, không lọc theo family — mô phỏng lớp thật lọc lỏng.
    struct LoosePackages(FakeOps);

    impl TweakOps for LoosePackages {
        fn reg_read(&self, p: &str, n: &str) -> Result<Option<RegData>, String> {
            self.0.reg_read(p, n)
        }
        fn reg_write(&self, p: &str, n: &str, d: &RegData) -> Result<(), String> {
            self.0.reg_write(p, n, d)
        }
        fn reg_delete(&self, p: &str, n: &str) -> Result<(), String> {
            self.0.reg_delete(p, n)
        }
        fn service_start(&self, n: &str) -> Result<Option<StartType>, String> {
            self.0.service_start(n)
        }
        fn set_service_start(&self, n: &str, s: StartType) -> Result<(), String> {
            self.0.set_service_start(n, s)
        }
        fn stop_service(&self, n: &str) -> Result<(), String> {
            self.0.stop_service(n)
        }
        fn task_enabled(&self, p: &str) -> Result<Option<bool>, String> {
            self.0.task_enabled(p)
        }
        fn set_task_enabled(&self, p: &str, e: bool) -> Result<(), String> {
            self.0.set_task_enabled(p, e)
        }
        fn packages(&self, _family: &str) -> Result<Vec<PackageInfo>, String> {
            Ok(self.0.packages.lock().unwrap().clone())
        }
        fn remove_package(&self, n: &str, a: bool) -> Result<(), String> {
            self.0.remove_package(n, a)
        }
        fn deprovision(&self, f: &str) -> Result<bool, String> {
            self.0.deprovision(f)
        }
        fn open_uri(&self, u: &str) -> Result<(), String> {
            self.0.open_uri(u)
        }
        fn system_info(&self) -> Result<SystemInfo, String> {
            self.0.system_info()
        }
        fn restart_explorer(&self) -> Result<(), String> {
            self.0.restart_explorer()
        }
    }

    #[test]
    fn deprovision_that_removed_nothing_drops_its_fresh_snapshot() {
        let e = env();
        // #0: gỡ được G.One ⇒ giữ; #1: deprovision hỏng ⇒ bỏ; #2: deprovision gỡ thật ⇒ giữ.
        let f = FakeOps::default().with_package("G.One_1").with_provisioned("G.Three_1").failing("deprovision:G.Two_1");
        let r = apply(&e.cat, &f, &e.undo, &ids(&["gamebar"]), true, &quiet);
        assert_eq!(r.outcomes[0].errors, vec!["gamebar#1: fake failure: deprovision:G.Two_1".to_string()]);
        let (u, _) = UndoStore::load(&e.undo);
        assert_eq!((u.get("gamebar", 0).is_some(), u.get("gamebar", 1).is_some(), u.get("gamebar", 2).is_some()), (true, false, true));
    }

    #[test]
    fn snapshot_not_stuck_when_all_users_run_changed_nothing() {
        for failing_deprovision in [false, true] {
            let e = env();
            let mut f = FakeOps::default().with_package("A.App_1").failing("remove_package:A.App_1!1:true");
            if failing_deprovision {
                f = f.failing("deprovision:A.App_1");
            }
            let r = apply(&e.cat, &f, &e.undo, &ids(&["app"]), true, &quiet);
            assert_eq!(r.restart, Restart::None);
            let (u, _) = UndoStore::load(&e.undo);
            assert!(!u.has_any("app"), "deprovision={failing_deprovision}: không gỡ được gì ⇒ không để lại ảnh chụp");
            // Người dùng tự gỡ app sau đó ⇒ hoàn tác không được mở Store cho thứ WinFreeUp chưa từng gỡ.
            f.packages.lock().unwrap().clear();
            revert(&e.cat, &f, &e.undo, &ids(&["app"]), &quiet);
            assert!(!f.calls().iter().any(|c| c.starts_with("open_uri:")), "{:?}", f.calls());
        }
    }

    #[test]
    fn read_and_revert_ignore_packages_of_another_family() {
        let e = env();
        let f = LoosePackages(FakeOps::default().with_package("A.App_1").with_package("B.Other_1"));
        apply(&e.cat, &f, &e.undo, &ids(&["app"]), false, &quiet);
        let read = read_all(&e.cat, &f, &e.undo).unwrap();
        assert_eq!(read.tweaks.iter().find(|t| t.id == "app").unwrap().status, TweakStatus::Applied, "B.Other không phải gói của mục");
        let r = revert(&e.cat, &f, &e.undo, &ids(&["app"]), &quiet);
        assert_eq!(r.outcomes[0].store_opened, vec!["9P1J8S7CCWWT"]);
    }

    #[test]
    fn registry_delete_with_foreign_snapshot_kind_is_an_error_and_keeps_it() {
        let e = env();
        let (mut u, _) = UndoStore::load(&e.undo);
        u.record_if_absent("del", 0, Snapshot::Service { start: StartType::Auto });
        u.save().unwrap();
        let f = FakeOps::default();
        let r = revert(&e.cat, &f, &e.undo, &ids(&["del"]), &quiet);
        assert_eq!(r.outcomes[0].errors, vec!["del#0: snapshot_mismatch".to_string()]);
        let (u, _) = UndoStore::load(&e.undo);
        assert!(u.get("del", 0).is_some(), "ảnh chụp lạ ⇒ giữ nguyên để người sửa xem");
    }

    #[test]
    fn packages_of_another_family_are_never_removed() {
        let e = env();
        let f = LoosePackages(FakeOps::default().with_package("A.App_1").with_package("B.Other_1"));
        apply(&e.cat, &f, &e.undo, &ids(&["app"]), false, &quiet);
        assert_eq!(f.0.calls(), vec!["remove_package:A.App_1!1:false"]);
    }

    #[test]
    fn revert_without_snapshot_leaves_values_not_at_target_alone() {
        let e = env();
        // x đang ở đích (0) ⇒ trả default; y người dùng tự đặt 5 ⇒ không đụng.
        let f = FakeOps::default()
            .with_reg(r"HKCU\A", "x", RegData::Dword(0))
            .with_reg(r"HKCU\A", "y", RegData::Dword(5))
            .with_service("DiagTrack", StartType::Manual)
            .with_task(r"\T\One", true);
        let r = revert(&e.cat, &f, &e.undo, &ids(&["reg", "svc", "task"]), &quiet);
        assert!(r.outcomes.iter().all(|o| o.errors.is_empty()), "{:?}", r.outcomes);
        assert_eq!(f.get(r"HKCU\A", "x"), Some(RegData::Dword(1)));
        assert_eq!(f.get(r"HKCU\A", "y"), Some(RegData::Dword(5)));
        assert_eq!(f.calls(), vec![r"reg_write:HKCU\A|x"], "dịch vụ/tác vụ không ở đích ⇒ không đụng");
    }

    #[test]
    fn duplicate_ids_run_once() {
        let e = env();
        let f = FakeOps::default().with_reg(r"HKCU\A", "x", RegData::Dword(7));
        apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &quiet);
        let seen = Mutex::new(Vec::new());
        let r = revert(&e.cat, &f, &e.undo, &ids(&["reg", "task", "reg"]), &|ev| {
            if let TweakEvent::Started { id, index, total } = ev {
                seen.lock().unwrap().push(format!("{id} {index}/{total}"));
            }
        });
        assert_eq!(r.outcomes.len(), 2);
        assert_eq!(*seen.lock().unwrap(), vec!["reg 0/2", "task 1/2"]);
        assert_eq!(f.get(r"HKCU\A", "x"), Some(RegData::Dword(7)));
    }

    #[test]
    fn revert_that_only_clears_snapshots_needs_no_restart() {
        let e = env();
        let f = FakeOps::default().with_reg(r"HKCU\A", "x", RegData::Dword(7));
        apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &quiet);
        // Người dùng tự trả tay về như cũ.
        f.reg_write(r"HKCU\A", "x", &RegData::Dword(7)).unwrap();
        f.reg_delete(r"HKCU\A", "y").unwrap();
        let before = f.calls().len();
        let r = revert(&e.cat, &f, &e.undo, &ids(&["reg"]), &quiet);
        assert!(r.outcomes[0].errors.is_empty(), "{:?}", r.outcomes[0].errors);
        assert_eq!(r.restart, Restart::None, "máy không đổi gì ⇒ không nhắc khởi động lại Explorer");
        assert_eq!(f.calls().len(), before, "giá trị đã đúng ⇒ không ghi");
        let (u, _) = UndoStore::load(&e.undo);
        assert!(!u.has_any("reg"), "vẫn dọn ảnh chụp");
    }

    #[test]
    fn runs_hold_the_global_lock() {
        let e = env();
        let f = FakeOps::default();
        apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &|_| {
            assert!(RUN_LOCK.try_lock().is_err(), "đang chạy thì khoá phải bị giữ");
        });
    }

    #[test]
    fn registry_delete_of_absent_value_is_still_snapshotted() {
        let e = env();
        let f = FakeOps::default();
        apply(&e.cat, &f, &e.undo, &ids(&["del"]), false, &quiet);
        let (u, _) = UndoStore::load(&e.undo);
        assert_eq!(u.get("del", 0), Some(&Snapshot::Registry { data: None }));
        assert!(f.calls().is_empty());
    }

    #[test]
    fn invalid_ids_are_not_echoed() {
        let e = env();
        let f = FakeOps::default();
        let r = apply(&e.cat, &f, &e.undo, &ids(&["Bad id\nTWEAK APPLY x OK"]), false, &quiet);
        assert_eq!(r.outcomes[0].id, "<invalid>");
        assert_eq!(r.outcomes[0].errors, vec!["<invalid>: unknown_id".to_string()]);
        assert!(log_lines("APPLY", &r).iter().all(|l| !l.contains('\n')));
    }

    #[test]
    fn events_bracket_each_tweak() {
        let e = env();
        let f = FakeOps::default();
        let seen = Mutex::new(Vec::new());
        apply(&e.cat, &f, &e.undo, &ids(&["reg", "task"]), false, &|ev| {
            seen.lock().unwrap().push(match ev {
                TweakEvent::Started { id, index, total } => format!("start {id} {index}/{total}"),
                TweakEvent::Finished { outcome } => format!("end {}", outcome.id),
            })
        });
        assert_eq!(*seen.lock().unwrap(), vec!["start reg 0/2", "end reg", "start task 1/2", "end task"]);
    }

    #[test]
    fn read_all_marks_undo_and_serializes_flat_status() {
        let e = env();
        let f = FakeOps::default().with_package("A.App_1");
        apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &quiet);
        let r = read_all(&e.cat, &f, &e.undo).unwrap();
        let reg = r.tweaks.iter().find(|t| t.id == "reg").unwrap();
        assert!(reg.has_undo);
        let v = serde_json::to_value(reg).unwrap();
        assert_eq!(v["status"], "applied");
        assert_eq!(v["needs_restart"], "explorer");
        let recall = serde_json::to_value(r.tweaks.iter().find(|t| t.id == "recall").unwrap()).unwrap();
        assert_eq!(recall["status"], "not_applied", "build 26100 của FakeOps ⇒ hỗ trợ");
    }

    #[test]
    fn log_lines_are_greppable() {
        let e = env();
        let f = FakeOps::default().failing(r"reg_write:HKCU\A|x");
        let r = apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &quiet);
        let lines = log_lines("APPLY", &r);
        assert_eq!(lines[0], "TWEAK APPLY reg ERRORS partial");
        assert_eq!(lines[1], r"TWEAK ERROR reg#0: fake failure: reg_write:HKCU\A|x");
    }

    #[test]
    fn unsupported_but_snapshotted_can_be_reverted_but_not_reapplied() {
        let e = env();
        let mut f = FakeOps::default();
        let r = apply(&e.cat, &f, &e.undo, &ids(&["recall"]), false, &quiet);
        assert_eq!(r.outcomes[0].status, TweakStatus::Applied);
        // Máy đổi build/edition (vd hạ cấp) — mục có ảnh chụp nên trạng thái thật vẫn «đã áp dụng».
        f.sys.build = 22631;
        let read = read_all(&e.cat, &f, &e.undo).unwrap();
        let v = serde_json::to_value(read.tweaks.iter().find(|t| t.id == "recall").unwrap()).unwrap();
        assert_eq!((v["status"].as_str(), v["reason"].as_str(), v["has_undo"].as_bool()), (Some("unsupported"), Some("build_min:26100"), Some(true)));
        f.reg_delete(r"HKLM\SOFTWARE\Policies\R", "v").unwrap();
        let before = f.calls().len();
        let r = apply(&e.cat, &f, &e.undo, &ids(&["recall"]), false, &quiet);
        assert_eq!(r.outcomes[0].errors, vec!["recall: not_allowed".to_string()]);
        assert_eq!(f.calls().len(), before, "sai build ⇒ không áp dụng lại");
        assert_eq!(r.restart, Restart::None);
        f.reg_write(r"HKLM\SOFTWARE\Policies\R", "v", &RegData::Dword(1)).unwrap();
        let r = revert(&e.cat, &f, &e.undo, &ids(&["recall"]), &quiet);
        assert!(r.outcomes[0].errors.is_empty(), "{:?}", r.outcomes[0].errors);
        assert_eq!(f.get(r"HKLM\SOFTWARE\Policies\R", "v"), None, "hoàn tác vẫn được");
        let (u, _) = UndoStore::load(&e.undo);
        assert!(!u.has_any("recall"));
    }

    #[test]
    fn store_page_opened_once_per_product_id_per_run() {
        let e = env();
        let f = FakeOps::default().with_package("G.One_1").with_package("G.Two_1").with_package("G.Three_1");
        let r = apply(&e.cat, &f, &e.undo, &ids(&["gamebar"]), false, &quiet);
        assert_eq!(r.outcomes[0].status, TweakStatus::Applied);
        let r = revert(&e.cat, &f, &e.undo, &ids(&["gamebar"]), &quiet);
        assert!(r.outcomes[0].errors.is_empty(), "{:?}", r.outcomes[0].errors);
        let opened = f.calls().iter().filter(|c| c.starts_with("open_uri:")).count();
        assert_eq!(opened, 1, "ba gói cùng ProductId ⇒ chỉ mở trang Store một lần");
        assert_eq!(r.outcomes[0].store_opened, vec!["9NZKPSTSNW4P"]);
        let (u, _) = UndoStore::load(&e.undo);
        assert!((0..3).all(|i| u.get("gamebar", i).is_some()), "chưa cài lại ⇒ giữ ảnh chụp cả ba gói");
        // Lượt sau vẫn mở lại được.
        revert(&e.cat, &f, &e.undo, &ids(&["gamebar"]), &quiet);
        assert_eq!(f.calls().iter().filter(|c| c.starts_with("open_uri:")).count(), 2);
    }
}
