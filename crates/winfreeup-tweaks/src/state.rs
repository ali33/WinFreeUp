//! Trạng thái một mục — luôn tính từ máy thật (spec mục 3.2).
use serde::Serialize;

use crate::model::{to_reg_data, Group, Op, Tweak};
use crate::ops::{SystemInfo, TweakOps};
use crate::undo::UndoStore;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TweakStatus {
    Applied,
    NotApplied,
    Partial,
    /// `reason`: `build_min:<n>`, `build_max:<n>`, `edition`, `missing` (dịch vụ/tác vụ không có trên máy).
    Unsupported { reason: String },
    /// Máy do tổ chức quản lý và chính sách đã đặt giá trị khác — không cho áp dụng.
    Managed,
    /// Gói app không có trên máy và chưa từng bị WinFreeUp gỡ — giao diện ẩn mục này.
    NotPresent,
}

impl TweakStatus {
    /// Được phép áp dụng/hoàn tác.
    pub fn actionable(&self) -> bool {
        matches!(self, TweakStatus::Applied | TweakStatus::NotApplied | TweakStatus::Partial)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpState {
    Match,
    Differ,
    /// Thành phần không có trên máy (dịch vụ, tác vụ, gói chưa từng gỡ) — bỏ qua khi tính trạng thái.
    Missing,
}

/// `Err(reason)` nếu build/edition không hợp.
pub fn supported(t: &Tweak, sys: &SystemInfo) -> Result<(), String> {
    let w = &t.windows;
    if sys.build < w.min_build {
        return Err(format!("build_min:{}", w.min_build));
    }
    if w.max_build != 0 && sys.build > w.max_build {
        return Err(format!("build_max:{}", w.max_build));
    }
    if !w.editions.iter().any(|e| e == "*" || e.eq_ignore_ascii_case(&sys.edition)) {
        return Err("edition".into());
    }
    Ok(())
}

/// Trạng thái một thao tác so với đích. `had_snapshot` chỉ dùng cho `appx_remove`.
pub fn op_state(ops: &dyn TweakOps, op: &Op, had_snapshot: bool) -> Result<OpState, String> {
    Ok(match op {
        Op::RegistrySet { path, name, value_type, value, .. } => {
            let target = to_reg_data(*value_type, value)?;
            if ops.reg_read(path, name)? == Some(target) {
                OpState::Match
            } else {
                OpState::Differ
            }
        }
        Op::RegistryDelete { path, name } => {
            if ops.reg_read(path, name)?.is_none() {
                OpState::Match
            } else {
                OpState::Differ
            }
        }
        Op::ServiceStartup { service, start, .. } => match ops.service_start(service)? {
            None => OpState::Missing,
            Some(s) if s == *start => OpState::Match,
            Some(_) => OpState::Differ,
        },
        Op::ScheduledTaskDisable { task } => match ops.task_enabled(task)? {
            None => OpState::Missing,
            Some(false) => OpState::Match,
            Some(true) => OpState::Differ,
        },
        Op::AppxRemove { package_family, .. } => {
            if !ops.packages(package_family)?.is_empty() {
                OpState::Differ
            } else if had_snapshot {
                OpState::Match
            } else {
                OpState::Missing
            }
        }
    })
}

fn is_policy_path(path: &str) -> bool {
    path.to_ascii_lowercase().contains(r"\policies\")
}

/// Trạng thái cả mục và các lỗi đọc (nguyên văn). Lỗi đọc một thao tác ⇒ tính là `Differ`.
pub fn tweak_status(t: &Tweak, sys: &SystemInfo, ops: &dyn TweakOps, undo: &UndoStore) -> (TweakStatus, Vec<String>) {
    if let Err(reason) = supported(t, sys) {
        // App không có trên máy thì ẩn luôn, kể cả khi sai build — «không hỗ trợ» chỉ hiện cho thứ đang có.
        let absent = t.group == Group::Bloatware
            && t.ops.iter().enumerate().all(|(i, op)| matches!(op_state(ops, op, undo.get(&t.id, i).is_some()), Ok(OpState::Missing)));
        let status = if absent { TweakStatus::NotPresent } else { TweakStatus::Unsupported { reason } };
        return (status, vec![]);
    }
    let mut errors = Vec::new();
    let mut states = Vec::with_capacity(t.ops.len());
    for (i, op) in t.ops.iter().enumerate() {
        let had = undo.get(&t.id, i).is_some();
        if sys.managed && !had {
            if let Op::RegistrySet { path, name, value_type, value, .. } = op {
                if is_policy_path(path) {
                    match (ops.reg_read(path, name), to_reg_data(*value_type, value)) {
                        (Ok(Some(cur)), Ok(target)) if cur != target => return (TweakStatus::Managed, vec![]),
                        _ => {}
                    }
                }
            }
        }
        match op_state(ops, op, had) {
            Ok(s) => states.push(s),
            Err(e) => {
                errors.push(format!("{}#{i}: {e}", t.id));
                states.push(OpState::Differ);
            }
        }
    }
    let present: Vec<OpState> = states.into_iter().filter(|s| *s != OpState::Missing).collect();
    let status = if present.is_empty() {
        if t.group == Group::Bloatware {
            TweakStatus::NotPresent
        } else {
            TweakStatus::Unsupported { reason: "missing".into() }
        }
    } else if present.iter().all(|s| *s == OpState::Match) {
        TweakStatus::Applied
    } else if present.iter().all(|s| *s == OpState::Differ) {
        TweakStatus::NotApplied
    } else {
        TweakStatus::Partial
    };
    (status, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeOps;
    use crate::model::{parse_catalog, StartType};
    use crate::ops::RegData;
    use crate::undo::Snapshot;

    const CAT: &str = r#"
[[tweak]]
id = "two_values"
group = "privacy"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [
  { kind = "registry_set", path = 'HKCU\A', name = "x", type = "dword", value = 0, default = 1 },
  { kind = "registry_set", path = 'HKCU\A', name = "y", type = "dword", value = 0, default = "absent" },
]

[[tweak]]
id = "policy"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 19041, editions = ["Home", "Pro"] }
ops = [ { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\P', name = "v", type = "dword", value = 1, default = "absent" } ]

[[tweak]]
id = "recall"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 26100 }
ops = [ { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\R', name = "v", type = "dword", value = 1, default = "absent" } ]

[[tweak]]
id = "tasks"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "scheduled_task_disable", task = '\T\One' }, { kind = "scheduled_task_disable", task = '\T\Two' } ]

[[tweak]]
id = "svc"
group = "privacy"
level = "recommended"
risk = "caution"
windows = { min_build = 19041 }
ops = [ { kind = "service_startup", service = "DiagTrack", start = "disabled", default = "auto" } ]

[[tweak]]
id = "app"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "A.App_1", store_product_id = "9P1J8S7CCWWT" } ]
"#;

    fn cat() -> Vec<Tweak> {
        parse_catalog(CAT).unwrap()
    }
    fn tw(id: &str) -> Tweak {
        cat().into_iter().find(|t| t.id == id).unwrap()
    }
    fn empty_undo() -> (tempfile::TempDir, UndoStore) {
        let d = tempfile::tempdir().unwrap();
        let (u, _) = UndoStore::load(&d.path().join("u.json"));
        (d, u)
    }

    #[test]
    fn applied_not_applied_partial() {
        let (_d, u) = empty_undo();
        let t = tw("two_values");
        let f = FakeOps::default();
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::NotApplied);
        let f = FakeOps::default().with_reg(r"HKCU\A", "x", RegData::Dword(0));
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::Partial);
        let f = f.with_reg(r"HKCU\A", "y", RegData::Dword(0));
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::Applied);
    }

    #[test]
    fn wrong_build_or_edition_is_unsupported() {
        let (_d, u) = empty_undo();
        let mut f = FakeOps::default();
        f.sys.build = 22631;
        assert_eq!(tweak_status(&tw("recall"), &f.sys, &f, &u).0, TweakStatus::Unsupported { reason: "build_min:26100".into() });
        f.sys.edition = "Enterprise".into();
        assert_eq!(tweak_status(&tw("policy"), &f.sys, &f, &u).0, TweakStatus::Unsupported { reason: "edition".into() });
    }

    #[test]
    fn managed_only_when_org_policy_already_differs() {
        let (_d, mut u) = empty_undo();
        let t = tw("policy");
        let mut f = FakeOps::default().with_reg(r"HKLM\SOFTWARE\Policies\P", "v", RegData::Dword(3));
        // Máy cá nhân: giá trị lạ dưới Policies không khoá.
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::NotApplied);
        f.sys.managed = true;
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::Managed);
        // Máy quản lý nhưng value chưa có ⇒ vẫn cho áp dụng.
        let mut g = FakeOps::default();
        g.sys.managed = true;
        assert_eq!(tweak_status(&t, &g.sys, &g, &u).0, TweakStatus::NotApplied);
        // Chính WinFreeUp đã ghi (có ảnh chụp) ⇒ không coi là tổ chức quản lý.
        u.record_if_absent("policy", 0, Snapshot::Registry { data: None });
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::NotApplied);
    }

    #[test]
    fn missing_tasks_are_ignored_and_all_missing_is_unsupported() {
        let (_d, u) = empty_undo();
        let t = tw("tasks");
        let f = FakeOps::default().with_task(r"\T\One", false);
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::Applied, "tác vụ Two không có trên máy ⇒ bỏ qua");
        let f = FakeOps::default();
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::Unsupported { reason: "missing".into() });
        let f = FakeOps::default().with_service("DiagTrack", StartType::Auto);
        assert_eq!(tweak_status(&tw("svc"), &f.sys, &f, &u).0, TweakStatus::NotApplied);
    }

    #[test]
    fn app_hidden_unless_installed_or_removed_by_us() {
        let (_d, mut u) = empty_undo();
        let t = tw("app");
        let f = FakeOps::default();
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::NotPresent);
        let g = FakeOps::default().with_package("A.App_1");
        assert_eq!(tweak_status(&t, &g.sys, &g, &u).0, TweakStatus::NotApplied);
        u.record_if_absent("app", 0, Snapshot::Appx { family: "A.App_1".into() });
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::Applied, "đã gỡ bởi WinFreeUp ⇒ hiện «Đã gỡ»");
    }

    #[test]
    fn absent_app_on_unsupported_build_is_hidden_not_unsupported() {
        let (_d, u) = empty_undo();
        let mut t = tw("app");
        t.windows.min_build = 22000;
        let mut f = FakeOps::default();
        f.sys.build = 19045;
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::NotPresent);
        let mut g = FakeOps::default().with_package("A.App_1");
        g.sys.build = 19045;
        assert_eq!(tweak_status(&t, &g.sys, &g, &u).0, TweakStatus::Unsupported { reason: "build_min:22000".into() });
    }

    #[test]
    fn read_error_is_reported_verbatim_and_counts_as_differ() {
        let (_d, u) = empty_undo();
        let t = tw("two_values");
        let f = FakeOps::default().with_reg(r"HKCU\A", "y", RegData::Dword(0)).failing(r"reg_read:HKCU\A|x");
        let (s, errs) = tweak_status(&t, &f.sys, &f, &u);
        assert_eq!(s, TweakStatus::Partial);
        assert_eq!(errs, vec![r"two_values#0: fake failure: reg_read:HKCU\A|x".to_string()]);
    }

    #[test]
    fn status_json_shape() {
        assert_eq!(serde_json::to_string(&TweakStatus::NotApplied).unwrap(), r#"{"status":"not_applied"}"#);
        assert_eq!(
            serde_json::to_string(&TweakStatus::Unsupported { reason: "edition".into() }).unwrap(),
            r#"{"status":"unsupported","reason":"edition"}"#
        );
    }
}
