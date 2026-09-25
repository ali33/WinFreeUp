# WinFreeUp — Dự án con 3: Tinh chỉnh (gỡ bloatware + quyền riêng tư)

- Ngày: 2026-09-25
- Trạng thái: chờ duyệt
- Làm sau: dự án con 2 (`2026-09-25-winfreeup-kham-may-design.md`); dùng chung khung của v0.1

## 1. Mục tiêu và bối cảnh

Gộp hai mảng của tầm nhìn ban đầu — **Gỡ bloatware** và **Quyền riêng tư** — thành một tab
**"Tinh chỉnh"**, vì cả hai cùng một khuôn: một danh sách thay đổi cấu hình Windows, mỗi mục có
trạng thái hiện tại, áp dụng, hoàn tác.

### Tiêu chí thành công

Người dùng phổ thông bấm một mức sẵn (**Cơ bản / Khuyến nghị / Triệt để**), xem danh sách thay đổi,
bấm **Áp dụng**, và:
- mọi mục báo đúng kết quả đọc lại từ máy thật,
- mọi mục **hoàn tác được từ trong app**,
- máy không mất tính năng bảo mật và không cần cài lại Windows.

### Ràng buộc (kế thừa v0.1)

- Một file `.exe`, `requireAdministrator`, Windows 10 1903+ / 11, 64-bit.
- Tauri 2 + React + TypeScript + Fluent UI React v9; lõi Rust `winfreeup-core`.
- Tiếng Việt, chuỗi trong `src/i18n/vi.json`; luật lỗi và nhật ký chung.
- **Không đụng**: Defender, Windows Update, SmartScreen, Firewall.
- **Không gỡ**: thành phần mà thiếu nó phải cài lại Windows (Store, Edge, framework, shell).

## 2. Danh mục dạng dữ liệu

`tweaks/catalog.toml`, nhúng vào exe lúc build (`include_str!`).

```toml
[[tweak]]
id            = "ads_start_suggestions"     # khoá kỹ thuật, tra chuỗi trong vi.json
group         = "privacy"                   # privacy | bloatware
level         = "basic"                     # basic | recommended | aggressive
risk          = "safe"                      # safe | caution
windows       = { min_build = 19041, max_build = 0, editions = ["*"] }  # max_build = 0: không giới hạn
needs_restart = "none"                      # none | explorer | logoff | reboot
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
windows = { min_build = 22000, max_build = 0, editions = ["*"] }
ops = [ { kind = "appx_remove", package_family = "Clipchamp.Clipchamp_yxz26nhyzhsrt",
          store_product_id = "9P1J8S7CCWWT" } ]
```

- `editions`: `"*"` hoặc danh sách trong `Home`, `Pro`, `Enterprise`, `Education`.
- `default`: giá trị mặc định của Windows, dùng khi hoàn tác mà không có ảnh chụp. `default = "absent"`
  nghĩa là mặc định khóa không tồn tại.
- Tên gói, ProductId Store và khóa registry trong danh mục **phải được xác minh trên máy thật**
  (Win 10 22H2, Win 11 23H2/24H2) khi viết danh mục — không chép mù từ nguồn khác.

## 3. Bộ máy

### 3.1 Thao tác cơ bản

| kind | Tham số | read_state | apply | revert |
|---|---|---|---|---|
| `registry_set` | path, name, type, value | đọc giá trị (hoặc "absent") | ghi | ghi lại ảnh chụp; ảnh chụp "absent" ⇒ xóa value |
| `registry_delete` | path, name | có/không | xóa | ghi lại ảnh chụp |
| `service_startup` | service, start (`disabled`/`manual`/`auto`) | kiểu khởi động | đổi kiểu + dừng dịch vụ nếu `disabled` | đổi về kiểu cũ (không tự khởi động dịch vụ) |
| `scheduled_task_disable` | task path | bật/tắt | tắt | bật lại nếu ảnh chụp là bật |
| `appx_remove` | package_family, store_product_id | cài/không (theo scope) | gỡ | mở `ms-windows-store://pdp/?ProductId=<id>` |

