//! Mọi thao tác chạm hệ thống của phần Tinh chỉnh. Bản thật ở `sys_windows::RealTweakOps`,
//! bản giả ở `fake::FakeOps`. Lỗi là chuỗi nguyên văn để đưa thẳng lên màn và nhật ký.
use serde::{Deserialize, Serialize};

use crate::model::StartType;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "lowercase")]
pub enum RegData {
    Dword(u32),
    Qword(u64),
    Sz(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackageInfo {
    pub full_name: String,
    pub family: String,
    pub is_framework: bool,
    /// Gói ký `System` — Windows không cho gỡ.
    pub non_removable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SystemInfo {
    /// `CurrentBuildNumber`, vd 26200.
    pub build: u32,
    /// Một trong `Home`, `Pro`, `Enterprise`, `Education`, `Other` — xem `edition_of`.
    pub edition: String,
    /// Máy vào domain hoặc đăng ký MDM (ProviderID `MS DM Server`).
    pub managed: bool,
    /// Ứng dụng đang chạy bằng tài khoản khác người đang đăng nhập (UAC nhập mật khẩu admin khác).
    pub other_user: bool,
}

/// `EditionID` trong registry ⇒ nhóm edition của danh mục.
pub fn edition_of(edition_id: &str) -> &'static str {
    let e = edition_id.to_ascii_lowercase();
    if e.starts_with("core") {
        "Home"
    } else if e.starts_with("professional") {
        "Pro"
    } else if e.starts_with("enterprise") || e.starts_with("iotenterprise") {
        "Enterprise"
    } else if e.starts_with("education") {
        "Education"
    } else {
        "Other"
    }
}

pub trait TweakOps: Send + Sync {
    /// `path` dạng `HKCU\...` hoặc `HKLM\...`. `Ok(None)` = key hoặc value không tồn tại.
    fn reg_read(&self, path: &str, name: &str) -> Result<Option<RegData>, String>;
    /// Tạo key nếu chưa có.
    fn reg_write(&self, path: &str, name: &str, data: &RegData) -> Result<(), String>;
    /// Value không tồn tại ⇒ `Ok(())`.
    fn reg_delete(&self, path: &str, name: &str) -> Result<(), String>;
    /// `Ok(None)` = dịch vụ không có trên máy.
    fn service_start(&self, name: &str) -> Result<Option<StartType>, String>;
    fn set_service_start(&self, name: &str, start: StartType) -> Result<(), String>;
    /// Dịch vụ vốn đã dừng ⇒ `Ok(())`.
    fn stop_service(&self, name: &str) -> Result<(), String>;
    /// `path` đầy đủ, vd `\Microsoft\Windows\...\Consolidator`. `Ok(None)` = tác vụ không có.
    fn task_enabled(&self, path: &str) -> Result<Option<bool>, String>;
    fn set_task_enabled(&self, path: &str, enabled: bool) -> Result<(), String>;
    /// Các gói của người dùng hiện tại thuộc `family` (thường 0 hoặc 1 gói).
    fn packages(&self, family: &str) -> Result<Vec<PackageInfo>, String>;
    fn remove_package(&self, full_name: &str, all_users: bool) -> Result<(), String>;
    fn deprovision(&self, family: &str) -> Result<(), String>;
    fn open_uri(&self, uri: &str) -> Result<(), String>;
    fn system_info(&self) -> Result<SystemInfo, String>;
    fn restart_explorer(&self) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edition_mapping() {
        assert_eq!(edition_of("Core"), "Home");
        assert_eq!(edition_of("CoreSingleLanguage"), "Home");
        assert_eq!(edition_of("Professional"), "Pro");
        assert_eq!(edition_of("ProfessionalWorkstation"), "Pro");
        assert_eq!(edition_of("ProfessionalEducation"), "Pro");
        assert_eq!(edition_of("Enterprise"), "Enterprise");
        assert_eq!(edition_of("EnterpriseS"), "Enterprise");
        assert_eq!(edition_of("Education"), "Education");
        assert_eq!(edition_of("ServerStandard"), "Other");
    }

    #[test]
    fn reg_data_json_shape() {
        assert_eq!(serde_json::to_string(&RegData::Dword(1)).unwrap(), r#"{"type":"dword","value":1}"#);
        let back: RegData = serde_json::from_str(r#"{"type":"sz","value":"a"}"#).unwrap();
        assert_eq!(back, RegData::Sz("a".into()));
    }
}
