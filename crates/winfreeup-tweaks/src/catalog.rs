//! Danh mục nhúng vào exe và luật kiểm danh mục (spec mục 7 «Danh mục»). Danh mục sai ⇒ test đỏ ⇒ CI đỏ.
use std::collections::HashSet;

use crate::blocklist::{is_blocked_package, is_forbidden_reg_path, is_forbidden_service};
use crate::model::{default_data, parse_catalog, to_reg_data, Group, Op, RegValue, Tweak, ABSENT};

pub const CATALOG_TOML: &str = include_str!("../catalog.toml");

const EDITIONS: [&str; 5] = ["*", "Home", "Pro", "Enterprise", "Education"];

/// Danh mục đã nhúng. Chỉ hỏng khi danh mục sai — test `builtin_catalog_is_valid` chặn trước khi build.
pub fn builtin() -> Result<Vec<Tweak>, String> {
    parse_catalog(CATALOG_TOML)
}

/// Khoá chuỗi hiển thị của một mục: `tweaks.item.<id>.name` và `tweaks.item.<id>.desc`.
pub fn name_key(id: &str) -> String {
    format!("tweaks.item.{id}.name")
}
pub fn desc_key(id: &str) -> String {
    format!("tweaks.item.{id}.desc")
}

/// ProductId Store: 12 ký tự (app Store, vd `9P1J8S7CCWWT`) hoặc 14 ký tự bắt đầu `XP` (app Win32 trên Store).
pub fn is_product_id(s: &str) -> bool {
    let ok = s.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    ok && (s.len() == 12 || (s.len() == 14 && s.starts_with("XP")))
}

/// Mọi vi phạm, mỗi dòng một lỗi (rỗng = hợp lệ). `has_key` tra chuỗi trong file tiếng Việt.
pub fn validate(catalog: &[Tweak], has_key: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut errs = Vec::new();
    let mut seen = HashSet::new();
    for t in catalog {
        let id = t.id.as_str();
        if !seen.insert(id) {
            errs.push(format!("{id}: duplicate id"));
        }
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            errs.push(format!("{id}: id must be [a-z0-9_]+"));
        }
        for k in [name_key(id), desc_key(id)] {
            if !has_key(&k) {
                errs.push(format!("{id}: missing string {k}"));
            }
        }
        let w = &t.windows;
        if w.max_build != 0 && w.min_build > w.max_build {
            errs.push(format!("{id}: min_build {} > max_build {}", w.min_build, w.max_build));
        }
        if w.editions.is_empty() || w.editions.iter().any(|e| !EDITIONS.contains(&e.as_str())) {
            errs.push(format!("{id}: bad editions {:?}", w.editions));
        }
        if t.ops.is_empty() {
            errs.push(format!("{id}: no ops"));
        }
        for (i, op) in t.ops.iter().enumerate() {
            let is_appx = matches!(op, Op::AppxRemove { .. });
            if is_appx != (t.group == Group::Bloatware) {
                errs.push(format!("{id}#{i}: bloatware tweaks hold only appx_remove, privacy tweaks never do"));
            }
            match op {
                Op::RegistrySet { path, value_type, value, default, .. } => {
                    check_path(id, i, path, &mut errs);
                    // `"absent"` chỉ có nghĩa ở `default` — ở `value` (kể cả kiểu sz) là lỗi viết danh mục.
                    if matches!(value, RegValue::Str(s) if s == ABSENT) {
                        errs.push(format!("{id}#{i}: value must not be \"{ABSENT}\" (only default may be)"));
                    } else if let Err(e) = to_reg_data(*value_type, value) {
                        errs.push(format!("{id}#{i}: value: {e}"));
                    }
                    if let Err(e) = default_data(*value_type, default) {
                        errs.push(format!("{id}#{i}: default: {e}"));
                    }
                }
                Op::RegistryDelete { path, .. } => check_path(id, i, path, &mut errs),
                Op::ServiceStartup { service, .. } => {
                    if is_forbidden_service(service) {
                        errs.push(format!("{id}#{i}: service {service} is off-limits"));
                    }
                }
                Op::ScheduledTaskDisable { task } => {
                    if !task.starts_with('\\') {
                        errs.push(format!("{id}#{i}: task path must start with \\"));
                    }
                }
                Op::AppxRemove { package_family, store_product_id } => {
                    if !package_family.contains('_') {
                        errs.push(format!("{id}#{i}: package_family must be Name_PublisherId"));
                    }
                    if is_blocked_package(package_family) {
                        errs.push(format!("{id}#{i}: {package_family} is on the do-not-remove list"));
                    }
                    if !is_product_id(store_product_id) {
                        errs.push(format!("{id}#{i}: bad store_product_id {store_product_id:?}"));
                    }
                }
            }
        }
    }
    errs
}

