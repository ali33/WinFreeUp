//! Kiểu của danh mục `catalog.toml` và các kiểu dữ liệu dùng chung.
use serde::{Deserialize, Serialize};

use crate::ops::RegData;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    Privacy,
    Bloatware,
}

/// Thứ tự khai báo = thứ tự mức: Basic < Recommended < Aggressive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Basic,
    Recommended,
    Aggressive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Safe,
    Caution,
}

/// Thứ tự khai báo = mức nặng: None < Explorer < Logoff < Reboot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Restart {
    #[default]
    None,
    Explorer,
    Logoff,
    Reboot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StartType {
    Disabled,
    Manual,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RegType {
    Dword,
    Qword,
    Sz,
}

/// Giá trị như viết trong TOML: số nguyên hoặc chuỗi. Chuỗi `"absent"` ở `default` = mặc định không có value.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RegValue {
    Int(i64),
    Str(String),
}

pub const ABSENT: &str = "absent";

fn all_editions() -> Vec<String> {
    vec!["*".to_string()]
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WindowsReq {
    pub min_build: u32,
    /// 0 = không giới hạn.
    #[serde(default)]
    pub max_build: u32,
    /// `"*"` hoặc các giá trị trong `Home`, `Pro`, `Enterprise`, `Education`.
    #[serde(default = "all_editions")]
    pub editions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    RegistrySet {
        path: String,
        name: String,
        #[serde(rename = "type")]
        value_type: RegType,
        value: RegValue,
        default: RegValue,
    },
    RegistryDelete {
        path: String,
        name: String,
    },
    ServiceStartup {
        service: String,
        start: StartType,
        default: StartType,
    },
    ScheduledTaskDisable {
        task: String,
    },
    AppxRemove {
        package_family: String,
        store_product_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Tweak {
    pub id: String,
    pub group: Group,
    pub level: Level,
    pub risk: Risk,
    pub windows: WindowsReq,
    #[serde(default)]
    pub needs_restart: Restart,
    pub ops: Vec<Op>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogFile {
    tweak: Vec<Tweak>,
}

/// Đọc danh mục. Lỗi cú pháp/kiểu ⇒ `Err` kèm thông điệp nguyên văn của `toml`.
pub fn parse_catalog(text: &str) -> Result<Vec<Tweak>, String> {
    toml::from_str::<CatalogFile>(text).map(|f| f.tweak).map_err(|e| e.to_string())
}

/// Đổi giá trị TOML sang dữ liệu registry theo kiểu; sai kiểu hoặc tràn ⇒ `Err`.
pub fn to_reg_data(value_type: RegType, v: &RegValue) -> Result<RegData, String> {
    match (value_type, v) {
        (RegType::Dword, RegValue::Int(n)) => u32::try_from(*n).map(RegData::Dword).map_err(|_| format!("dword out of range: {n}")),
        (RegType::Qword, RegValue::Int(n)) => u64::try_from(*n).map(RegData::Qword).map_err(|_| format!("qword out of range: {n}")),
        (RegType::Sz, RegValue::Str(s)) => Ok(RegData::Sz(s.clone())),
        (t, v) => Err(format!("value {v:?} does not match type {t:?}")),
    }
}

/// `None` = mặc định của Windows là không có value.
pub fn default_data(value_type: RegType, v: &RegValue) -> Result<Option<RegData>, String> {
    match v {
        RegValue::Str(s) if s == ABSENT => Ok(None),
        other => to_reg_data(value_type, other).map(Some),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[[tweak]]
id            = "ads_start_suggestions"
group         = "privacy"
level         = "basic"
risk          = "safe"
windows       = { min_build = 19041, max_build = 0, editions = ["*"] }
needs_restart = "none"
ops = [
  { kind = "registry_set",
    path = 'HKCU\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager',
    name = "SubscribedContent-338388Enabled", type = "dword", value = 0, default = 1 },
]

[[tweak]]
id    = "app_clipchamp"
group = "bloatware"
level = "basic"
risk  = "safe"
windows = { min_build = 22000 }
ops = [ { kind = "appx_remove", package_family = "Clipchamp.Clipchamp_yxz26nhyzhsrt",
          store_product_id = "9P1J8S7CCWWT" } ]
"#;

    #[test]
    fn parses_spec_example() {
        let c = parse_catalog(SAMPLE).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].level, Level::Basic);
        assert_eq!(c[1].windows.editions, vec!["*"]);
        assert_eq!(c[1].windows.max_build, 0);
        assert_eq!(c[1].needs_restart, Restart::None);
        match &c[0].ops[0] {
            Op::RegistrySet { value_type, value, default, .. } => {
                assert_eq!(*value_type, RegType::Dword);
                assert_eq!(to_reg_data(*value_type, value).unwrap(), RegData::Dword(0));
                assert_eq!(default_data(*value_type, default).unwrap(), Some(RegData::Dword(1)));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn registry_set_without_default_is_rejected() {
        let bad = SAMPLE.replace(", default = 1", "");
        let err = parse_catalog(&bad).unwrap_err();
        assert!(err.contains("default"), "{err}");
    }

    #[test]
    fn unknown_field_is_rejected() {
        let bad = SAMPLE.replace("risk  = \"safe\"", "risk  = \"safe\"\ncolour = \"red\"");
        assert!(parse_catalog(&bad).is_err());
    }

    #[test]
    fn absent_default_and_range_checks() {
        assert_eq!(default_data(RegType::Dword, &RegValue::Str("absent".into())).unwrap(), None);
        assert!(to_reg_data(RegType::Dword, &RegValue::Int(-1)).is_err());
        assert!(to_reg_data(RegType::Dword, &RegValue::Int(1 << 33)).is_err());
        assert!(to_reg_data(RegType::Sz, &RegValue::Int(1)).is_err());
        assert_eq!(to_reg_data(RegType::Sz, &RegValue::Str("x".into())).unwrap(), RegData::Sz("x".into()));
    }

    #[test]
    fn levels_and_restarts_are_ordered() {
        assert!(Level::Basic < Level::Recommended && Level::Recommended < Level::Aggressive);
        assert!(Restart::None < Restart::Explorer && Restart::Logoff < Restart::Reboot);
    }
}
