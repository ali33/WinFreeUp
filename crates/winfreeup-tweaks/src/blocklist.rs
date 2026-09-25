//! Danh sách cấm gỡ — mã hoá cứng, danh mục không ghi đè được (spec mục 3.5),
//! và danh sách cấm đụng (Defender, Windows Update, SmartScreen, Firewall — spec mục 1).

/// Tên gói (phần trước `_` của package family). `*` ở cuối = khớp tiền tố. So khớp không phân biệt hoa thường.
pub const BLOCKED_PACKAGES: &[&str] = &[
    "Microsoft.WindowsStore",
    "Microsoft.MicrosoftEdge*",
    "Microsoft.DesktopAppInstaller",
    "Microsoft.SecHealthUI",
    "Microsoft.Windows.Photos",
    "Microsoft.WindowsCalculator",
    "Microsoft.WindowsNotepad",
    "Microsoft.Paint",
    "Microsoft.ScreenSketch",
    "Microsoft.WindowsTerminal",
    "Microsoft.WindowsCamera",
    "Microsoft.XboxIdentityProvider",
    "Microsoft.Xbox.TCUI",
    "Microsoft.VCLibs*",
    "Microsoft.UI.Xaml*",
    "Microsoft.NET.Native*",
    "Microsoft.WindowsAppRuntime*",
    "Microsoft.Windows.StartMenuExperienceHost",
    "Microsoft.Windows.ShellExperienceHost",
    "MicrosoftWindows.Client.CBS",
    // Teams mới — dùng chung cho công việc và cá nhân. Chỉ Teams cá nhân cũ (`MicrosoftTeams`) mới được gỡ.
    "MSTeams",
];

/// Chuỗi con (không phân biệt hoa thường) mà đường dẫn registry của danh mục không được chứa.
pub const FORBIDDEN_REG_FRAGMENTS: &[&str] = &[
    r"\Windows Defender",
    r"\Microsoft Defender",
    r"\WindowsUpdate",
    r"\SmartScreen",
    r"\WindowsFirewall",
    r"\SharedAccess",
    r"\wuauserv",
    r"\WinDefend",
    r"\mpssvc",
];

pub const FORBIDDEN_SERVICES: &[&str] = &[
    "WinDefend", "WdNisSvc", "SecurityHealthService", "wscsvc", "Sense", "mpssvc", "BFE", "wuauserv", "UsoSvc",
    "WaaSMedicSvc", "BITS", "DoSvc", "AppXSvc", "ClipSVC", "InstallService",
];

/// `Name_PublisherId` ⇒ `Name`. Chuỗi không có `_` thì trả nguyên.
pub fn package_name(family: &str) -> &str {
    family.rsplit_once('_').map(|(n, _)| n).unwrap_or(family)
}

fn matches(pattern: &str, name: &str) -> bool {
    let (p, n) = (pattern.to_ascii_lowercase(), name.to_ascii_lowercase());
    match p.strip_suffix('*') {
        Some(prefix) => n.starts_with(prefix),
        None => n == p,
    }
}

/// true nếu gói (tên hoặc family) thuộc danh sách cấm gỡ.
pub fn is_blocked_package(name_or_family: &str) -> bool {
    let name = package_name(name_or_family);
    BLOCKED_PACKAGES.iter().any(|p| matches(p, name))
}

pub fn is_forbidden_reg_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    FORBIDDEN_REG_FRAGMENTS.iter().any(|f| p.contains(&f.to_ascii_lowercase()))
}

pub fn is_forbidden_service(name: &str) -> bool {
    FORBIDDEN_SERVICES.iter().any(|s| s.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_prefix_matching() {
        assert!(is_blocked_package("Microsoft.WindowsStore_8wekyb3d8bbwe"));
        assert!(is_blocked_package("microsoft.windowsstore"));
        assert!(is_blocked_package("Microsoft.MicrosoftEdge.Stable_8wekyb3d8bbwe"));
        assert!(is_blocked_package("Microsoft.VCLibs.140.00_8wekyb3d8bbwe"));
        assert!(is_blocked_package("Microsoft.Paint_8wekyb3d8bbwe"));
        assert!(!is_blocked_package("Microsoft.MSPaint_8wekyb3d8bbwe"), "Paint 3D không phải Paint mới");
        assert!(!is_blocked_package("Clipchamp.Clipchamp_yxz26nhyzhsrt"));
        assert!(is_blocked_package("MSTeams_8wekyb3d8bbwe"), "Teams công việc/mới không bao giờ bị gỡ");
        assert!(!is_blocked_package("MicrosoftTeams_8wekyb3d8bbwe"), "Teams cá nhân cũ là gói khác");
        assert!(!is_blocked_package("Microsoft.WindowsStoreX_1"), "khớp đúng tên, không khớp tiền tố khi không có *");
    }

    #[test]
    fn package_name_strips_publisher() {
        assert_eq!(package_name("Microsoft.Windows.Photos_8wekyb3d8bbwe"), "Microsoft.Windows.Photos");
        assert_eq!(package_name("king.com.CandyCrushSaga_kgqvnymyfvs32"), "king.com.CandyCrushSaga");
        assert_eq!(package_name("NoUnderscore"), "NoUnderscore");
    }

    #[test]
    fn security_components_are_forbidden() {
        assert!(is_forbidden_reg_path(r"HKLM\SOFTWARE\Policies\Microsoft\Windows Defender"));
        assert!(is_forbidden_reg_path(r"HKLM\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU"));
        assert!(!is_forbidden_reg_path(r"HKLM\SOFTWARE\Policies\Microsoft\Windows\DataCollection"));
        assert!(is_forbidden_service("windefend"));
        assert!(!is_forbidden_service("DiagTrack"));
    }
}