Mọi thao tác đi qua trait `SystemOps` (registry, dịch vụ, tác vụ, gói app) để test thay bằng lớp giả.

### 3.2 Trạng thái một mục

Luôn **tính từ máy thật**, không từ ký ức của app:

- **Đã áp dụng**: mọi thao tác khớp đích.
- **Chưa áp dụng**: không thao tác nào khớp.
- **Một phần**: một số khớp (hổ phách).
- **Không hỗ trợ trên máy này**: sai build/edition — hiện mờ kèm lý do, không cho chọn.
- **Bị chính sách tổ chức quản lý**: máy vào domain hoặc đăng ký MDM **và** giá trị dưới
  `...\Policies\...` đã tồn tại với giá trị khác đích trước khi áp dụng — khóa, không cho áp dụng.

### 3.3 Hoàn tác

- Trước khi chạy một thao tác, chụp giá trị cũ vào `%LOCALAPPDATA%\WinFreeUp\tweaks-undo.json`
  (theo `tweak_id` + chỉ số thao tác). **Chỉ ghi lần đầu** — áp dụng lại không đè ảnh chụp gốc.
- Hoàn tác thành công ⇒ xóa ảnh chụp của mục đó.
- Không có ảnh chụp ⇒ dùng `default` trong danh mục.
- Ghi file ảnh chụp theo kiểu ghi file tạm rồi đổi tên (không hỏng khi mất điện giữa chừng).

### 3.4 Lượt áp dụng

1. Tạo điểm khôi phục (không được ⇒ băng hổ phách, người dùng chọn tiếp/dừng — như v0.1).
2. Chạy lần lượt từng thao tác; lỗi ⇒ ghi lại, chạy tiếp thao tác khác (không giao dịch toàn cục,
   nhưng thao tác nào đã chạy đều có ảnh chụp).
3. Đọc lại trạng thái thật, báo kết quả từng mục.
4. Gom `needs_restart`: `explorer` ⇒ nút "Khởi động lại Explorer"; `logoff`/`reboot` ⇒ nhắc.
5. Ghi nhật ký chung.

### 3.5 Gỡ app — phạm vi

- **Mặc định: tài khoản hiện tại** (`PackageManager.RemovePackageAsync` với gói của người dùng đang
  đăng nhập — lưu ý app chạy Admin nhưng vẫn là cùng người dùng).
- **Nâng cao: mọi tài khoản + chặn cài lại** (`RemovePackageOptions.RemoveForAllUsers` +
  `DeprovisionPackageForAllUsersAsync`): ô tích riêng, cảnh báo "Khó hoàn tác; bản cập nhật Windows lớn
  có thể vẫn cài lại một số app".
- **Danh sách cấm gỡ mã hóa cứng trong lõi** (danh mục không ghi đè được; test chặn danh mục vi phạm):
  `Microsoft.WindowsStore`, `Microsoft.MicrosoftEdge*`, `Microsoft.DesktopAppInstaller`,
  `Microsoft.SecHealthUI`, `Microsoft.Windows.Photos`, `Microsoft.WindowsCalculator`,
  `Microsoft.WindowsNotepad`, `Microsoft.Paint`, `Microsoft.ScreenSketch`,
  `Microsoft.WindowsTerminal`, `Microsoft.WindowsCamera`, `Microsoft.XboxIdentityProvider`,
  `Microsoft.Xbox.TCUI`, mọi gói framework (`Microsoft.VCLibs*`, `Microsoft.UI.Xaml*`,
  `Microsoft.NET.Native*`, `Microsoft.WindowsAppRuntime*`), mọi gói `IsFramework`/`NonRemovable`,
  và gói shell (`Microsoft.Windows.StartMenuExperienceHost`, `Microsoft.Windows.ShellExperienceHost`,
  `MicrosoftWindows.Client.CBS`).

## 4. Nội dung danh mục v1

### 4.1 Gỡ app

