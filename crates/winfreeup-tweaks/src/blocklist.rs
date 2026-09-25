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
    // Teams mới — dùng chung cho công việc và cá nhân.
    // Teams cá nhân cũ (MicrosoftTeams) không bị chặn nhưng hiện không có trong danh mục.
    "MSTeams",
    // Thêm sau lượt rà Task 2: Store/winget, codec ảnh-video, đăng nhập tài khoản, gói ngôn ngữ, OneDrive, dịch vụ game.
    // Cố ý KHÔNG chặn tiền tố `MicrosoftWindows.Client.*` — Widgets (`...Client.WebExperience`) được phép gỡ.
    "Microsoft.StorePurchaseApp",
    "Microsoft.Services.Store.Engagement",
    "Microsoft.Winget.Source*",
    "Microsoft.HEIFImageExtension",
    "Microsoft.HEVCVideoExtension",
    "Microsoft.VP9VideoExtensions",
    "Microsoft.WebMediaExtensions",
    "Microsoft.WebpImageExtension",
    "Microsoft.RawImageExtension",
    "Microsoft.AV1VideoExtension",
    "Microsoft.AAD.BrokerPlugin",
    "Microsoft.AccountsControl",
    "Microsoft.Windows.CloudExperienceHost",
    "Microsoft.LanguageExperiencePack*",
    "Microsoft.OneDriveSync",
    "Microsoft.GamingServices",
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
    r"\PolicyManager\",
    r"\Microsoft\MRT",
];

/// Tên value registry (không phân biệt hoa thường) mà danh mục không được đặt hay xoá, ở bất kỳ khoá nào.
pub const FORBIDDEN_VALUE_NAMES: &[&str] = &[
    "EnableSmartScreen",
    "SmartScreenEnabled",
    "ShellSmartScreenLevel",
    "EnableWebContentEvaluation",
    "DisableAntiSpyware",
    "DisableRealtimeMonitoring",
    "NoAutoUpdate",
    "NoWindowsUpdate",
];

/// Dịch vụ cấm đụng — qua `service_startup` và qua khoá `...\Services\<tên>`. `*` ở cuối = khớp tiền tố
/// (dịch vụ theo phiên người dùng có hậu tố `_<LUID>`). So khớp không phân biệt hoa thường.
pub const FORBIDDEN_SERVICES: &[&str] = &[
    "WinDefend", "MDCoreSvc", "WdBoot", "WdFilter", "WdNisDrv", "WdNisSvc", "Sense", "SecurityHealthService",
    "wscsvc", "webthreatdefsvc", "webthreatdefusersvc*", "SgrmBroker", "mpssvc", "mpsdrv", "BFE", "wuauserv",
    "UsoSvc", "WaaSMedicSvc", "BITS", "DoSvc", "TrustedInstaller", "AppXSvc", "ClipSVC", "InstallService",
];

