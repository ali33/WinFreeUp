//! Truy vấn WMI gọn: mỗi lỗi trả về thông điệp nguyên văn để hiện «Không đo được: <lý do>».
//! Kết nối WMI không chuyển luồng được (`!Send`), nên mỗi luồng tự mở kết nối của mình.
//!
//! Bảo mật COM: `COMLibrary::new()` (wmi 0.16.0) gọi `CoInitializeEx(MTA)` rồi `CoInitializeSecurity` mặc định;
//! nếu tiến trình đã gọi `CoInitializeSecurity` trước (Task 20: SD chỉ SYSTEM + Administrators) thì lần gọi của wmi
//! nhận `RPC_E_TOO_LATE` và wmi bỏ qua lỗi đó, giữ nguyên cấu hình đã đặt. Vì «ai gọi trước thắng», vỏ app phải gọi
//! `CoInitializeSecurity` TRƯỚC mọi kết nối WMI đầu tiên.
use serde::de::DeserializeOwned;
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use wmi::{COMLibrary, WMIConnection, WMIError};

pub const NS_CIMV2: &str = r"ROOT\CIMV2";
pub const NS_WMI: &str = r"ROOT\WMI";
pub const NS_STORAGE: &str = r"ROOT\Microsoft\Windows\Storage";

/// Chỉ khi luồng đã khởi tạo COM ở chế độ khác (STA, `RPC_E_CHANGED_MODE`) mới dùng COM sẵn có;
/// mọi lỗi khác thành thông điệp `<namespace>: COM: <lỗi>`.
fn com_or_error(r: Result<COMLibrary, WMIError>, namespace: &str) -> Result<COMLibrary, String> {
    match r {
        Ok(c) => Ok(c),
        // SAFETY: RPC_E_CHANGED_MODE nghĩa là COM ĐÃ được khởi tạo trên luồng này (STA) và vẫn dùng được;
        // COMLibrary không gọi CoUninitialize khi drop nên không phá trạng thái COM của luồng.
        Err(WMIError::HResultError { hres }) if hres == RPC_E_CHANGED_MODE.0 => Ok(unsafe { COMLibrary::assume_initialized() }),
        Err(e) => Err(format!("{namespace}: COM: {e}")),
    }
}

/// Khởi tạo COM (MTA) cho luồng hiện tại rồi mở kết nối. Luồng đã là STA thì dùng luôn COM sẵn có.
pub fn connect(namespace: &str) -> Result<WMIConnection, String> {
    let com = com_or_error(COMLibrary::new(), namespace)?;
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

    #[test]
    fn only_changed_mode_falls_back_to_the_existing_com() {
        let changed = Err(WMIError::HResultError { hres: RPC_E_CHANGED_MODE.0 });
        assert!(com_or_error(changed, NS_CIMV2).is_ok());
        // E_ACCESSDENIED: không được coi là «COM đã sẵn», phải thành lỗi có chữ.
        let denied = Err(WMIError::HResultError { hres: 0x8007_0005_u32 as i32 });
        let e = com_or_error(denied, NS_CIMV2).err().unwrap();
        assert!(e.starts_with(r"ROOT\CIMV2: COM: "), "{e}");
    }

    #[test]
    fn sta_thread_still_connects() {
        // Luồng đã khởi tạo COM kiểu STA ⇒ COMLibrary::new trả RPC_E_CHANGED_MODE ⇒ dùng COM sẵn có.
        std::thread::spawn(|| {
            // SAFETY: khởi tạo COM cho chính luồng test này; luồng kết thúc ngay sau đó.
            unsafe { windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED) }
                .ok()
                .unwrap();
            let v: Vec<Os> = query(NS_CIMV2, "SELECT Caption FROM Win32_OperatingSystem").unwrap();
            assert!(v[0].caption.contains("Windows"));
        })
        .join()
        .unwrap();
    }
}