| Mức | App (package family gốc) |
|---|---|
| Cơ bản | Candy Crush (`king.com.*`), TikTok (`BytedancePte.Ltd.TikTok`), Instagram (`Facebook.Instagram*`), Disney+ (`Disney.*`), Spotify ghim sẵn (`SpotifyAB.SpotifyMusic`), Mẹo (`Microsoft.Getstarted`), Trung tâm Phản hồi (`Microsoft.WindowsFeedbackHub`), Tin tức (`Microsoft.BingNews`), Microsoft 365 hub (`Microsoft.MicrosoftOfficeHub`), Clipchamp (`Clipchamp.Clipchamp`), Power Automate (`Microsoft.PowerAutomateDesktop`); Win 10: Mixed Reality Portal (`Microsoft.MixedReality.Portal`), 3D Viewer (`Microsoft.Microsoft3DViewer`), Paint 3D (`Microsoft.MSPaint` — **không nhầm** với Paint mới `Microsoft.Paint`), Skype (`Microsoft.SkypeApp`), People (`Microsoft.People`) |
| Khuyến nghị | + Thời tiết (`Microsoft.BingWeather`), Bản đồ (`Microsoft.WindowsMaps`), Solitaire (`Microsoft.MicrosoftSolitaireCollection`), Teams cá nhân (`MicrosoftTeams` — **không** gỡ `MSTeams`, bản dùng cho công việc), Copilot (`Microsoft.Copilot`), Widgets (`MicrosoftWindows.Client.WebExperience`), Family (`MicrosoftCorporationII.MicrosoftFamily`), Cortana Win 10 (`Microsoft.549981C3F5F10`) |
| Triệt để | + Xbox (`Microsoft.GamingApp`, `Microsoft.XboxApp`), Game Bar (`Microsoft.XboxGamingOverlay`, `Microsoft.XboxGameOverlay`, `Microsoft.XboxSpeechToTextOverlay` — ⚠ mất Win+G quay màn hình), Phone Link (`Microsoft.YourPhone`), Outlook mới (`Microsoft.OutlookForWindows`), Mail & Calendar (`microsoft.windowscommunicationsapps`) |

Chỉ app **đang có trên máy** mới hiện trong danh sách.

### 4.2 Quyền riêng tư & quảng cáo