fn check_path(id: &str, i: usize, path: &str, errs: &mut Vec<String>) {
    let upper = path.to_ascii_uppercase();
    if !(upper.starts_with("HKCU\\") || upper.starts_with("HKLM\\")) {
        errs.push(format!("{id}#{i}: path must start with HKCU\\ or HKLM\\"));
    }
    if is_forbidden_reg_path(path) {
        errs.push(format!("{id}#{i}: path {path} is off-limits"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Level;

    /// Chuỗi tên/mô tả của từng mục — Task Tích hợp đổi đường dẫn sang `src/i18n/vi.json`.
    const VI_JSON: &str = include_str!("../../../src/features/tinh-chinh/catalog.vi.json");

    fn vi_keys() -> HashSet<String> {
        let v: serde_json::Map<String, serde_json::Value> = serde_json::from_str(VI_JSON).unwrap();
        v.into_iter().filter(|(_, s)| s.as_str().is_some_and(|s| !s.trim().is_empty())).map(|(k, _)| k).collect()
    }

    #[test]
    fn builtin_catalog_is_valid() {
        let c = builtin().unwrap();
        let keys = vi_keys();
        let errs = validate(&c, &|k| keys.contains(k));
        assert!(errs.is_empty(), "catalog errors:\n{}", errs.join("\n"));
    }

    #[test]
    fn builtin_catalog_covers_all_three_levels_and_groups() {
        let c = builtin().unwrap();
        for l in [Level::Basic, Level::Recommended, Level::Aggressive] {
            assert!(c.iter().any(|t| t.level == l && t.group == Group::Privacy), "privacy {l:?}");
            assert!(c.iter().any(|t| t.level == l && t.group == Group::Bloatware), "bloatware {l:?}");
        }
    }

    fn one(toml: &str) -> Vec<String> {
        let c = parse_catalog(toml).unwrap();
        validate(&c, &|_| true)
    }

    const HEAD: &str = "[[tweak]]\nid = \"x\"\ngroup = \"bloatware\"\nlevel = \"basic\"\nrisk = \"safe\"\nwindows = { min_build = 19041 }\n";

    #[test]
    fn blocked_package_in_catalog_is_rejected() {
        let e = one(&format!("{HEAD}ops = [{{ kind = \"appx_remove\", package_family = \"Microsoft.WindowsStore_8wekyb3d8bbwe\", store_product_id = \"9WZDNCRFJBMP\" }}]"));
        assert!(e.iter().any(|m| m.contains("do-not-remove")), "{e:?}");
        let e = one(&format!("{HEAD}ops = [{{ kind = \"appx_remove\", package_family = \"Microsoft.VCLibs.140.00_8wekyb3d8bbwe\", store_product_id = \"9WZDNCRFJBMP\" }}]"));
        assert!(e.iter().any(|m| m.contains("do-not-remove")), "{e:?}");
        let e = one(&format!("{HEAD}ops = [{{ kind = \"appx_remove\", package_family = \"MSTeams_8wekyb3d8bbwe\", store_product_id = \"XP8BT8DW290MPQ\" }}]"));
        assert!(e.iter().any(|m| m.contains("do-not-remove")), "{e:?}");
    }

    #[test]
    fn duplicate_ids_bad_builds_and_missing_strings_are_rejected() {
        let t = format!("{HEAD}ops = [{{ kind = \"appx_remove\", package_family = \"A.B_1\", store_product_id = \"9P1J8S7CCWWT\" }}]\n");
        let c = parse_catalog(&format!("{t}{t}")).unwrap();
        let e = validate(&c, &|_| false);
        assert!(e.iter().any(|m| m.contains("duplicate id")));
        assert!(e.iter().any(|m| m.contains("missing string tweaks.item.x.name")));
        let e = one(&t.replace("min_build = 19041", "min_build = 22000, max_build = 19045"));
        assert!(e.iter().any(|m| m.contains("min_build")), "{e:?}");
        let e = one(&t.replace("9P1J8S7CCWWT", ""));
        assert!(e.iter().any(|m| m.contains("store_product_id")), "{e:?}");
        assert!(is_product_id("XP8BT8DW290MPQ"));
        assert!(!is_product_id("9P1J8S7CCWW"), "11 ký tự");
        assert!(!is_product_id("9p1j8s7ccwwt"), "chữ thường");
    }

    #[test]
    fn security_settings_are_rejected() {
        let p = "[[tweak]]\nid = \"y\"\ngroup = \"privacy\"\nlevel = \"basic\"\nrisk = \"safe\"\nwindows = { min_build = 19041 }\n";
        let e = one(&format!("{p}ops = [{{ kind = \"registry_set\", path = 'HKLM\\SOFTWARE\\Policies\\Microsoft\\Windows Defender', name = \"DisableAntiSpyware\", type = \"dword\", value = 1, default = \"absent\" }}]"));
        assert!(e.iter().any(|m| m.contains("off-limits")), "{e:?}");
        let e = one(&format!("{p}ops = [{{ kind = \"service_startup\", service = \"wuauserv\", start = \"disabled\", default = \"manual\" }}]"));
        assert!(e.iter().any(|m| m.contains("off-limits")), "{e:?}");
        let e = one(&format!("{p}ops = [{{ kind = \"registry_set\", path = 'HKCR\\x', name = \"n\", type = \"dword\", value = 1, default = \"absent\" }}]"));
        assert!(e.iter().any(|m| m.contains("HKCU")), "{e:?}");
    }

    /// Lệch kế hoạch có chủ ý: `"absent"` chỉ có nghĩa ở `default`; ở `value` của `registry_set` là lỗi.
    #[test]
    fn absent_as_registry_set_value_is_rejected() {
        let p = "[[tweak]]\nid = \"z\"\ngroup = \"privacy\"\nlevel = \"basic\"\nrisk = \"safe\"\nwindows = { min_build = 19041 }\n";
        let e = one(&format!("{p}ops = [{{ kind = \"registry_set\", path = 'HKCU\\Software\\X', name = \"n\", type = \"sz\", value = \"absent\", default = \"absent\" }}]"));
        assert!(e.iter().any(|m| m.contains("value must not be \"absent\"")), "{e:?}");
        let e = one(&format!("{p}ops = [{{ kind = \"registry_set\", path = 'HKCU\\Software\\X', name = \"n\", type = \"sz\", value = \"on\", default = \"absent\" }}]"));
        assert!(e.is_empty(), "{e:?}");
    }

    #[test]
    fn group_and_op_kind_must_agree() {
        let e = one(&format!("{HEAD}ops = [{{ kind = \"scheduled_task_disable\", task = '\\A\\B' }}]"));
        assert!(e.iter().any(|m| m.contains("bloatware tweaks hold only")), "{e:?}");
    }
}