/// Chuỗi con (không phân biệt hoa thường) mà đường dẫn tác vụ theo lịch của danh mục không được chứa.
pub const FORBIDDEN_TASK_FRAGMENTS: &[&str] = &[
    r"\Windows Defender\",
    r"\UpdateOrchestrator\",
    r"\WindowsUpdate\",
    r"\WaaSMedic\",
    r"\InstallService\",
    r"\ExploitGuard\",
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

fn contains_any(haystack: &str, fragments: &[&str]) -> bool {
    let h = haystack.to_ascii_lowercase();
    fragments.iter().any(|f| h.contains(&f.to_ascii_lowercase()))
}

/// true nếu đường dẫn chạm thành phần bảo mật/cập nhật, kể cả khoá `...\Services\<dịch vụ cấm đụng>`.
pub fn is_forbidden_reg_path(path: &str) -> bool {
    if contains_any(path, FORBIDDEN_REG_FRAGMENTS) {
        return true;
    }
    let p = path.to_ascii_lowercase();
    p.match_indices(r"\services\")
        .any(|(at, m)| is_forbidden_service(p[at + m.len()..].split('\\').next().unwrap_or("")))
}

pub fn is_forbidden_service(name: &str) -> bool {
    FORBIDDEN_SERVICES.iter().any(|s| matches(s, name))
}

pub fn is_forbidden_value_name(name: &str) -> bool {
    FORBIDDEN_VALUE_NAMES.iter().any(|n| n.eq_ignore_ascii_case(name))
}

pub fn is_forbidden_task(task: &str) -> bool {
    contains_any(task, FORBIDDEN_TASK_FRAGMENTS)
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

    /// Lệch kế hoạch có chủ ý (lượt rà Task 2): lưới cấm đụng dày hơn.
    #[test]
    fn wider_security_net() {
        for n in ["EnableSmartScreen", "smartscreenenabled", "ShellSmartScreenLevel", "EnableWebContentEvaluation",
            "DisableAntiSpyware", "DisableRealtimeMonitoring", "NoAutoUpdate", "NOWINDOWSUPDATE"] {
            assert!(is_forbidden_value_name(n), "{n}");
        }
        assert!(!is_forbidden_value_name("AllowTelemetry"));
        assert!(is_forbidden_reg_path(r"HKLM\SOFTWARE\Microsoft\PolicyManager\current\device\Update"));
        assert!(is_forbidden_reg_path(r"HKLM\SOFTWARE\Policies\Microsoft\MRT"));
        assert!(is_forbidden_reg_path(r"HKLM\SYSTEM\CurrentControlSet\Services\SgrmBroker"));
        assert!(is_forbidden_reg_path(r"HKLM\SYSTEM\CurrentControlSet\Services\webthreatdefusersvc_3a1b2\Parameters"));
        assert!(!is_forbidden_reg_path(r"HKLM\SYSTEM\CurrentControlSet\Services\DiagTrack"));
        for s in ["MDCoreSvc", "WdBoot", "WdFilter", "WdNisDrv", "webthreatdefsvc", "webthreatdefusersvc_3a1b2",
            "SgrmBroker", "TrustedInstaller", "mpsdrv", "BFE", "UsoSvc"] {
            assert!(is_forbidden_service(s), "{s}");
        }
        assert!(!is_forbidden_service("webthreatdefsvcX"), "không có * thì khớp đúng tên");
        assert!(is_forbidden_task(r"\Microsoft\Windows\Windows Defender\Windows Defender Scheduled Scan"));
        assert!(is_forbidden_task(r"\Microsoft\Windows\UpdateOrchestrator\Schedule Scan"));
        assert!(is_forbidden_task(r"\microsoft\windows\waasmedic\PerformRemediation"));
        assert!(is_forbidden_task(r"\Microsoft\Windows\ExploitGuard\ExploitGuard MDM policy Refresh"));
        assert!(!is_forbidden_task(r"\Microsoft\Windows\Customer Experience Improvement Program\Consolidator"));
    }

    #[test]
    fn system_packages_added_after_review_are_blocked() {
        for p in ["Microsoft.StorePurchaseApp_8wekyb3d8bbwe", "Microsoft.Services.Store.Engagement_8wekyb3d8bbwe",
            "Microsoft.Winget.Source_8wekyb3d8bbwe", "Microsoft.HEIFImageExtension_8wekyb3d8bbwe",
            "Microsoft.HEVCVideoExtension_8wekyb3d8bbwe", "Microsoft.VP9VideoExtensions_8wekyb3d8bbwe",
            "Microsoft.WebMediaExtensions_8wekyb3d8bbwe", "Microsoft.WebpImageExtension_8wekyb3d8bbwe",
            "Microsoft.RawImageExtension_8wekyb3d8bbwe", "Microsoft.AV1VideoExtension_8wekyb3d8bbwe",
            "Microsoft.AAD.BrokerPlugin_cw5n1h2txyewy", "Microsoft.AccountsControl_cw5n1h2txyewy",
            "Microsoft.Windows.CloudExperienceHost_cw5n1h2txyewy", "Microsoft.LanguageExperiencePackvi-VN_8wekyb3d8bbwe",
            "Microsoft.OneDriveSync_8wekyb3d8bbwe", "Microsoft.GamingServices_8wekyb3d8bbwe"] {
            assert!(is_blocked_package(p), "{p}");
        }
        assert!(!is_blocked_package("MicrosoftWindows.Client.WebExperience_cw5n1h2txyewy"), "Widgets cố ý cho gỡ");
    }
}