| Mức | id | Mục | Cơ chế chính |
|---|---|---|---|
| Cơ bản | `ads_id` | Tắt ID quảng cáo | `HKCU\...\CurrentVersion\AdvertisingInfo` `Enabled=0` |
| Cơ bản | `ads_start_suggestions` | Tắt gợi ý/quảng cáo Start, Settings, màn hình khóa, tự cài app quảng cáo | `HKCU\...\ContentDeliveryManager`: `SystemPaneSuggestionsEnabled`, `SubscribedContent-338388Enabled`, `-338389Enabled`, `-338387Enabled`, `-353694Enabled`, `-353696Enabled`, `RotatingLockScreenOverlayEnabled`, `SilentInstalledAppsEnabled` = 0 |
| Cơ bản | `ads_explorer` | Tắt quảng cáo trong File Explorer | `HKCU\...\Explorer\Advanced` `ShowSyncProviderNotifications=0` (explorer) |
| Cơ bản | `tailored_experiences` | Tắt trải nghiệm cá nhân hóa theo dữ liệu chẩn đoán | `HKCU\...\CurrentVersion\Privacy` `TailoredExperiencesWithDiagnosticDataEnabled=0` |
| Cơ bản | `scoobe` | Tắt màn "Hãy hoàn tất thiết lập thiết bị" | `HKCU\...\CurrentVersion\UserProfileEngagement` `ScoobeSystemSettingEnabled=0` |
| Khuyến nghị | `telemetry_min` | Hạ telemetry xuống mức thấp nhất edition cho phép | `HKLM\SOFTWARE\Policies\Microsoft\Windows\DataCollection` `AllowTelemetry` = 0 (Enterprise/Education) hoặc 1 (Home/Pro — ghi rõ "Bắt buộc", không phải "Tắt") — hai mục riêng theo `editions` |
| Khuyến nghị | `svc_diagtrack` | Tắt dịch vụ Connected User Experiences and Telemetry | `service_startup DiagTrack disabled` (caution) |
| Khuyến nghị | `search_no_bing` | Bỏ kết quả Bing khỏi ô tìm kiếm Start | `HKCU\Software\Policies\Microsoft\Windows\Explorer` `DisableSearchBoxSuggestions=1` (explorer) |
| Khuyến nghị | `widgets_feed` | Tắt bảng tin Widgets / Tin tức & sở thích | Win 11: `HKLM\SOFTWARE\Policies\Microsoft\Dsh` `AllowNewsAndInterests=0`; Win 10: `HKLM\SOFTWARE\Policies\Microsoft\Windows\Windows Feeds` `EnableFeeds=0` — hai mục riêng theo build |
| Khuyến nghị | `activity_history` | Tắt lịch sử hoạt động | `HKLM\SOFTWARE\Policies\Microsoft\Windows\System` `PublishUserActivities=0`, `UploadUserActivities=0` |
| Khuyến nghị | `inking_typing` | Tắt gửi mẫu gõ phím và viết tay | `HKCU\Software\Microsoft\Input\TIPC` `Enabled=0`; `HKCU\Software\Microsoft\InputPersonalization` `RestrictImplicitInkCollection=1`, `RestrictImplicitTextCollection=1` |
| Khuyến nghị | `ceip_tasks` | Tắt tác vụ Chương trình Cải thiện Trải nghiệm | task `\Microsoft\Windows\Customer Experience Improvement Program\Consolidator`, `\UsbCeip` |
| Khuyến nghị | `do_lan_only` | Delivery Optimization chỉ chia sẻ trong LAN | `HKLM\SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization` `DODownloadMode=1` |
| Khuyến nghị | `recall_off` | Tắt Recall | `HKLM\SOFTWARE\Policies\Microsoft\Windows\WindowsAI` `DisableAIDataAnalysis=1`; `min_build = 26100` |
| Triệt để | `task_appraiser` | Tắt Compatibility Appraiser | task `\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser` (caution) |
| Triệt để | `svc_dmwappush` | Tắt dịch vụ `dmwappushservice` | `service_startup dmwappushservice disabled` (caution) |
| Triệt để | `cloud_clipboard` | Tắt đồng bộ clipboard lên đám mây | `HKLM\SOFTWARE\Policies\Microsoft\Windows\System` `AllowCrossDeviceClipboard=0` |
| Triệt để | `settings_sync` | Tắt đồng bộ cài đặt giữa các máy | `HKLM\SOFTWARE\Policies\Microsoft\Windows\SettingSync` `DisableSettingSync=2`, `DisableSettingSyncUserOverride=1` |

### 4.3 Cố ý không có (và lý do)

| Không làm | Lý do |
|---|---|
| Tắt Defender, Windows Update, SmartScreen, Firewall | Giảm bảo mật, không phải dọn máy |
| Chặn telemetry bằng file `hosts` | Làm hỏng Store và Update |
| Gỡ OneDrive | Dễ mất file chỉ-trên-mây |
| Gỡ phần mềm hãng (McAfee dùng thử, app Dell/HP…) | App Win32, gỡ im lặng rủi ro — bản sau |
| Tắt dịch vụ vị trí toàn hệ thống | Làm hỏng app hợp lệ |
| Gỡ Edge, Store, framework | Có thể phải cài lại Windows |

## 5. Giao diện

