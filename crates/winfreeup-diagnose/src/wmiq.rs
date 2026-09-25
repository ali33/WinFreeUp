//! Truy vấn WMI gọn: mỗi lỗi trả về thông điệp nguyên văn để hiện «Không đo được: <lý do>».
//! Kết nối WMI không chuyển luồng được (`!Send`), nên mỗi luồng tự mở kết nối của mình.
use serde::de::DeserializeOwned;
use wmi::{COMLibrary, WMIConnection};

pub const NS_CIMV2: &str = r"ROOT\CIMV2";
pub const NS_WMI: &str = r"ROOT\WMI";
pub const NS_STORAGE: &str = r"ROOT\Microsoft\Windows\Storage";

/// Khởi tạo COM (MTA) cho luồng hiện tại rồi mở kết nối. Luồng đã là STA thì dùng luôn COM sẵn có.
pub fn connect(namespace: &str) -> Result<WMIConnection, String> {
    // SAFETY: nhánh lỗi chỉ xảy ra khi luồng đã khởi tạo COM (khác chế độ) — COM vẫn dùng được.
    let com = COMLibrary::new().unwrap_or_else(|_| unsafe { COMLibrary::assume_initialized() });
    WMIConnection::with_namespace_path(namespace, com).map_err(|e| format!("{namespace}: {e}"))
}

/// Một câu WQL trên kết nối có sẵn.
pub fn query_on<T: DeserializeOwned>(conn: &WMIConnection, wql: &str) -> Result<Vec<T>, String> {
    conn.raw_query(wql).map_err(|e| format!("{wql}: {e}"))
}

/// Mở kết nối, chạy một câu WQL, đóng kết nối.
pub fn query<T: DeserializeOwned>(namespace: &str, wql: &str) -> Result<Vec<T>, String> {
    query_on(&connect(namespace)?, wql)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Os {
        caption: String,
    }

    #[test]
    fn cimv2_answers_without_admin() {
        let v: Vec<Os> = query(NS_CIMV2, "SELECT Caption FROM Win32_OperatingSystem").unwrap();
        assert!(v[0].caption.contains("Windows"));
    }

    #[test]
    fn bad_class_is_an_error_message_not_a_panic() {
        let e = query::<Os>(NS_CIMV2, "SELECT Caption FROM Khong_Co_Lop").unwrap_err();
        assert!(e.contains("Khong_Co_Lop"), "{e}");
    }
}
