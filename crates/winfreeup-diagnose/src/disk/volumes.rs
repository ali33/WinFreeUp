//! Danh sách ổ đĩa và chọn bộ quét cho từng ổ.
use serde::Serialize;
use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW};

use crate::util::{from_wide, wide};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DriveKind {
    Fixed,
    Removable,
    Network,
    Cdrom,
    Ram,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VolumeInfo {
    /// Dạng `C:\`.
    pub root: String,
    pub label: String,
    /// "NTFS", "FAT32", "exFAT", "ReFS"…
    pub fs: String,
    pub total: u64,
    pub free: u64,
    pub kind: DriveKind,
}

/// Bộ quét dùng cho một ổ, kèm lý do khi phải dùng đường chậm (giao diện dịch mã lý do).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ScanPlan {
    Mft,
    Walk { reason: WalkReason },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum WalkReason {
    /// Hệ tệp không phải NTFS (FAT32, exFAT, ReFS…).
    NotNtfs { fs: String },
    /// Ổ mạng, ổ CD… — không đọc MFT được.
    NotLocal { kind: DriveKind },
    /// Đã thử MFT nhưng hỏng; `message` là lỗi nguyên văn.
    MftFailed { message: String },
}

/// Spec 3.1: NTFS cục bộ (ổ cố định hoặc USB) ⇒ MFT; còn lại ⇒ duyệt thư mục.
pub fn plan_for(v: &VolumeInfo) -> ScanPlan {
    if !matches!(v.kind, DriveKind::Fixed | DriveKind::Removable) {
        return ScanPlan::Walk { reason: WalkReason::NotLocal { kind: v.kind } };
    }
    if !v.fs.eq_ignore_ascii_case("NTFS") {
        return ScanPlan::Walk { reason: WalkReason::NotNtfs { fs: v.fs.clone() } };
    }
    ScanPlan::Mft
}

/// Mã của `GetDriveTypeW` ⇒ loại ổ.
pub fn drive_kind(code: u32) -> DriveKind {
    match code {
        2 => DriveKind::Removable,
        3 => DriveKind::Fixed,
        4 => DriveKind::Network,
        5 => DriveKind::Cdrom,
        6 => DriveKind::Ram,
        _ => DriveKind::Unknown,
    }
}

/// Đọc thông tin một ổ theo gốc `X:\`. Ổ không có đĩa (đầu đọc thẻ trống, CD rỗng) ⇒ `None`.
pub fn volume_info(root: &str) -> Option<VolumeInfo> {
    let w = wide(root);
    let mut label = [0u16; 261];
    let mut fs = [0u16; 261];
    // SAFETY: w là chuỗi kết thúc NUL; hai bộ đệm khả ghi đúng độ dài truyền vào; các con trỏ null là tham số
    // tùy chọn mà API cho phép bỏ qua.
    let ok = unsafe {
        GetVolumeInformationW(
            w.as_ptr(),
            label.as_mut_ptr(),
            label.len() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            fs.as_mut_ptr(),
            fs.len() as u32,
        )
    };
    if ok == 0 {
        return None;
    }
    let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
    // SAFETY: w là chuỗi kết thúc NUL; ba biến u64 cục bộ khả ghi.
    let ok = unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, &mut total, &mut free) };
    if ok == 0 {
        return None;
    }
    Some(VolumeInfo {
        root: root.to_string(),
        label: from_wide(&label),
        fs: from_wide(&fs),
        total,
        free,
        // SAFETY: w là chuỗi kết thúc NUL còn sống.
        kind: drive_kind(unsafe { GetDriveTypeW(w.as_ptr()) }),
    })
}

/// Mọi ổ có ký tự, theo thứ tự chữ cái.
pub fn list_volumes() -> Vec<VolumeInfo> {
    // SAFETY: không tham số.
    let mask = unsafe { GetLogicalDrives() };
    (0..26u8)
        .filter(|i| mask & (1 << i) != 0)
        .filter_map(|i| volume_info(&format!("{}:\\", (b'A' + i) as char)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vol(fs: &str, kind: DriveKind) -> VolumeInfo {
        VolumeInfo { root: "E:\\".into(), label: String::new(), fs: fs.into(), total: 1, free: 1, kind }
    }

    #[test]
    fn ntfs_local_drives_use_the_mft() {
        assert_eq!(plan_for(&vol("NTFS", DriveKind::Fixed)), ScanPlan::Mft);
        assert_eq!(plan_for(&vol("NTFS", DriveKind::Removable)), ScanPlan::Mft);
    }

    #[test]
    fn other_drives_walk_with_a_reason() {
        assert_eq!(plan_for(&vol("FAT32", DriveKind::Removable)), ScanPlan::Walk { reason: WalkReason::NotNtfs { fs: "FAT32".into() } });
        assert_eq!(plan_for(&vol("exFAT", DriveKind::Fixed)), ScanPlan::Walk { reason: WalkReason::NotNtfs { fs: "exFAT".into() } });
        assert_eq!(plan_for(&vol("NTFS", DriveKind::Network)), ScanPlan::Walk { reason: WalkReason::NotLocal { kind: DriveKind::Network } });
    }

    #[test]
    fn plan_serializes_for_the_ui() {
        let v = serde_json::to_value(ScanPlan::Walk { reason: WalkReason::MftFailed { message: "Access is denied. (os error 5)".into() } }).unwrap();
        assert_eq!(v, serde_json::json!({"mode": "walk", "reason": {"code": "mft_failed", "message": "Access is denied. (os error 5)"}}));
        assert_eq!(serde_json::to_value(ScanPlan::Mft).unwrap(), serde_json::json!({"mode": "mft"}));
    }

    #[test]
    fn this_machine_has_its_system_drive_listed() {
        let sys = crate::util::system_drive_root().unwrap();
        let sys = sys.to_str().unwrap();
        let all = list_volumes();
        let c = all.iter().find(|v| v.root.eq_ignore_ascii_case(sys)).expect("ổ hệ thống");
        assert!(c.total > 0 && c.free <= c.total);
        assert_eq!(c.kind, DriveKind::Fixed);
        assert!(!c.fs.is_empty());
    }
}