```
[ Cơ bản ]  [ Khuyến nghị ]  [ Triệt để ]        mức đang khớp tô đậm; không khớp ⇒ "Tùy chỉnh"
▼ Gỡ app (12 đang có · 3 đã gỡ)
   ☑ Candy Crush            Cơ bản      Chưa gỡ
   ☐ Xbox Game Bar  ⚠        Triệt để    Chưa gỡ
   ✓ Clipchamp              Cơ bản      Đã gỡ       [Cài lại từ Store]
▼ Quyền riêng tư & quảng cáo
   ☑ Tắt ID quảng cáo       Cơ bản      Chưa áp dụng
   ◐ Hạ telemetry           Khuyến nghị Một phần    ⓘ Home/Pro chỉ hạ được xuống "Bắt buộc"
   ─ Tắt Recall             Không hỗ trợ trên máy này (cần Copilot+)
☐ Nâng cao: gỡ cho mọi tài khoản và chặn cài lại ⚠
                                        [Hoàn tác đã chọn]  [Áp dụng 9 thay đổi]
```

- Mở tab ⇒ đọc trạng thái mọi mục (vòng quay; liệt kê app 1–3 giây).
- Bấm mức sẵn ⇒ tích mọi mục có `level` ≤ mức đó, bỏ tích phần còn lại; mục không hỗ trợ/bị quản lý
  không bao giờ được tích.
- Nút **Áp dụng** đếm số thay đổi thật (bỏ qua mục đã ở trạng thái đích).
- **Xác nhận**: có mục `caution` hoặc bật "mọi tài khoản" ⇒ hộp liệt kê các mục đó.
- Đang áp dụng ⇒ khóa mọi nút, tiến độ từng mục; xong ⇒ kết quả ✓ / ⚠ một phần / ✗ lỗi nguyên văn,
  nút khởi động lại Explorer hoặc nhắc khởi động lại máy nếu cần.
- **Hoàn tác đã chọn** ⇒ trả về ảnh chụp; app đã gỡ ⇒ mở trang Store từng app.

## 6. Xử lý lỗi

- Luật chung như v0.1 (băng đỏ / hổ phách, móc lỗi toàn cục, gộp trùng, tối đa 5 dòng, log đầy đủ).
- Mỗi thao tác hỏng độc lập — một khóa bị từ chối quyền không dừng cả lượt.
- File ảnh chụp hỏng/không đọc được ⇒ băng hổ phách "Không đọc được dữ liệu hoàn tác; hoàn tác sẽ
  dùng giá trị mặc định của Windows", đổi tên file hỏng thành `.bak`, không xóa.
- Không tạo được điểm khôi phục ⇒ băng hổ phách, người dùng chọn.

## 7. Kiểm thử

- **Bộ máy** (`cargo test`): mỗi `kind` chạy trên khóa thật riêng `HKCU\Software\WinFreeUpTest\<uuid>`
  (dọn sau mỗi test); dịch vụ/tác vụ/gói app qua lớp giả `SystemOps`. Ca bắt buộc:
  ảnh chụp chỉ ghi lần đầu; hoàn tác ảnh chụp "absent" ⇒ xóa value; trạng thái Một phần;
  không có ảnh chụp ⇒ dùng `default`; file ảnh chụp hỏng ⇒ `.bak` + dùng `default`.
- **Danh mục** (chạy trong CI, danh mục sai ⇒ build hỏng): `id` duy nhất; mọi `id` có chuỗi trong
  `vi.json`; mọi `registry_set` có `default`; mọi `appx_remove` có `store_product_id`;
  **không gói nào khớp danh sách cấm gỡ**; `min_build ≤ max_build` khi `max_build ≠ 0`.
- **Máy trạng thái mức sẵn** (Vitest): bấm mức ⇒ đúng tập tích; nhận diện mức hiện tại hoặc "Tùy chỉnh".
- **Thử tay**: Windows Sandbox (Win 10, Win 11, bản Pro) và máy ảo cho Home; kiểm tra từng mục áp dụng
  ⇒ đọc lại ⇒ hoàn tác ⇒ đọc lại.

## 8. Ngoài phạm vi

- Xuất/nhập cấu hình sang máy khác.
- Tắt dịch vụ Windows ngoài hai dịch vụ trong danh mục.
- Gỡ OneDrive, phần mềm hãng, Edge.
- Cập nhật danh mục qua mạng (danh mục chỉ đổi theo phiên bản exe).
