# WinFreeUp — Dự án con 2 «Khám máy» — Kế hoạch triển khai

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Thêm tab «Khám máy» vào `WinFreeUp.exe`: trong dưới một phút sau khi mở tab, người dùng biết thư mục nào chiếm chỗ nhiều nhất (cây thư mục), app nào ngốn RAM nhất (theo dõi trực tiếp), và danh sách vấn đề của máy kèm gợi ý xử lý (khám nhanh) — xử lý ngay tại chỗ bằng hành động an toàn (xóa vào Thùng rác, tắt app khởi động, kết thúc app).

**Architecture:** Crate Rust mới `crates/winfreeup-diagnose` (không phụ thuộc Tauri, dùng `CancelToken`/`CoreError` của `winfreeup-core`) chia ba khu: `disk` (cây thư mục gọn trong bộ nhớ, `WalkScanner` duyệt song song, `MftScanner` đọc thẳng MFT, luật bảo vệ, Thùng rác), `perf` (ảnh chụp tiến trình bằng `NtQuerySystemInformation`, bộ đếm PDH, ETW mạng theo app, WMI nhiệt độ, hàm thuần phát hiện cơn giật/hạ xung), `health` (luật khám nhanh thuần `evaluate()`, thu số liệu, danh sách khởi động qua StartupApproved). Mỗi khu có một «dịch vụ» trả lỗi dạng chuỗi để vỏ Tauri chỉ cần nối lệnh. Giao diện nằm trọn trong `src/features/kham-may/` (React + Fluent UI v9, biểu đồ SVG tự vẽ, không thêm gói npm), gọi lõi qua interface `KhamMayApi` để Vitest thử bằng bản giả. Mọi file dùng chung với v0.1 chỉ bị sửa ở **một** task Tích hợp cuối cùng.

**Tech Stack:** Rust 1.90 (edition 2021), windows-sys 0.61.2, windows 0.61.3 (COM: `IFileOperation`, `SHGetFileInfo`), wmi 0.16.0 (ghim, xem Global Constraints), ferrisetw 1.2.0 (ETW), png 0.18.1, serde, chrono; React 19.3, TypeScript 5.9, Fluent UI React v9 (9.74), Vitest 5 + jsdom + Testing Library; Tauri 2.11 (chỉ ở Task 20).

**Spec:** `docs/superpowers/specs/2026-09-25-winfreeup-kham-may-design.md` (đọc kèm kế hoạch v0.1 `docs/superpowers/plans/2026-09-25-winfreeup-don-o-dia.md` — kiểu lõi, luật lỗi, vỏ Tauri đều lấy từ đó).

## Global Constraints

Giá trị chép nguyên văn từ spec; mọi task ngầm bao gồm mục này.

- Kế thừa v0.1: «Một file `.exe`, luôn chạy quyền Admin (`requireAdministrator`), Windows 10 1903+ / 11, 64-bit.» «Tauri 2 + React + TypeScript + Fluent UI React v9; lõi Rust `winfreeup-core`.» «Tiếng Việt; chuỗi hiển thị trong `src/i18n/vi.json`» — *tạm* nằm ở `src/features/kham-may/vi.json` cho tới Task 20 (xem «Đợt thi công»). «Cùng luật lỗi (băng đỏ/hổ phách, móc lỗi toàn cục), cùng nhật ký `%LOCALAPPDATA%\WinFreeUp\logs\`.»
- «**Không cài driver nhân** dưới bất kỳ hình thức nào.» «**Không** dùng driver (WinRing0 và tương tự nằm trong danh sách driver có lỗ hổng bị Microsoft chặn).»
- Lõi không chứa chữ hiển thị: lõi chỉ trả mã (`busy`, `protected`, `essential`, `no_sensor`…) và thông điệp lỗi hệ thống nguyên văn; giao diện dịch.
- Ổ đĩa: «ổ NTFS cục bộ ⇒ `MftScanner`; ổ khác (FAT32, exFAT, USB, mạng) hoặc MFT lỗi ⇒ `WalkScanner`, kèm băng hổ phách "Đang dùng chế độ quét chậm vì <lý do nguyên văn>"». «Dung lượng = dung lượng thực chiếm trên đĩa (allocated size)… **Hard link đếm một lần.** Không đi theo reparse point.» «Thư mục không đọc được vẫn là một nút, gắn cờ `unreadable`, hiển thị "Không đọc được".» «`tree_children(node_id)` → tối đa **200 con lớn nhất** + một dòng tổng "(+N mục nhỏ khác, X GB)".» Lệnh: «`disk_volumes()`», «`disk_scan(volume)` + sự kiện `disk-scan-progress`», «`disk_scan_cancel()`».
- Xóa: «`IFileOperation` với `FOFX_RECYCLEONDELETE`», hộp xác nhận «ghi tên, dung lượng, số file», «Xong thì trừ dung lượng khỏi cây ngay (không quét lại), ghi nhật ký.» Danh sách bảo vệ «so sau khi `canonicalize`»: «thư mục gốc mọi ổ, `%WINDIR%`, `Program Files`, `Program Files (x86)`, `ProgramData`, `%USERPROFILE%` và `C:\Users`, `pagefile.sys`, `hiberfil.sys`, `swapfile.sys`, `System Volume Information`, `$Recycle.Bin`» — «con bên trong `%USERPROFILE%` (vd `Downloads\abc`) vẫn xóa được, con bên trong các thư mục hệ thống còn lại thì không». `hiberfil.sys`: «chỉ là chữ, không tự tắt».
- Khám nhanh: «Quy tắc là **hàm thuần** `evaluate(metrics: &HealthMetrics) -> Vec<Finding>`». Ngưỡng: `disk_full` «🟠 trống < 15% · 🔴 trống < 5%»; `disk_health` «🔴 khi `MSFT_PhysicalDisk.HealthStatus` ≠ Healthy hoặc SMART báo sắp hỏng»; `system_hdd` «🟠 (`MediaType` = HDD)»; `ram_pressure` «🟠 dùng > 85% · 🔴 commit charge > 90% giới hạn»; `startup_apps` «🟠 > 8 app đang bật»; `uptime` «🟠 > 7 ngày»; `power_saver` «Tiết kiệm pin khi đang cắm sạc»; `cpu_throttle` «lấy mẫu 10 giây lúc khám»; `cpu_hot` «🟠 > 90 °C, chỉ khi đọc được nhiệt độ». «Nguồn số liệu nào không đọc được ⇒ dòng đó hiện "Không đo được: <lý do>", không phải 🟢.»
- Khởi động: «nguồn `HKCU/HKLM\...\Run`, thư mục Startup của người dùng và chung. Công tắc bật/tắt ghi vào `...\Explorer\StartupApproved\{Run,Run32,StartupFolder}` — đúng cơ chế Task Manager, bật lại được. Mỗi lần đổi ghi nhật ký.»
- Hiệu năng: «Chỉ lấy mẫu khi mục này đang hiển thị; rời mục ⇒ dừng. Lấy mẫu **mỗi 1 giây**, giữ **5 phút** gần nhất. Chi phí của chính WinFreeUp mục tiêu < 2% CPU; vượt ⇒ giãn chu kỳ còn 2 giây.» Bốn biểu đồ «**CPU**, **RAM**, **Hoạt động đĩa** (% active time), **Mạng** (tải lên / tải xuống)». Bảng app: «Gộp tiến trình theo app (theo đường dẫn exe; tên thân thiện lấy từ `FileDescription`, kèm biểu tượng)», cột «Tên · RAM (private working set) · CPU % · Đĩa (MB/s) · Mạng (KB/s)», «mặc định RAM». Chặn kết thúc: «`System`, `Registry`, `smss`, `csrss`, `wininit`, `winlogon`, `services`, `lsass`, `dwm`, `svchost`, `fontdrvhost`, và chính WinFreeUp. Chặn theo tên **và** đường dẫn nằm trong `%WINDIR%\System32`».
- Cơn giật: «CPU **hoặc** Hoạt động đĩa > 90% trong **≥ 3 mẫu liên tiếp**», «Hai cơn cách nhau < 2 giây ⇒ gộp làm một», mỗi cơn «giờ bắt đầu, thời lượng, chỉ số nào vượt, **3 app ngốn nhất**… cờ "trùng lúc hạ xung"». Hạ xung: «`\Processor Information(_Total)\% Processor Performance`», «**CPU tải > 80% và hiệu năng < 70% liên tục ≥ 10 giây**», «Bộ đếm không có ⇒ ẩn chỉ báo, ghi "Không đo được"». Nhiệt độ: «`MSAcpi_ThermalZoneTemperature` (WMI `root\wmi`)… phần mười Kelvin», «ngoài khoảng 20–110 °C, hoặc đứng yên suốt 60 giây ⇒… "Máy này không cho đọc nhiệt độ"». Mạng: «`GetIfTable2`», ETW «provider `Microsoft-Windows-Kernel-Network`… Phiên đặt tên riêng `WinFreeUp-Net`; khởi động thấy phiên cùng tên còn sót ⇒ dừng rồi tạo lại; đóng app ⇒ dừng phiên», «ETW không khởi động được ⇒ cột Mạng hiện "–", băng hổ phách nêu lý do; biểu đồ tổng vẫn chạy».
- Lỗi: «**Mỗi nguồn số liệu hỏng độc lập**… Không bao giờ để trống cả tab.» «Mọi thao tác chờ (quét ổ, khám nhanh, mở tầng cây, xóa, kết thúc app) có chỉ báo động; tải lại một phần thì phủ mờ, không xoá trắng; chặn thao tác chồng (đang xóa thì khóa nút xóa khác).»
- **Phiên bản đo trên máy dev (2026-09-25):** rustc/cargo 1.90.0 (`x86_64-pc-windows-msvc`), Node v22.23.2, npm 10.9.x. Crate: windows-sys 0.61.2, windows 0.61.3, wmi 0.16.0, ferrisetw 1.2.0, png 0.18.1, tempfile 3.27, junction 2.1. Không thêm gói npm nào.
- **Ghim `wmi = "=0.16.0"` và `windows = "0.61"`** (cùng bản `windows` mà Tauri 2.11 dùng). Đã đo: với `wmi 0.18.4` (khai `windows`/`windows-core` dạng khoảng `">=0.59, <0.63"`), khi gộp vào workspace có Tauri, cargo ghép `windows 0.61.3` với `windows-core 0.62.2` ⇒ `wmi` không biên dịch (`IWbemObjectSink: windows_core::Interface is not satisfied`).
- Crate `winfreeup-diagnose` là **workspace riêng** (bảng `[workspace]` rỗng trong `Cargo.toml` của nó, có `Cargo.lock` riêng) từ Task 1 tới Task 19: chạy `cargo test`/`cargo clippy` **bên trong** `crates/winfreeup-diagnose`. Task 20 xóa bảng đó và thêm crate vào `members` gốc; từ đó dùng `cargo test -p winfreeup-diagnose` ở gốc.
- Test chỉ **đọc** hệ thống thật (danh sách ổ, tiến trình, bộ đếm, registry khởi động); ghi thì chỉ vào thư mục tạm, tiến trình con do chính test tạo, hoặc bản giả trong bộ nhớ. Hai test chạm hệ thống bị `#[ignore]` và chạy tay: Thùng rác thật (`recycle_moves`), VHD cần Admin (`mft_matches_walk`).
- Git: tiền tố `rtk`; `git add` **từng tệp một**; mọi commit kết thúc bằng dòng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; mỗi task một worktree + nhánh `task/km-<số>-<tên-ngắn>` (tiền tố `km-` để không trùng tên nhánh `task/<số>-…` của v0.1) tách từ `feat/kham-may`; không push, không merge vào `main` khi chưa hỏi người dùng.
- Luật chung của người dùng không áp dụng (ghi để người rà khỏi hỏi): bộ lọc trong URL (ứng dụng máy bàn, không có địa chỉ để gửi nhau), đánh dấu T7/CN (biểu đồ chỉ có trục 5 phút, không có trục ngày), tên người/ID. Luật **vòng quay mọi thao tác chờ**, **lỗi lên màn** và **reduced-motion đổi sang nhịp mờ tỏ** thì áp dụng đầy đủ.

## Review Focus

1. Ổ FAT32/exFAT/USB/ổ mạng có thể trả `FileId = 0` cho mọi file: nếu gộp hard link theo mã file thì cả ổ chỉ còn một file — người dùng mong thấy đủ file và đủ dung lượng. (Test: Task 13 `file_id_zero_is_never_treated_as_a_hard_link`.)
2. Ổ NTFS cụm nhỏ (512 byte, thẻ nhớ/ổ cũ) làm một bản ghi MFT 1 KiB nằm vắt qua hai đoạn của `$MFT`: đọc theo từng đoạn sẽ mất bản ghi và lệch mọi bản ghi sau đó — người dùng mong cây đầy đủ. (Test: Task 17 `image_tree_matches_the_script_that_built_it` — ảnh `ntfs-testfs1.img` có đúng tình huống này: đoạn đầu 511 cụm, thiếu thì chỉ thấy 500/512 thư mục.)
3. React StrictMode (bản dev) chạy effect hai lần: khám nhanh gọi lõi hai lần ⇒ lần hai bị «busy» thành băng đỏ; `perfStart → perfStop → perfStart` chạy chồng ⇒ lắng nghe cũ không được gỡ, mỗi mẫu tới hai lần. Người dùng mong một lần khám, không băng đỏ vô cớ. (Test: Task 11 `khám đúng một lần kể cả StrictMode`, Task 2 `start → stop → start … chạy lần lượt`.)
4. Đang ở mục Bộ nhớ & Hiệu năng rồi bấm sang tab «Dọn dẹp» của App (tab Khám máy vẫn gắn, chỉ bị ẩn): lấy mẫu phải dừng, quay lại thì chạy tiếp. (Test: Task 16 `App chuyển sang tab khác (active = false) thì cũng dừng lấy mẫu`; Task 20 nối `active={tab === 'diagnose'}`.)
5. Đường dẫn «lách» danh sách bảo vệ: junction trong `Downloads` trỏ vào `C:\Windows\System32`, đường có `..` (`C:\Users\an\Downloads\..\..\..\Windows`), tiền tố `\\?\` — phải bị chặn xóa, kể cả khi giao diện (bị sửa) vẫn gửi lệnh xóa. (Test: Task 4 `junction_into_a_protected_dir_is_blocked_after_canonicalize`, `dot_dot_and_verbatim_prefix_cannot_sneak_past`; Task 19 `protected_nodes_are_refused_even_if_the_ui_asks`.)

---

## File Structure

```
crates/winfreeup-diagnose/                 crate MỚI (workspace riêng tới Task 20)
├─ Cargo.toml, Cargo.lock                  ĐỦ mọi phụ thuộc ngay từ đầu (Task 1); Task 20 bỏ [workspace], xóa Cargo.lock riêng
├─ testdata/ntfs-testfs1.img, create-testfs1.sh, README.md   ảnh NTFS 2 MiB để test MFT không cần Admin (Task 17)
└─ src/
   ├─ lib.rs                                khai báo SẴN mọi module (Task 1) — task sau không sửa
   ├─ util.rs                               wide, from_wide, filetime_to_unix, reveal_in_explorer (Task 1)
   ├─ actlog.rs                             ActionLog — nhật ký hành động, tạo file lúc ghi dòng đầu (Task 1)
   ├─ wmiq.rs                               connect, query, query_on (Task 8)
   ├─ app.rs                                KhamMay — gom 3 dịch vụ cho vỏ Tauri (Task 19)
   ├─ disk/
   │  ├─ mod.rs                             khai báo SẴN (Task 1)
   │  ├─ tree.rs                            TreeBuilder, DiskTree, NodeView, ChildrenPage, MAX_CHILDREN = 200 (Task 3)
   │  ├─ protect.rs                         ProtectRules (Task 4)
   │  ├─ volumes.rs                         VolumeInfo, list_volumes, plan_for, ScanPlan, WalkReason (Task 5)
   │  ├─ recycle.rs                         recycle (IFileOperation), recycle_flags (Task 5)
   │  ├─ scan.rs                            DiskScanner, ScanProgress, ScanStatus, Throttle (Task 13)
   │  ├─ walk.rs                            WalkScanner, read_dir_entries, is_link_tag (Task 13)
   │  ├─ mft.rs                             MftScanner, scan_ntfs, parse_boot, apply_fixups, parse_runlist, parse_record (Task 17)
   │  └─ service.rs                         DiskService, DiskOps, RealDiskOps (Task 19)
   ├─ perf/
   │  ├─ mod.rs                             khai báo SẴN (Task 1)
   │  ├─ detect.rs                          find_stutters, find_throttle, throttled_now, top_apps, TempTracker, next_interval_ms (Task 6)
   │  ├─ procs.rs                           snapshot (NtQuerySystemInformation), exe_path, PathCache, kill, own_cpu_100ns (Task 7)
   │  ├─ apps.rs                            EssentialRules, aggregate, AppRow, ProcRow, ProcSample (Task 7)
   │  ├─ icon.rs                            file_description, icon_data_url, base64 (Task 7)
   │  ├─ counters.rs                        CpuDiskCounters (PDH), memory, net_totals (Task 8)
   │  ├─ thermal.rs                         ThermalReader, decikelvin_to_celsius (Task 8)
   │  ├─ netetw.rs                          NetTrace (ETW WinFreeUp-Net), direction, stop_stale_session (Task 8)
   │  ├─ monitor.rs                         Sampler, History, PerfMonitor, PerfTick, Sample (Task 14)
   │  └─ service.rs                         PerfService, Killer, KillResult (Task 14)
   └─ health/
      ├─ mod.rs                             khai báo SẴN (Task 1)
      ├─ startup.rs                         Startup, Registry, WinRegistry, StartupEntry (Task 9)
      ├─ rules.rs                           HealthMetrics, Finding, Level, evaluate + từng luật (Task 15)
      ├─ collect.rs                         collect_streaming, system_media, disk_health, power, throttle_points (Task 18)
      └─ service.rs                         HealthService, settings_uri (Task 18)
src/features/kham-may/                      giao diện MỚI
├─ vi.json, i18n.ts, i18n.test.ts           chuỗi của tab (Task 2) — Task 20 trộn vào src/i18n/vi.json rồi xóa vi.json, i18n.test.ts
├─ api/types.ts, api/tauri.ts, api/tauri.test.ts   kiểu + cầu nối Tauri (Task 2)
├─ fmt.ts, fmt.test.ts, ui/Spin.tsx, km.css, testing/fakeApi.ts   (Task 2)
├─ disk/treeModel.ts, treeModel.test.ts, DiskView.tsx, DiskView.test.tsx   (Task 10)
├─ overview/findingText.ts, findingText.test.ts, StartupList.tsx, OverviewView.tsx, OverviewView.test.tsx   (Task 11)
├─ perf/perfModel.ts, perfModel.test.tsx, LineChart.tsx, AppTable.tsx, StutterList.tsx, PerfView.tsx, PerfView.test.tsx   (Task 12)
└─ KhamMayTab.tsx, KhamMayTab.test.tsx      (Task 16)
File dùng chung với v0.1 — CHỈ Task 20 sửa: Cargo.toml (gốc), Cargo.lock, src-tauri/Cargo.toml, src-tauri/src/main.rs,
  src-tauri/src/kham_may.rs (mới), src-tauri/tauri.conf.json, src/i18n/vi.json, src/App.tsx, src/App.test.tsx, src/styles.css
CHỈ Task 21 sửa: .github/workflows/ci.yml; tạo docs/thu-tay-kham-may.md
```

Nguyên tắc chia: mỗi file một trách nhiệm; mọi lời gọi chạm hệ thống nằm sau một trait/hàm có bản giả (`DiskOps`, `Registry`, `Killer`) hoặc chỉ đọc; logic quyết định (luật khám, cơn giật, hạ xung, nhiệt độ, gộp app, luật bảo vệ, cây) là hàm thuần. Như v0.1, `lib.rs` và ba `mod.rs` khai báo **sẵn** mọi module ở Task 1; module chưa tới lượt là **file khung** một dòng `//!`, task chủ sở hữu thay toàn bộ dòng đó.

## Đợt thi công

Theo `CLAUDE.md` mục «Quy trình thi công». Nhánh tính năng **`feat/kham-may`** tách từ `feat/v0.1-don-o-dia` **sau Đợt 0 của v0.1** và chạy **song song** với các đợt còn lại của v0.1: vì vậy Task 1–19, 21 không chạm file nào của v0.1; mọi chỗ nối vào v0.1 dồn vào **Task 20**, chỉ mở khi v0.1 đã xong (Task 18 của v0.1 gộp vào `feat/v0.1-don-o-dia`). Mỗi task một worktree + nhánh `task/km-<số>-<tên-ngắn>` tách từ `feat/kham-may`; hết mỗi đợt gộp vào `feat/kham-may` rồi chạy **toàn bộ** `cargo test` (trong `crates/winfreeup-diagnose`) + `npm test`; mỗi task một lượt rà riêng.

| Đợt | Task (song song trong đợt) | Chờ | File — ai sở hữu |
|---|---|---|---|
| K0 | **Task 1** rồi **Task 2** — một đội, tuần tự | Đợt 0 của v0.1 | Task 1: `crates/winfreeup-diagnose/Cargo.toml`, `Cargo.lock`, `lib.rs`, ba `mod.rs`, mọi file khung. Task 2: mọi file khung của `src/features/kham-may/` (`vi.json`, `api/types.ts`, `testing/fakeApi.ts`…) |
| K1 | Task 3 (tree) · 4 (protect) · 5 (volumes, recycle) · 6 (detect) · 7 (procs, apps, icon) · 8 (counters, wmiq, thermal, netetw) · 9 (startup) · 10 (UI Ổ đĩa) · 11 (UI Tổng quan) · 12 (UI Hiệu năng) | K0 | mỗi task chỉ sửa file riêng |
| K2 | Task 13 (scan, walk) · 14 (monitor, perf/service) · 15 (rules) · 16 (KhamMayTab) | 13 ← 3; 14 ← 6, 7, 8; 15 ← 6, 8; 16 ← 10, 11, 12 | không chung file |
| K3 | Task 17 (mft) · 18 (collect, health/service) | 17 ← 13; 18 ← 8, 9, 15 | không chung file |
| K4 | Task 19 (disk/service, app) | 3, 4, 5, 13, 14, 17, 18 | — |
| K5 | Task 20 **Tích hợp** | K4 **và** v0.1 xong Task 18 | task DUY NHẤT sửa file của v0.1 |
| K6 | Task 21 (CI + thử tay) | Task 20 | `.github/workflows/ci.yml` |

Đường găng: 1 → 3 → 13 → 17 → 19 → 20 → 21. Đã kiểm khi viết kế hoạch: dựng worktree giả cho từng task (các đợt trước + riêng task đó, module còn lại là file khung) — cả 19 task Rust/giao diện đều biên dịch, qua test và clippy độc lập.

Vì test đếm theo bộ lọc, mỗi task báo số test **của riêng nó** và yêu cầu toàn bộ bộ test «0 failed»; tổng chỉ kiểm ở bước gộp đợt.

## Chuẩn bị (một lần, trước Đợt K0)

- [ ] Tạo nhánh tính năng từ `feat/v0.1-don-o-dia` ngay sau khi Đợt 0 của v0.1 đã gộp (có `crates/winfreeup-core` với `CancelToken`, `CoreError`, `Result`, `error::io_err`; có `src/format.ts` với `formatBytes`; có `package.json` đủ gói):

```bash
cd D:/META/WinFreeUp
rtk git branch feat/kham-may feat/v0.1-don-o-dia
```

Expected: nhánh `feat/kham-may` trỏ vào cùng commit với `feat/v0.1-don-o-dia` lúc đó.

- [ ] Mỗi task: dựng worktree riêng bằng skill `superpowers:using-git-worktrees`, nhánh `task/km-<số>-<tên-ngắn>` tách từ `feat/kham-may` (vd `task/km-03-cay`). Worktree mới cần `npm ci` một lần trước khi chạy Vitest. Lệnh Rust của Task 1–19 chạy trong `crates/winfreeup-diagnose` của worktree đó.

---

### Task 1: Khung crate `winfreeup-diagnose`, tiện ích, nhật ký hành động

**Files:**
- Create: `crates/winfreeup-diagnose/Cargo.toml` (và `Cargo.lock` sinh ra khi build)
- Create: `crates/winfreeup-diagnose/src/lib.rs`, `src/disk/mod.rs`, `src/health/mod.rs`, `src/perf/mod.rs`
- Create: `crates/winfreeup-diagnose/src/util.rs`, `src/actlog.rs`
- Create (file khung, một dòng `//!`): các file còn lại trong bảng ở Step 1
- Test: `util.rs`, `actlog.rs` (module `tests`)

**Interfaces:**
- Consumes (Đợt 0 của v0.1): crate `winfreeup-core` qua đường dẫn `../winfreeup-core` — các task sau dùng `winfreeup_core::{CancelToken, CoreError, Result}` và `winfreeup_core::error::io_err(path: &Path, source: std::io::Error) -> CoreError`.
- Produces:
  - `util::wide(s: impl AsRef<OsStr>) -> Vec<u16>` (UTF-16 kết thúc 0), `util::from_wide(buf: &[u16]) -> String`, `util::filetime_to_unix(ft: i64) -> u32`, `util::reveal_in_explorer(path: &Path) -> Result<(), String>`.
  - `actlog::log_dir(local_appdata: &Path) -> PathBuf` (= `…\WinFreeUp\logs`), `actlog::ActionLog` + `new(dir: PathBuf)`, `line(&self, text: &str) -> Result<(), String>` (tạo file `YYYY-MM-DD_HHmmss.log` ở dòng đầu), `path(&self) -> Option<PathBuf>`.
  - `lib.rs` khai báo sẵn `actlog, app, disk, health, perf, util, wmiq`; `disk/mod.rs`: `mft, protect, recycle, scan, service, tree, volumes, walk`; `health/mod.rs`: `collect, rules, service, startup`; `perf/mod.rs`: `apps, counters, detect, icon, monitor, netetw, procs, service, thermal`. Không task nào sau này sửa bốn file này (trừ Task 20 sửa `Cargo.toml`).

- [ ] **Step 1: Tạo crate và các file khung**

`crates/winfreeup-diagnose/Cargo.toml`:

```toml
[package]
name = "winfreeup-diagnose"
version = "0.1.0"
edition = "2021"

# Tạm là workspace riêng để build/test độc lập khi nhánh v0.1 chưa gộp xong.
# Task Tích hợp xoá bảng này và thêm crate vào `members` của workspace gốc.
[workspace]

[dependencies]
winfreeup-core = { path = "../winfreeup-core" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", default-features = false, features = ["clock", "std"] }
png = "0.18"

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.61", features = [
  "Wdk_System_SystemInformation",
  "Win32_Foundation",
  "Win32_Graphics_Gdi",
  "Win32_NetworkManagement_IpHelper",
  "Win32_NetworkManagement_Ndis",
  "Win32_Networking_WinSock",
  "Win32_Security",
  "Win32_Storage_FileSystem",
  "Win32_System_IO",
  "Win32_System_Ioctl",
  "Win32_System_Performance",
  "Win32_System_Power",
  "Win32_System_Registry",
  "Win32_System_SystemInformation",
  "Win32_System_Threading",
  "Win32_UI_Shell",
  "Win32_UI_WindowsAndMessaging",
] }
windows = { version = "0.61", features = [
  "Win32_Foundation",
  "Win32_System_Com",
  "Win32_UI_Shell",
  "Win32_UI_Shell_Common",
] }
# Ghim 0.16.0: bản này đòi đúng cặp windows/windows-core ^0.61 như Tauri 2.11. wmi 0.18 khai khoảng
# ">=0.59, <0.63" nên khi gộp workspace cargo có thể ghép windows 0.61 với windows-core 0.62 ⇒ không biên dịch.
wmi = "=0.16.0"
ferrisetw = "1.2"

[dev-dependencies]
tempfile = "3"
junction = "2"
```

`crates/winfreeup-diagnose/src/lib.rs`:

```rust
//! Khám máy: cây thư mục, khám nhanh, theo dõi hiệu năng. Không phụ thuộc Tauri, không chứa chữ hiển thị.

pub mod actlog;
pub mod app;
pub mod disk;
pub mod health;
pub mod perf;
pub mod util;
pub mod wmiq;
```

`crates/winfreeup-diagnose/src/disk/mod.rs`:

```rust
//! Ổ đĩa: quét cây thư mục (WalkScanner, MftScanner), luật bảo vệ, xóa vào Thùng rác.
pub mod mft;
pub mod protect;
pub mod recycle;
pub mod scan;
pub mod service;
pub mod tree;
pub mod volumes;
pub mod walk;
```

`crates/winfreeup-diagnose/src/health/mod.rs`:

```rust
//! Tổng quan: thu số liệu, luật đánh giá thuần, danh sách khởi động.
pub mod collect;
pub mod rules;
pub mod service;
pub mod startup;
```

`crates/winfreeup-diagnose/src/perf/mod.rs`:

```rust
//! Bộ nhớ & Hiệu năng: lấy mẫu, bảng ứng dụng, cơn giật, hạ xung, nhiệt độ, mạng theo app.
pub mod apps;
pub mod counters;
pub mod detect;
pub mod icon;
pub mod monitor;
pub mod netetw;
pub mod procs;
pub mod service;
pub mod thermal;
```

File khung — mỗi file đúng MỘT dòng, task chủ sở hữu thay toàn bộ (đường dẫn tính từ `crates/winfreeup-diagnose/src/`):

| File | Nội dung dòng khung |
|---|---|
| `disk/tree.rs` | `//! khung — nội dung ở Task 3.` |
| `disk/protect.rs` | `//! khung — nội dung ở Task 4.` |
| `disk/volumes.rs`, `disk/recycle.rs` | `//! khung — nội dung ở Task 5.` |
| `perf/detect.rs` | `//! khung — nội dung ở Task 6.` |
| `perf/procs.rs`, `perf/apps.rs`, `perf/icon.rs` | `//! khung — nội dung ở Task 7.` |
| `perf/counters.rs`, `wmiq.rs`, `perf/thermal.rs`, `perf/netetw.rs` | `//! khung — nội dung ở Task 8.` |
| `health/startup.rs` | `//! khung — nội dung ở Task 9.` |
| `disk/scan.rs`, `disk/walk.rs` | `//! khung — nội dung ở Task 13.` |
| `perf/monitor.rs`, `perf/service.rs` | `//! khung — nội dung ở Task 14.` |
| `health/rules.rs` | `//! khung — nội dung ở Task 15.` |
| `disk/mft.rs` | `//! khung — nội dung ở Task 17.` |
| `health/collect.rs`, `health/service.rs` | `//! khung — nội dung ở Task 18.` |
| `disk/service.rs`, `app.rs` | `//! khung — nội dung ở Task 19.` |

`.gitignore` gốc đã có dòng `target/` (khớp cả `crates/winfreeup-diagnose/target/`) — không sửa.

- [ ] **Step 2: Viết test hỏng**

`crates/winfreeup-diagnose/src/util.rs` (chỉ phần test; mã viết ở Step 4):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_round_trips_vietnamese() {
        let w = wide("Tài liệu");
        assert_eq!(*w.last().unwrap(), 0);
        assert_eq!(from_wide(&w), "Tài liệu");
    }

    #[test]
    fn filetime_converts_to_unix_seconds() {
        assert_eq!(filetime_to_unix(EPOCH_DIFF_100NS + 10_000_000), 1);
        assert_eq!(filetime_to_unix(0), 0);
        // 2026-09-25 00:00:00 UTC
        assert_eq!(filetime_to_unix(EPOCH_DIFF_100NS + 1_790_294_400 * 10_000_000), 1_790_294_400);
    }
}
```

`crates/winfreeup-diagnose/src/actlog.rs` (chỉ phần test):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_dir_follows_v01_layout() {
        assert_eq!(log_dir(Path::new(r"C:\Users\a\AppData\Local")), PathBuf::from(r"C:\Users\a\AppData\Local\WinFreeUp\logs"));
    }

    #[test]
    fn file_is_created_lazily_and_lines_append() {
        let t = tempfile::tempdir().unwrap();
        let log = ActionLog::new(t.path().join("WinFreeUp").join("logs"));
        assert!(log.path().is_none());
        log.line(r"DISK_RECYCLE bytes=10 files=1 C:\Users\a\Tải về\x.iso").unwrap();
        log.line("STARTUP_DISABLE hkcu_run OneDrive").unwrap();
        let path = log.path().unwrap();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert_eq!(name.len(), "2026-09-25_101010.log".len());
        let text = fs::read_to_string(path).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.contains(r"Tải về\x.iso"));
    }
}
```

- [ ] **Step 3: Chạy test, thấy hỏng**

Run (trong `crates/winfreeup-diagnose`): `cargo test`
Expected: FAIL biên dịch — `cannot find function wide`, `cannot find type ActionLog`. Lần build đầu tải wmi, ferrisetw, windows… mất vài phút.

- [ ] **Step 4: Viết mã (đặt TRÊN khối `#[cfg(test)]`)**

`util.rs`:

```rust
//! Tiện ích nhỏ dùng chung: chuỗi UTF-16 cho Win32, đổi thời gian FILETIME, mở Explorer.
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

/// Chuỗi UTF-16 kết thúc bằng 0 cho hàm Win32 `...W`.
pub fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

/// Đọc chuỗi UTF-16 tới ký tự 0 đầu tiên (hoặc hết mảng).
pub fn from_wide(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// Số khoảng 100 ns từ 1601-01-01 tới 1970-01-01.
const EPOCH_DIFF_100NS: i64 = 116_444_736_000_000_000;

/// FILETIME (100 ns từ 1601) ⇒ giây Unix, kẹp vào khoảng của u32.
pub fn filetime_to_unix(ft: i64) -> u32 {
    if ft <= EPOCH_DIFF_100NS {
        return 0;
    }
    ((ft - EPOCH_DIFF_100NS) / 10_000_000).min(u32::MAX as i64) as u32
}

/// Mở Explorer và chọn sẵn mục (`explorer.exe /select,<path>`). Lỗi là thông điệp nguyên văn.
pub fn reveal_in_explorer(path: &Path) -> Result<(), String> {
    let mut arg = OsString::from("/select,");
    arg.push(path.as_os_str());
    std::process::Command::new("explorer.exe").arg(arg).spawn().map(|_| ()).map_err(|e| format!("explorer.exe: {e}"))
}
```

`actlog.rs`:

```rust
//! Nhật ký hành động của Khám máy (xóa vào Thùng rác, bật/tắt khởi động, kết thúc app):
//! `%LOCALAPPDATA%\WinFreeUp\logs\YYYY-MM-DD_HHmmss.log`, cùng thư mục và dạng tên với nhật ký dọn của v0.1.
//! File chỉ được tạo ở dòng đầu tiên, để mở tab mà không làm gì thì không sinh file rỗng.
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::Local;

pub fn log_dir(local_appdata: &Path) -> PathBuf {
    local_appdata.join("WinFreeUp").join("logs")
}

pub struct ActionLog {
    dir: PathBuf,
    file: Mutex<Option<(PathBuf, File)>>,
}

impl ActionLog {
    pub fn new(dir: PathBuf) -> Self {
        ActionLog { dir, file: Mutex::new(None) }
    }

    /// Ghi một dòng có dấu giờ. Trả `Err(thông điệp)` nếu không ghi được — người gọi báo lên màn.
    pub fn line(&self, text: &str) -> Result<(), String> {
        let mut guard = self.file.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
            let path = self.dir.join(Local::now().format("%Y-%m-%d_%H%M%S.log").to_string());
            let f = OpenOptions::new().create(true).append(true).open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            *guard = Some((path, f));
        }
        let (path, f) = guard.as_mut().expect("vừa mở");
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
        writeln!(f, "{ts} {text}").and_then(|_| f.flush()).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn path(&self) -> Option<PathBuf> {
        self.file.lock().ok().and_then(|g| g.as_ref().map(|(p, _)| p.clone()))
    }
}
```

- [ ] **Step 5: Chạy test và clippy, thấy qua**

Run (trong `crates/winfreeup-diagnose`): `cargo test` rồi `cargo clippy --all-targets -- -D warnings`
Expected: `test result: ok. 4 passed; 0 failed`; clippy không lỗi. Kiểm workspace gốc không bị ảnh hưởng: ở gốc repo `cargo test -p winfreeup-core` ⇒ `0 failed`.

- [ ] **Step 6: Commit**

```bash
rtk git add crates/winfreeup-diagnose/Cargo.toml
rtk git add crates/winfreeup-diagnose/Cargo.lock
rtk git add crates/winfreeup-diagnose/src/lib.rs
rtk git add crates/winfreeup-diagnose/src/util.rs
rtk git add crates/winfreeup-diagnose/src/actlog.rs
rtk git add crates/winfreeup-diagnose/src/wmiq.rs
rtk git add crates/winfreeup-diagnose/src/app.rs
rtk git add crates/winfreeup-diagnose/src/disk/mod.rs
rtk git add crates/winfreeup-diagnose/src/disk/tree.rs
rtk git add crates/winfreeup-diagnose/src/disk/protect.rs
rtk git add crates/winfreeup-diagnose/src/disk/volumes.rs
rtk git add crates/winfreeup-diagnose/src/disk/recycle.rs
rtk git add crates/winfreeup-diagnose/src/disk/scan.rs
rtk git add crates/winfreeup-diagnose/src/disk/walk.rs
rtk git add crates/winfreeup-diagnose/src/disk/mft.rs
rtk git add crates/winfreeup-diagnose/src/disk/service.rs
rtk git add crates/winfreeup-diagnose/src/health/mod.rs
rtk git add crates/winfreeup-diagnose/src/health/startup.rs
rtk git add crates/winfreeup-diagnose/src/health/rules.rs
rtk git add crates/winfreeup-diagnose/src/health/collect.rs
rtk git add crates/winfreeup-diagnose/src/health/service.rs
rtk git add crates/winfreeup-diagnose/src/perf/mod.rs
rtk git add crates/winfreeup-diagnose/src/perf/detect.rs
rtk git add crates/winfreeup-diagnose/src/perf/procs.rs
rtk git add crates/winfreeup-diagnose/src/perf/apps.rs
rtk git add crates/winfreeup-diagnose/src/perf/icon.rs
rtk git add crates/winfreeup-diagnose/src/perf/counters.rs
rtk git add crates/winfreeup-diagnose/src/perf/thermal.rs
rtk git add crates/winfreeup-diagnose/src/perf/netetw.rs
rtk git add crates/winfreeup-diagnose/src/perf/monitor.rs
rtk git add crates/winfreeup-diagnose/src/perf/service.rs
rtk git commit -m "feat(diagnose): khung crate Khám máy, tiện ích Win32, nhật ký hành động

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Khung giao diện tab Khám máy — chuỗi, kiểu dữ liệu, cầu nối Tauri, bản giả

**Files:**
- Create: `src/features/kham-may/vi.json`, `i18n.ts`, `i18n.test.ts`
- Create: `src/features/kham-may/api/types.ts`, `api/tauri.ts`, `api/tauri.test.ts`
- Create: `src/features/kham-may/fmt.ts`, `fmt.test.ts`
- Create: `src/features/kham-may/ui/Spin.tsx`, `km.css`, `testing/fakeApi.ts`

**Interfaces:**
- Consumes (Đợt 0 của v0.1): `formatBytes(n: number): string` từ `src/format.ts`; gói `@fluentui/react-components`, `@tauri-apps/api` đã có trong `package.json`. Không đụng `src/i18n/*`.
- Produces (mọi task giao diện sau dùng nguyên văn):
  - `vi.json`: ĐỦ mọi chuỗi của tab (khoá tiền tố `km.`). Task 10–16 **không** thêm khoá.
  - `i18n.ts`: `tk(key: string, params?: Record<string, string | number>): string` (thiếu khoá ⇒ trả chính khoá + `console.warn`), `hasKey(key: string): boolean`.
  - `api/types.ts`: kiểu khớp serde của crate (`VolumeInfo`, `ScanStatus`, `ScanPlan`, `WalkReason`, `NodeView`, `ChildrenPage`, `ScanSummary`, `DeleteResult`, `Finding`, `FindingId`, `Level`, `StartupEntry`, `StartupList`, `Sample`, `AppRow`, `ProcRow`, `TopApp`, `StutterView`, `TempState`, `PerfTick`, `KillResult`), interface `KhamMayApi` (18 hàm — xem code) và `Notify = (severity: 'error' | 'warning', message: string) => void` (cùng hình dạng `Notify` của v0.1).
  - `api/tauri.ts`: `khamMayTauriApi: KhamMayApi` — tên lệnh/sự kiện Task 20 phải đăng ký đúng: `disk_volumes`, `disk_scan {root}` + `disk-scan-progress`, `disk_scan_cancel`, `tree_children {nodeId}`, `disk_delete {nodeId}`, `disk_reveal {nodeId}`, `health_check` + `health-finding`, `health_throttle`, `startup_list`, `startup_set {id, enabled}`, `open_settings {page}`, `perf_start` + `perf-tick`, `perf-error`, `perf_stop`, `app_kill {key}`, `reveal_path {path}`, `app_icon {path}`. `perfStart`/`perfStop` xếp hàng lần lượt.
  - `fmt.ts`: `formatBytes` (dùng lại), `formatCount`, `formatMBps`, `formatKBps`, `formatRate`, `formatSeconds`, `formatPct`, `formatDate(unixSeconds)` (`dd/MM/yyyy` giờ địa phương), `formatClock(ms)` (`HH:mm:ss`), `friendly(e: unknown): string` (mã lõi ⇒ câu dễ hiểu).
  - `ui/Spin.tsx`: `Spin({ label?, size? })` — Fluent `Spinner` trong `<span class="wfu-busy km-busy">`.
  - `km.css`: mọi lớp `km-*` các task sau dùng, gồm luật `prefers-reduced-motion` đổi quay thành nhịp mờ tỏ.
  - `testing/fakeApi.ts`: `fakeApi(over?: Partial<KhamMayApi>): KhamMayApi` (mọi hàm là `vi.fn`), dữ liệu mẫu `ROOT`, `PAGES` (cây giả: `C:\` ─ Users(1) ─ an(2) ─ Downloads(3) ─ film.mkv(4); Windows(5) 🔒; hiberfil.sys(6) 🔒), `FINDINGS`, `node()`, `sample()`, `app()`, `tick()`, `deferred()`, `GB`.

- [ ] **Step 1: Viết test hỏng**

`src/features/kham-may/api/tauri.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invoke, listen, unlisten } = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn(), unlisten: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

import { khamMayTauriApi as api } from './tauri';

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  unlisten.mockReset();
  listen.mockResolvedValue(unlisten);
});

describe('khamMayTauriApi', () => {
  it('quét ổ: nghe disk-scan-progress trước khi gọi, luôn gỡ kể cả khi lỗi', async () => {
    let handler: (e: { payload: unknown }) => void = () => {};
    listen.mockImplementation(async (_n: string, h: typeof handler) => {
      handler = h;
      return unlisten;
    });
    invoke.mockImplementation(async () => {
      handler({ payload: { files: 1, bytes: 2, current: 'C:\\a', percent: null } });
      throw 'busy';
    });
    const seen: unknown[] = [];
    await expect(api.diskScan('C:\\', (s) => seen.push(s))).rejects.toBe('busy');
    expect(listen).toHaveBeenCalledWith('disk-scan-progress', expect.any(Function));
    expect(invoke).toHaveBeenCalledWith('disk_scan', { root: 'C:\\' });
    expect(seen).toHaveLength(1);
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('khám nhanh nghe health-finding và trả danh sách cuối', async () => {
    invoke.mockResolvedValue([{ id: 'uptime' }]);
    await expect(api.healthCheck(() => {})).resolves.toEqual([{ id: 'uptime' }]);
    expect(listen).toHaveBeenCalledWith('health-finding', expect.any(Function));
    expect(invoke).toHaveBeenCalledWith('health_check');
  });

  it('theo dõi hiệu năng: nghe tới khi dừng, bắt đầu lại thì gỡ lắng nghe cũ', async () => {
    invoke.mockResolvedValue([]);
    await api.perfStart(() => {}, () => {});
    expect(listen.mock.calls.map((c) => c[0])).toEqual(['perf-tick', 'perf-error']);
    expect(unlisten).not.toHaveBeenCalled();
    await api.perfStart(() => {}, () => {});
    expect(unlisten).toHaveBeenCalledTimes(2);
    await api.perfStop();
    expect(unlisten).toHaveBeenCalledTimes(4);
    expect(invoke).toHaveBeenLastCalledWith('perf_stop');
  });

  it('start → stop → start gọi liền nhau (StrictMode) chạy lần lượt, không sót lắng nghe', async () => {
    invoke.mockResolvedValue([]);
    const order: string[] = [];
    listen.mockImplementation(async (name: string) => {
      order.push(`listen:${name}`);
      await new Promise((r) => setTimeout(r, 5));
      return () => order.push(`unlisten:${name}`);
    });
    invoke.mockImplementation(async (cmd: string) => {
      order.push(cmd);
      return [];
    });
    await Promise.all([api.perfStart(() => {}, () => {}), api.perfStop(), api.perfStart(() => {}, () => {})]);
    expect(order).toEqual([
      'listen:perf-tick',
      'listen:perf-error',
      'perf_start',
      'unlisten:perf-tick',
      'unlisten:perf-error',
      'perf_stop',
      'listen:perf-tick',
      'listen:perf-error',
      'perf_start',
    ]);
    await api.perfStop();
  });

  it('bắt đầu theo dõi hỏng thì không để lại lắng nghe', async () => {
    invoke.mockRejectedValue('PDH open: 0xC0000BB8');
    await expect(api.perfStart(() => {}, () => {})).rejects.toBe('PDH open: 0xC0000BB8');
    expect(unlisten).toHaveBeenCalledTimes(2);
  });

  it('các lệnh đơn giản gửi đúng tên và tham số', async () => {
    invoke.mockResolvedValue(null);
    await api.diskVolumes();
    await api.diskScanCancel();
    await api.treeChildren(7);
    await api.diskDelete(8);
    await api.diskReveal(9);
    await api.healthThrottle();
    await api.startupList();
    await api.startupSet('hkcu_run:OneDrive', false);
    await api.openSettings('power');
    await api.appKill('c:\\x.exe');
    await api.revealPath('C:\\x.exe');
    await api.appIcon('C:\\x.exe');
    expect(invoke.mock.calls).toEqual([
      ['disk_volumes'],
      ['disk_scan_cancel'],
      ['tree_children', { nodeId: 7 }],
      ['disk_delete', { nodeId: 8 }],
      ['disk_reveal', { nodeId: 9 }],
      ['health_throttle'],
      ['startup_list'],
      ['startup_set', { id: 'hkcu_run:OneDrive', enabled: false }],
      ['open_settings', { page: 'power' }],
      ['app_kill', { key: 'c:\\x.exe' }],
      ['reveal_path', { path: 'C:\\x.exe' }],
      ['app_icon', { path: 'C:\\x.exe' }],
    ]);
  });
});
```

`src/features/kham-may/fmt.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { formatClock, formatCount, formatDate, formatKBps, formatMBps, formatRate, formatSeconds, friendly } from './fmt';

describe('định dạng', () => {
  it('số và tốc độ theo kiểu Việt Nam', () => {
    expect(formatCount(1234567)).toBe('1.234.567');
    expect(formatMBps(1.5 * 1024 ** 2)).toBe('1,5');
    expect(formatMBps(-3)).toBe('0');
    expect(formatKBps(2048)).toBe('2');
    expect(formatRate(3 * 1024 ** 2)).toBe('3 MB/s');
    expect(formatRate(512 * 1024)).toBe('512 KB/s');
    expect(formatSeconds(4200)).toBe('4,2');
  });

  it('ngày và giờ theo giờ địa phương', () => {
    const local = new Date(2026, 8, 25, 7, 3, 9);
    expect(formatDate(local.getTime() / 1000)).toBe('25/09/2026');
    expect(formatClock(local.getTime())).toBe('07:03:09');
    expect(formatDate(0)).toBe('');
  });

  it('mã lỗi của lõi thành câu dễ hiểu, lỗi khác giữ nguyên văn', () => {
    expect(friendly('busy')).toContain('đang bận');
    expect(friendly('protected')).toContain('không xóa được');
    expect(friendly('essential')).toContain('thiết yếu');
    expect(friendly('unknown_node')).toContain('quét lại');
    expect(friendly(new Error('Access is denied. (os error 5)'))).toBe('Access is denied. (os error 5)');
  });
});
```

`src/features/kham-may/i18n.test.ts`:

```ts
import { describe, expect, it, vi } from 'vitest';
import { hasKey, tk } from './i18n';
import km from './vi.json';

// File này và ./vi.json bị xoá ở Task Tích hợp: khi đó chuỗi nằm trong src/i18n/vi.json và
// test «mọi chuỗi đều khác rỗng» của src/i18n/i18n.test.ts phủ luôn các khoá km.*.
describe('chuỗi tab Khám máy', () => {
  it('mọi khoá có tiền tố km. và không rỗng', () => {
    for (const [k, v] of Object.entries(km)) {
      expect(k.startsWith('km.'), k).toBe(true);
      expect((v as string).trim().length, k).toBeGreaterThan(0);
    }
  });

  it('thay tham số; thiếu khoá thì trả chính khoá và cảnh báo console', () => {
    expect(tk('km.disk.rest', { count: 3, size: '1 GB' })).toBe('(+3 mục nhỏ khác, 1 GB)');
    expect(hasKey('km.tab.overview')).toBe(true);
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    expect(tk('km.khong.co')).toBe('km.khong.co');
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });
});
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `npx vitest run src/features/kham-may`
Expected: FAIL — `Failed to resolve import "./tauri"`, `"./fmt"`, `"./i18n"`.

- [ ] **Step 3: Viết mã**

`src/features/kham-may/vi.json`:

```json
{
  "km.tab.overview": "Tổng quan",
  "km.tab.disk": "Ổ đĩa",
  "km.tab.perf": "Bộ nhớ & Hiệu năng",

  "km.err.busy": "WinFreeUp đang bận với thao tác trước, hãy đợi xong rồi thử lại.",
  "km.err.protected": "Đây là thư mục của Windows hoặc của chương trình — không xóa được.",
  "km.err.essential": "Đây là tiến trình thiết yếu của Windows — không kết thúc được.",
  "km.err.staleTree": "Cây thư mục đã cũ, hãy quét lại.",
  "km.err.unknownApp": "App này đã thoát.",
  "km.err.aborted": "Bạn đã hủy thao tác xóa, không có gì bị xóa.",
  "km.err.logWrite": "Không ghi được nhật ký: {message}",

  "km.level.critical": "🔴 Nghiêm trọng",
  "km.level.warn": "🟠 Nên xử lý",
  "km.level.ok": "🟢 Ổn",
  "km.level.unknown": "⚪ Không đo được",

  "km.overview.title": "Khám nhanh",
  "km.overview.recheck": "Khám lại",
  "km.overview.checking": "Đang khám…",
  "km.overview.summary": "{critical} vấn đề nghiêm trọng · {warn} vấn đề nên xử lý",
  "km.overview.allGood": "Không thấy vấn đề nào.",
  "km.overview.unknown": "Không đo được: {detail}",
  "km.overview.failed": "Không khám được: {message}",
  "km.overview.throttleFailed": "Không đo được tốc độ CPU: {message}",

  "km.finding.disk_full.critical": "Ổ hệ thống gần đầy: chỉ còn {value}% trống. Windows có thể không cập nhật được và máy sẽ rất chậm.",
  "km.finding.disk_full.warn": "Ổ hệ thống chỉ còn {value}% trống — máy dễ chậm và thiếu chỗ cập nhật.",
  "km.finding.disk_full.ok": "Ổ hệ thống còn {value}% trống.",
  "km.finding.disk_health.critical": "Ổ cứng «{detail}» đang báo lỗi sức khỏe — hãy sao lưu dữ liệu ngay.",
  "km.finding.disk_health.ok": "Ổ cứng không báo lỗi sức khỏe.",
  "km.finding.system_hdd.warn": "Windows đang nằm trên ổ cứng quay (HDD) — thường là nguyên nhân lớn nhất khiến máy chậm.",
  "km.finding.system_hdd.ok": "Windows nằm trên ổ SSD.",
  "km.finding.ram_pressure.critical": "Máy đã dùng gần hết bộ nhớ ({value}% bộ nhớ ảo) — app có thể bị đóng đột ngột.",
  "km.finding.ram_pressure.warn": "RAM đang dùng {value}% — mở thêm app máy sẽ chậm.",
  "km.finding.ram_pressure.ok": "RAM đang dùng {value}%.",
  "km.finding.startup_apps.warn": "{value} app tự chạy cùng máy — máy khởi động chậm và tốn RAM.",
  "km.finding.startup_apps.ok": "{value} app tự chạy cùng máy.",
  "km.finding.uptime.warn": "Máy đã {value} ngày chưa khởi động lại. Lưu ý: bấm «Shut down» không tính là khởi động lại vì Windows bật Fast Startup — hãy chọn «Restart».",
  "km.finding.uptime.ok": "Máy khởi động lại cách đây {value} ngày.",
  "km.finding.power_saver.warn": "Máy đang cắm sạc nhưng vẫn để chế độ tiết kiệm pin — CPU bị giới hạn tốc độ.",
  "km.finding.power_saver.ok": "Chế độ nguồn bình thường.",
  "km.finding.cpu_throttle.warn": "CPU đang bị hạ xung (thường do nóng hoặc chế độ nguồn).",
  "km.finding.cpu_throttle.ok": "CPU chạy đủ tốc độ.",
  "km.finding.cpu_throttle.pending": "Đang đo tốc độ CPU trong 10 giây…",
  "km.finding.cpu_hot.warn": "CPU nóng {value} °C — máy tự hạ tốc độ để khỏi hỏng.",
  "km.finding.cpu_hot.ok": "Nhiệt độ CPU {value} °C.",

  "km.action.clean": "Dọn dẹp",
  "km.action.disk": "Xem Ổ đĩa",
  "km.action.backup": "Hướng dẫn sao lưu",
  "km.action.ssd": "Gợi ý chuyển SSD",
  "km.action.perf": "Xem Bộ nhớ & Hiệu năng",
  "km.action.startup": "Danh sách khởi động",
  "km.action.restart": "Gợi ý",
  "km.action.power": "Mở cài đặt Nguồn",
  "km.action.cooling": "Gợi ý vệ sinh / tản nhiệt",

  "km.guide.close": "Đóng",
  "km.guide.backup.title": "Sao lưu trước khi ổ hỏng",
  "km.guide.backup.body": "Ổ cứng báo lỗi sức khỏe có thể hỏng bất cứ lúc nào. Hãy chép ngay những gì quan trọng (Tài liệu, Ảnh, Màn hình nền, Zalo/email đã lưu) sang ổ USB, ổ cứng gắn ngoài hoặc OneDrive/Google Drive. Sau đó mang máy tới nơi sửa chữa để kiểm tra và thay ổ. Tránh chạy phần mềm chống phân mảnh hay chép dữ liệu nặng lên ổ đang hỏng.",
  "km.guide.ssd.title": "Chuyển Windows sang ổ SSD",
  "km.guide.ssd.body": "Ổ SSD nhanh hơn ổ quay nhiều lần: máy khởi động và mở app nhanh hẳn. Nhờ nơi sửa chữa lắp một ổ SSD (loại 2,5 inch SATA hoặc M.2 tùy máy) rồi chuyển nguyên Windows sang (nhân bản ổ) — dữ liệu và app giữ nguyên. Ổ cũ có thể giữ lại để chứa dữ liệu.",
  "km.guide.restart.title": "Khởi động lại đúng cách",
  "km.guide.restart.body": "Windows bật sẵn Fast Startup: khi bấm «Shut down», Windows chỉ ngủ đông phần lõi chứ không khởi động lại thật, nên lỗi và rác bộ nhớ vẫn còn. Hãy bấm Start → Nút nguồn → «Restart» ít nhất mỗi tuần một lần, nhất là sau khi Windows cập nhật.",
  "km.guide.cooling.title": "Giúp máy mát hơn",
  "km.guide.cooling.body": "Kê máy trên mặt phẳng cứng, không đặt trên chăn đệm. Vệ sinh khe gió và quạt (dùng bình khí nén hoặc nhờ nơi sửa chữa). Máy đã dùng vài năm nên thay keo tản nhiệt CPU. Máy xách tay có thể dùng đế tản nhiệt.",

  "km.startup.title": "App khởi động cùng máy",
  "km.startup.count": "{enabled}/{total} đang bật",
  "km.startup.empty": "Không có app nào tự chạy cùng máy.",
  "km.startup.loading": "Đang đọc danh sách khởi động…",
  "km.startup.failed": "Không đọc được danh sách khởi động: {message}",
  "km.startup.setFailed": "Không đổi được «{name}»: {message}",
  "km.startup.source.hkcu_run": "Riêng bạn",
  "km.startup.source.hklm_run": "Mọi người dùng",
  "km.startup.source.hklm_run32": "Mọi người dùng (32 bit)",
  "km.startup.source.user_folder": "Thư mục Startup của bạn",
  "km.startup.source.common_folder": "Thư mục Startup chung",

  "km.disk.volume": "Ổ đĩa",
  "km.disk.volumeOption": "{root} {label} — trống {free} / {total}",
  "km.disk.volumesFailed": "Không đọc được danh sách ổ đĩa: {message}",
  "km.disk.scan": "Quét",
  "km.disk.rescan": "Quét lại",
  "km.disk.cancel": "Hủy",
  "km.disk.cancelling": "Đang hủy…",
  "km.disk.scanning": "Đang quét {root}…",
  "km.disk.progress": "{files} file · {size}",
  "km.disk.scanFailed": "Không quét được: {message}",
  "km.disk.slowMode": "Đang dùng chế độ quét chậm vì {reason}.",
  "km.disk.reason.not_ntfs": "ổ dùng hệ tệp {fs}, không phải NTFS",
  "km.disk.reason.not_local": "đây không phải ổ cục bộ",
  "km.disk.reason.mft_failed": "không đọc được bảng MFT ({message})",
  "km.disk.done": "Quét xong trong {seconds} giây.",
  "km.disk.hint": "Chọn ổ rồi bấm Quét để xem thư mục nào chiếm chỗ nhiều nhất.",
  "km.disk.sort": "Sắp theo",
  "km.disk.sort.size": "Dung lượng",
  "km.disk.sort.name": "Tên",
  "km.disk.sort.modified": "Ngày sửa",
  "km.disk.col.name": "Tên",
  "km.disk.col.size": "Dung lượng",
  "km.disk.col.share": "% so với thư mục cha",
  "km.disk.col.modified": "Sửa gần nhất",
  "km.disk.expand": "Mở {name}",
  "km.disk.collapse": "Đóng {name}",
  "km.disk.unreadable": "Không đọc được",
  "km.disk.link": "Lối tắt tới chỗ khác (không tính dung lượng)",
  "km.disk.protected": "Được bảo vệ — không xóa được",
  "km.disk.rest": "(+{count} mục nhỏ khác, {size})",
  "km.disk.loadFailed": "Không mở được thư mục: {message}",
  "km.disk.reveal": "Mở trong Explorer",
  "km.disk.copy": "Sao chép đường dẫn",
  "km.disk.copied": "Đã chép đường dẫn.",
  "km.disk.copyFailed": "Không chép được đường dẫn: {message}",
  "km.disk.revealFailed": "Không mở được Explorer: {message}",
  "km.disk.delete": "Xóa vào Thùng rác",
  "km.disk.deleteTitle": "Xóa vào Thùng rác?",
  "km.disk.deleteBody": "«{name}» · {size} · {files} file. Bạn lấy lại được từ Thùng rác cho tới khi dọn Thùng rác.",
  "km.disk.deleteConfirm": "Xóa vào Thùng rác",
  "km.disk.deleteCancel": "Hủy",
  "km.disk.deleting": "Đang xóa…",
  "km.disk.deleted": "Đã chuyển «{name}» ({size}) vào Thùng rác.",
  "km.disk.deleteFailed": "Không xóa được «{name}»: {message}",
  "km.disk.hiberfil": "Tắt ngủ đông để lấy lại {size}",
  "km.disk.hiberfilGuide": "Cách làm",
  "km.disk.hiberfilTitle": "Tắt ngủ đông",
  "km.disk.hiberfilBody": "Mở Start, gõ cmd, bấm chuột phải «Command Prompt» → «Run as administrator», gõ lệnh powercfg /hibernate off rồi Enter. File hiberfil.sys sẽ biến mất. Lưu ý: tắt ngủ đông cũng tắt luôn Fast Startup và chế độ Hibernate. Muốn bật lại: powercfg /hibernate on.",

  "km.perf.starting": "Đang bắt đầu đo…",
  "km.perf.startFailed": "Không bắt đầu đo được: {message}",
  "km.perf.sampleFailed": "Lỗi khi lấy mẫu: {message}",
  "km.perf.chart.cpu": "CPU",
  "km.perf.chart.ram": "RAM",
  "km.perf.chart.disk": "Hoạt động đĩa",
  "km.perf.chart.net": "Mạng",
  "km.perf.netUp": "Tải lên",
  "km.perf.netDown": "Tải xuống",
  "km.perf.ramValue": "{pct}% · {used} / {total}",
  "km.perf.noValue": "Không đo được",
  "km.perf.diskMissing": "Không đo được hoạt động đĩa: {message}",
  "km.perf.throttled": "CPU đang bị hạ xung (thường do nóng hoặc chế độ nguồn).",
  "km.perf.notThrottled": "CPU chạy đủ tốc độ.",
  "km.perf.throttleMissing": "Không đo được hạ xung: {message}",
  "km.perf.temp": "Nhiệt độ CPU: {celsius} °C",
  "km.perf.tempMissing": "Máy này không cho đọc nhiệt độ",
  "km.perf.netAppMissing": "Không theo dõi được mạng theo từng app: {message}",
  "km.perf.slow": "Đang lấy mẫu mỗi 2 giây để WinFreeUp không làm máy nặng thêm.",
  "km.perf.stutters": "Cơn giật vừa ghi nhận",
  "km.perf.noStutters": "Chưa ghi nhận cơn giật nào trong 5 phút qua.",
  "km.perf.stutterLine": "{time} · {seconds} giây · {metrics}",
  "km.perf.stutterCpu": "CPU",
  "km.perf.stutterDisk": "Đĩa",
  "km.perf.stutterTop": "Ngốn nhất: {apps}",
  "km.perf.stutterThrottled": "Trùng lúc hạ xung",
  "km.perf.apps": "Ứng dụng",
  "km.perf.col.name": "Tên",
  "km.perf.col.ram": "RAM",
  "km.perf.col.cpu": "CPU",
  "km.perf.col.disk": "Đĩa (MB/s)",
  "km.perf.col.net": "Mạng (KB/s)",
  "km.perf.expand": "Xem các tiến trình của {name}",
  "km.perf.collapse": "Ẩn các tiến trình của {name}",
  "km.perf.kill": "Kết thúc app",
  "km.perf.reveal": "Mở vị trí file",
  "km.perf.essential": "Tiến trình thiết yếu của Windows",
  "km.perf.killTitle": "Kết thúc «{name}»?",
  "km.perf.killBody": "Mọi cửa sổ của app sẽ đóng ngay; phần chưa lưu sẽ mất.",
  "km.perf.killConfirm": "Kết thúc",
  "km.perf.killCancel": "Hủy",
  "km.perf.killing": "Đang kết thúc…",
  "km.perf.killed": "Đã kết thúc «{name}».",
  "km.perf.killPartial": "«{name}»: {count} tiến trình không kết thúc được, ví dụ: {first}",
  "km.perf.killFailed": "Không kết thúc được «{name}»: {message}",
  "km.perf.revealFailed": "Không mở được vị trí file: {message}",
  "km.perf.settingsFailed": "Không mở được Cài đặt: {message}"
}
```

`src/features/kham-may/i18n.ts`:

```ts
import km from './vi.json';

// Chuỗi của tab Khám máy tạm nằm ở `./vi.json` để thi công song song với v0.1 mà không chạm `src/i18n/vi.json`.
// Task Tích hợp trộn các khoá này vào `src/i18n/vi.json` rồi thay cả file này bằng: export { t as tk, hasKey } from '../../i18n';
const dict: Record<string, string> = km;

export type Params = Record<string, string | number>;

export function hasKey(key: string): boolean {
  return Object.prototype.hasOwnProperty.call(dict, key);
}

export function tk(key: string, params?: Params): string {
  if (!hasKey(key)) {
    console.warn(`[i18n] thiếu khoá: ${key}`);
    return key;
  }
  return dict[key].replace(/\{(\w+)\}/g, (whole, name: string) => (params && name in params ? String(params[name]) : whole));
}
```

`src/features/kham-may/api/types.ts`:

```ts
// Khớp serde của crate winfreeup-diagnose (tên trường snake_case) — đừng đổi tên.

export type DriveKind = 'fixed' | 'removable' | 'network' | 'cdrom' | 'ram' | 'unknown';

export interface VolumeInfo {
  /** Dạng "C:\\". */
  root: string;
  label: string;
  fs: string;
  total: number;
  free: number;
  kind: DriveKind;
}

export interface ScanStatus {
  files: number;
  bytes: number;
  current: string;
  /** Có khi quét bằng MFT; duyệt thư mục thì null. */
  percent: number | null;
}

export type WalkReason =
  | { code: 'not_ntfs'; fs: string }
  | { code: 'not_local'; kind: DriveKind }
  | { code: 'mft_failed'; message: string };

export type ScanPlan = { mode: 'mft' } | { mode: 'walk'; reason: WalkReason };

export interface NodeView {
  id: number;
  name: string;
  path: string;
  bytes: number;
  files: number;
  /** Giây Unix. */
  modified: number;
  is_dir: boolean;
  is_link: boolean;
  unreadable: boolean;
  has_children: boolean;
  protected: boolean;
}

export interface RestSummary {
  count: number;
  bytes: number;
}

export interface ChildrenPage {
  parent: NodeView;
  items: NodeView[];
  rest: RestSummary | null;
}

export interface ScanSummary {
  root: NodeView;
  plan: ScanPlan;
  elapsed_ms: number;
}

export interface Removed {
  bytes: number;
  files: number;
}

export interface DeleteResult {
  removed: Removed;
  parent: NodeView | null;
  log_error: string | null;
}

export type Level = 'critical' | 'warn' | 'ok' | 'unknown';

export type FindingId =
  | 'disk_full'
  | 'disk_health'
  | 'system_hdd'
  | 'ram_pressure'
  | 'startup_apps'
  | 'uptime'
  | 'power_saver'
  | 'cpu_throttle'
  | 'cpu_hot';

export interface Finding {
  id: FindingId;
  level: Level;
  value: number | null;
  detail: string | null;
}

export type StartupSource = 'hkcu_run' | 'hklm_run' | 'hklm_run32' | 'user_folder' | 'common_folder';

export interface StartupEntry {
  id: string;
  source: StartupSource;
  name: string;
  command: string;
  enabled: boolean;
}

export interface StartupList {
  entries: StartupEntry[];
  errors: string[];
}

export interface Sample {
  /** Giờ Unix (ms). */
  t_ms: number;
  dur_ms: number;
  cpu: number;
  ram_pct: number;
  ram_used: number;
  ram_total: number;
  disk_active: number | null;
  net_up_bps: number | null;
  net_down_bps: number | null;
  cpu_perf: number | null;
}

export interface ProcRow {
  pid: number;
  create_time: number;
  name: string;
  ram: number;
  cpu: number;
  disk_bps: number;
  net_up_bps: number | null;
  net_down_bps: number | null;
  essential: boolean;
}

export interface AppRow {
  key: string;
  name: string;
  path: string | null;
  ram: number;
  cpu: number;
  disk_bps: number;
  net_up_bps: number | null;
  net_down_bps: number | null;
  essential: boolean;
  procs: ProcRow[];
}

export interface TopApp {
  key: string;
  name: string;
  avg: number;
}

export interface StutterView {
  start_ms: number;
  end_ms: number;
  cpu: boolean;
  disk: boolean;
  top_cpu: TopApp[];
  top_disk: TopApp[];
  throttled: boolean;
}

export type TempState =
  | { state: 'ok'; celsius: number }
  | { state: 'unavailable'; code: 'no_sensor' | 'out_of_range' | 'stuck'; detail: string };

export interface PerfTick {
  sample: Sample;
  apps: AppRow[];
  stutters: StutterView[];
  throttled: boolean | null;
  perf_error: string | null;
  disk_error: string | null;
  temp: TempState;
  net_app_error: string | null;
  interval_ms: number;
}

export interface KillResult {
  killed: number;
  errors: string[];
  log_error: string | null;
}

/** Cầu nối tới vỏ Tauri cho tab Khám máy. Bản thật ở `api/tauri.ts`; test dùng `testing/fakeApi.ts`. */
export interface KhamMayApi {
  diskVolumes(): Promise<VolumeInfo[]>;
  diskScan(root: string, onProgress: (s: ScanStatus) => void): Promise<ScanSummary>;
  diskScanCancel(): Promise<void>;
  treeChildren(id: number): Promise<ChildrenPage>;
  diskDelete(id: number): Promise<DeleteResult>;
  diskReveal(id: number): Promise<void>;
  healthCheck(onFinding: (f: Finding) => void): Promise<Finding[]>;
  healthThrottle(): Promise<Finding>;
  startupList(): Promise<StartupList>;
  startupSet(id: string, enabled: boolean): Promise<StartupEntry>;
  openSettings(page: 'power' | 'battery' | 'startup' | 'storage'): Promise<void>;
  /** Bắt đầu lấy mẫu; trả lịch sử còn giữ. `onTick` nhận từng mẫu cho tới `perfStop`. */
  perfStart(onTick: (t: PerfTick) => void, onError: (message: string) => void): Promise<Sample[]>;
  perfStop(): Promise<void>;
  appKill(key: string): Promise<KillResult>;
  revealPath(path: string): Promise<void>;
  appIcon(path: string): Promise<string | null>;
  copyText(text: string): Promise<void>;
}

/** Cùng hình dạng `Notify` của v0.1 (`src/state/controller.ts`): App truyền xuống, lỗi lên băng đỏ/hổ phách. */
export type Notify = (severity: 'error' | 'warning', message: string) => void;
```

`src/features/kham-may/api/tauri.ts`:

```ts
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  ChildrenPage,
  DeleteResult,
  Finding,
  KhamMayApi,
  KillResult,
  PerfTick,
  Sample,
  ScanStatus,
  ScanSummary,
  StartupEntry,
  StartupList,
  VolumeInfo,
} from './types';

/** Gỡ lắng nghe của lần `perfStart` đang chạy. */
let perfUnlisten: UnlistenFn[] = [];
/** perfStart/perfStop xếp hàng lần lượt: StrictMode gọi start → stop → start liền nhau, chạy chồng thì
 *  lắng nghe của lần start đầu có thể bị bỏ sót không gỡ và mỗi mẫu tới hai lần. */
let perfQueue: Promise<unknown> = Promise.resolve();

function serial<T>(run: () => Promise<T>): Promise<T> {
  const next = perfQueue.then(run, run);
  perfQueue = next.catch(() => undefined);
  return next;
}

async function withEvent<E, R>(event: string, onEvent: (e: E) => void, run: () => Promise<R>): Promise<R> {
  const unlisten = await listen<E>(event, (e) => onEvent(e.payload));
  try {
    return await run();
  } finally {
    unlisten();
  }
}

// Tên lệnh/sự kiện khớp phần đăng ký trong src-tauri (Task Tích hợp). Tham số camelCase ⇒ Rust snake_case.
export const khamMayTauriApi: KhamMayApi = {
  diskVolumes: () => invoke<VolumeInfo[]>('disk_volumes'),
  diskScan: (root, onProgress) =>
    withEvent<ScanStatus, ScanSummary>('disk-scan-progress', onProgress, () => invoke<ScanSummary>('disk_scan', { root })),
  diskScanCancel: () => invoke<void>('disk_scan_cancel'),
  treeChildren: (id) => invoke<ChildrenPage>('tree_children', { nodeId: id }),
  diskDelete: (id) => invoke<DeleteResult>('disk_delete', { nodeId: id }),
  diskReveal: (id) => invoke<void>('disk_reveal', { nodeId: id }),
  healthCheck: (onFinding) => withEvent<Finding, Finding[]>('health-finding', onFinding, () => invoke<Finding[]>('health_check')),
  healthThrottle: () => invoke<Finding>('health_throttle'),
  startupList: () => invoke<StartupList>('startup_list'),
  startupSet: (id, enabled) => invoke<StartupEntry>('startup_set', { id, enabled }),
  openSettings: (page) => invoke<void>('open_settings', { page }),
  perfStart: (onTick, onError) =>
    serial(async () => {
      perfUnlisten.forEach((u) => u());
      perfUnlisten = [
        await listen<PerfTick>('perf-tick', (e) => onTick(e.payload)),
        await listen<string>('perf-error', (e) => onError(e.payload)),
      ];
      try {
        return await invoke<Sample[]>('perf_start');
      } catch (e) {
        perfUnlisten.forEach((u) => u());
        perfUnlisten = [];
        throw e;
      }
    }),
  perfStop: () =>
    serial(async () => {
      perfUnlisten.forEach((u) => u());
      perfUnlisten = [];
      await invoke<void>('perf_stop');
    }),
  appKill: (key) => invoke<KillResult>('app_kill', { key }),
  revealPath: (path) => invoke<void>('reveal_path', { path }),
  appIcon: (path) => invoke<string | null>('app_icon', { path }),
  copyText: (text) => navigator.clipboard.writeText(text),
};
```

`src/features/kham-may/fmt.ts`:

```ts
import { formatBytes } from '../../format';
import { tk } from './i18n';

export { formatBytes };

const one = new Intl.NumberFormat('vi-VN', { maximumFractionDigits: 1 });
const int = new Intl.NumberFormat('vi-VN', { maximumFractionDigits: 0 });

/** Số nguyên có dấu chấm ngăn cách hàng nghìn kiểu Việt Nam: 1234567 ⇒ "1.234.567". */
export function formatCount(n: number): string {
  return int.format(n);
}

/** Byte/giây ⇒ MB/s một chữ số thập phân ("0" khi không có hoạt động). */
export function formatMBps(bps: number): string {
  return one.format(Math.max(0, bps) / 1024 ** 2);
}

/** Byte/giây ⇒ KB/s số nguyên. */
export function formatKBps(bps: number): string {
  return int.format(Math.max(0, bps) / 1024);
}

/** Tốc độ mạng tự chọn đơn vị cho nhãn biểu đồ. */
export function formatRate(bps: number): string {
  if (bps >= 1024 ** 2) return `${one.format(bps / 1024 ** 2)} MB/s`;
  return `${int.format(Math.max(0, bps) / 1024)} KB/s`;
}

/** ms ⇒ giây một chữ số thập phân kiểu Việt Nam: 4200 ⇒ "4,2". */
export function formatSeconds(ms: number): string {
  return one.format(ms / 1000);
}

export function formatPct(v: number): string {
  return `${int.format(v)}%`;
}

/** Giây Unix ⇒ "dd/MM/yyyy" theo giờ địa phương; 0 ⇒ chuỗi rỗng. */
export function formatDate(unixSeconds: number): string {
  if (!unixSeconds) return '';
  const d = new Date(unixSeconds * 1000);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${p(d.getDate())}/${p(d.getMonth() + 1)}/${d.getFullYear()}`;
}

/** ms Unix ⇒ "HH:mm:ss" theo giờ địa phương. */
export function formatClock(ms: number): string {
  const d = new Date(ms);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

/**
 * Lỗi lệnh Tauri là chuỗi: mã ngắn của lõi đổi sang câu dễ hiểu, còn lại giữ nguyên văn để người dùng
 * chụp màn hình gửi lại được.
 */
export function friendly(e: unknown): string {
  const m = e instanceof Error ? e.message : typeof e === 'string' ? e : JSON.stringify(e);
  switch (m) {
    case 'busy':
      return tk('km.err.busy');
    case 'protected':
      return tk('km.err.protected');
    case 'essential':
      return tk('km.err.essential');
    case 'no_tree':
    case 'unknown_node':
      return tk('km.err.staleTree');
    case 'unknown_app':
      return tk('km.err.unknownApp');
    case 'aborted':
      return tk('km.err.aborted');
    default:
      return m;
  }
}
```

`src/features/kham-may/ui/Spin.tsx`:

```tsx
import { Spinner } from '@fluentui/react-components';

/** Vòng quay có nhãn. Lớp `wfu-busy` để luật reduced-motion chung của v0.1 (styles.css) cũng áp vào;
 *  `km.css` có luật riêng tương đương để tab này tự đủ trước khi gộp. */
export function Spin({ label, size = 'tiny' }: { label?: string; size?: 'extra-tiny' | 'tiny' | 'small' | 'medium' }) {
  return (
    <span className="wfu-busy km-busy">
      <Spinner size={size} label={label} labelPosition="after" />
    </span>
  );
}
```

`src/features/kham-may/km.css`:

```css
/* Tab Khám máy — mọi lớp có tiền tố km- để không đụng styles.css của v0.1. */

.km-root {
  display: grid;
  gap: 16px;
}

.km-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.km-muted {
  opacity: 0.75;
  font-size: 13px;
}

.km-num {
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  text-align: right;
}

.km-busy {
  display: inline-flex;
  align-items: center;
}

/* Tải lại một phần: giữ nội dung cũ mờ bên dưới lớp phủ, không xoá trắng. */
.km-overlay-host {
  position: relative;
}

.km-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 24px;
  background: color-mix(in srgb, Canvas 55%, transparent);
  z-index: 1;
}

/* ---- Tổng quan ---- */
.km-findings {
  display: grid;
  gap: 8px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.km-finding {
  display: grid;
  grid-template-columns: 150px 1fr auto;
  align-items: center;
  gap: 8px 12px;
  padding: 10px 12px;
  border-radius: 6px;
  border: 1px solid color-mix(in srgb, currentColor 14%, transparent);
}

.km-finding-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  justify-content: flex-end;
}

.km-startup {
  display: grid;
  gap: 4px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.km-startup-item {
  display: grid;
  grid-template-columns: auto 1fr;
  align-items: center;
  gap: 2px 12px;
}

.km-startup-cmd {
  grid-column: 2;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ---- Ổ đĩa ---- */
.km-tree {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.km-tree th,
.km-tree td {
  padding: 3px 6px;
  text-align: left;
}

.km-tree th {
  font-weight: 600;
  border-bottom: 1px solid color-mix(in srgb, currentColor 20%, transparent);
}

.km-tree tr:hover td {
  background: color-mix(in srgb, currentColor 6%, transparent);
}

.km-tree-name {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
}

.km-tree-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.km-share {
  width: 120px;
}

.km-share-bar {
  height: 8px;
  border-radius: 4px;
  background: color-mix(in srgb, currentColor 12%, transparent);
  overflow: hidden;
}

.km-share-fill {
  height: 100%;
  background: #0f6cbd;
}

.km-row-actions {
  display: flex;
  gap: 2px;
  justify-content: flex-end;
}

/* ---- Bộ nhớ & Hiệu năng ---- */
.km-charts {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
  gap: 12px;
}

.km-chart {
  display: grid;
  gap: 4px;
}

.km-chart-head {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  font-size: 13px;
}

.km-chart svg {
  width: 100%;
  height: 90px;
  display: block;
  border: 1px solid color-mix(in srgb, currentColor 14%, transparent);
  border-radius: 4px;
}

.km-chart-line {
  fill: none;
  stroke: #0f6cbd;
  stroke-width: 1.5;
  vector-effect: non-scaling-stroke;
}

.km-chart-line.km-alt {
  stroke: #8764b8;
}

/* Dải đỏ đánh dấu cơn giật. */
.km-chart-band {
  fill: #d13438;
  fill-opacity: 0.18;
}

.km-apps {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.km-apps th,
.km-apps td {
  padding: 3px 6px;
}

.km-apps th button {
  font-weight: 600;
}

.km-app-icon {
  width: 16px;
  height: 16px;
  flex: none;
}

.km-proc td:first-child {
  padding-left: 40px;
}

.km-stutters {
  display: grid;
  gap: 6px;
  margin: 0;
  padding: 0;
  list-style: none;
}

/* Giảm chuyển động: dừng quay nhưng KHÔNG bỏ tín hiệu — đổi sang nhịp mờ tỏ. */
@media (prefers-reduced-motion: reduce) {
  .km-busy .fui-Spinner__spinner,
  .km-busy .fui-Spinner__spinnerTail {
    animation: none !important;
  }
  .km-busy {
    animation: km-pulse 1.4s ease-in-out infinite;
  }
}

@keyframes km-pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.35;
  }
}
```

`src/features/kham-may/testing/fakeApi.ts`:

```ts
import { vi } from 'vitest';
import type { AppRow, ChildrenPage, Finding, KhamMayApi, NodeView, PerfTick, Sample } from '../api/types';

export const GB = 1024 ** 3;

export function node(id: number, name: string, bytes: number, extra: Partial<NodeView> = {}): NodeView {
  return {
    id,
    name,
    path: `C:\\${name}`,
    bytes,
    files: 1,
    modified: 1_790_000_000,
    is_dir: true,
    is_link: false,
    unreadable: false,
    has_children: true,
    protected: false,
    ...extra,
  };
}

/** Cây giả: C:\ (0) ─ Users (1) ─ an (2) ─ Downloads (3) ─ film.mkv (4); Windows (5, 🔒); hiberfil.sys (6, 🔒). */
export const ROOT = node(0, 'C:\\', 60 * GB, { path: 'C:\\', protected: true, files: 9 });
export const PAGES: Record<number, ChildrenPage> = {
  0: {
    parent: ROOT,
    items: [
      node(1, 'Users', 30 * GB, { protected: true, files: 5 }),
      node(5, 'Windows', 20 * GB, { protected: true, files: 3 }),
      node(6, 'hiberfil.sys', 8 * GB, { is_dir: false, has_children: false, protected: true }),
    ],
    rest: { count: 3, bytes: 2 * GB },
  },
  1: { parent: node(1, 'Users', 30 * GB, { protected: true }), items: [node(2, 'an', 30 * GB, { path: 'C:\\Users\\an', protected: true })], rest: null },
  2: {
    parent: node(2, 'an', 30 * GB, { path: 'C:\\Users\\an', protected: true }),
    items: [node(3, 'Downloads', 30 * GB, { path: 'C:\\Users\\an\\Downloads', files: 4 })],
    rest: null,
  },
  3: {
    parent: node(3, 'Downloads', 30 * GB, { path: 'C:\\Users\\an\\Downloads' }),
    items: [node(4, 'film.mkv', 30 * GB, { path: 'C:\\Users\\an\\Downloads\\film.mkv', is_dir: false, has_children: false })],
    rest: null,
  },
};

export const FINDINGS: Finding[] = [
  { id: 'disk_full', level: 'warn', value: 9.5, detail: null },
  { id: 'disk_health', level: 'ok', value: null, detail: null },
  { id: 'system_hdd', level: 'ok', value: null, detail: null },
  { id: 'ram_pressure', level: 'ok', value: 60, detail: null },
  { id: 'startup_apps', level: 'warn', value: 12, detail: null },
  { id: 'uptime', level: 'ok', value: 1.2, detail: null },
  { id: 'power_saver', level: 'ok', value: null, detail: null },
  { id: 'cpu_hot', level: 'unknown', value: null, detail: 'HRESULT Call failed with: 0x8004100C' },
];

export function sample(t_ms: number, cpu = 20): Sample {
  return {
    t_ms,
    dur_ms: 1000,
    cpu,
    ram_pct: 50,
    ram_used: 8 * GB,
    ram_total: 16 * GB,
    disk_active: 10,
    net_up_bps: 1024,
    net_down_bps: 4096,
    cpu_perf: 100,
  };
}

export function app(key: string, name: string, ram: number, extra: Partial<AppRow> = {}): AppRow {
  return {
    key,
    name,
    path: `C:\\Apps\\${key}`,
    ram,
    cpu: 1,
    disk_bps: 0,
    net_up_bps: 0,
    net_down_bps: 0,
    essential: false,
    procs: [{ pid: 100, create_time: 1, name: `${key}`, ram, cpu: 1, disk_bps: 0, net_up_bps: 0, net_down_bps: 0, essential: false }],
    ...extra,
  };
}

export function tick(t_ms: number, extra: Partial<PerfTick> = {}): PerfTick {
  return {
    sample: sample(t_ms),
    apps: [app('chrome.exe', 'Google Chrome', 2 * GB), app('svchost.exe', 'Service Host', GB, { essential: true, path: 'C:\\Windows\\System32\\svchost.exe' })],
    stutters: [],
    throttled: false,
    perf_error: null,
    disk_error: null,
    temp: { state: 'ok', celsius: 55 },
    net_app_error: null,
    interval_ms: 1000,
    ...extra,
  };
}

/** Bộ nối giả: mọi hàm là `vi.fn` trả dữ liệu mẫu ở trên; ghi đè từng hàm qua `over`. */
export function fakeApi(over: Partial<KhamMayApi> = {}): KhamMayApi {
  return {
    diskVolumes: vi.fn(async () => [
      { root: 'C:\\', label: 'Windows', fs: 'NTFS', total: 237 * GB, free: 20 * GB, kind: 'fixed' as const },
      { root: 'E:\\', label: 'USB', fs: 'FAT32', total: 32 * GB, free: 30 * GB, kind: 'removable' as const },
    ]),
    diskScan: vi.fn(async (_root: string, onProgress) => {
      onProgress({ files: 100, bytes: GB, current: 'C:\\Users', percent: 50 });
      return { root: ROOT, plan: { mode: 'mft' as const }, elapsed_ms: 4200 };
    }),
    diskScanCancel: vi.fn(async () => {}),
    treeChildren: vi.fn(async (id: number) => {
      const p = PAGES[id];
      if (!p) throw 'unknown_node';
      return p;
    }),
    diskDelete: vi.fn(async () => ({ removed: { bytes: 30 * GB, files: 1 }, parent: node(3, 'Downloads', 0), log_error: null })),
    diskReveal: vi.fn(async () => {}),
    healthCheck: vi.fn(async (onFinding) => {
      FINDINGS.forEach(onFinding);
      return FINDINGS;
    }),
    healthThrottle: vi.fn(async () => ({ id: 'cpu_throttle' as const, level: 'ok' as const, value: null, detail: null })),
    startupList: vi.fn(async () => ({
      entries: [
        { id: 'hkcu_run:OneDrive', source: 'hkcu_run' as const, name: 'OneDrive', command: 'C:\\OneDrive.exe /background', enabled: true },
        { id: 'user_folder:Zalo.lnk', source: 'user_folder' as const, name: 'Zalo.lnk', command: 'C:\\Startup\\Zalo.lnk', enabled: false },
      ],
      errors: [],
    })),
    startupSet: vi.fn(async (id: string, enabled: boolean) => ({
      id,
      source: 'hkcu_run' as const,
      name: id.split(':')[1],
      command: '',
      enabled,
    })),
    openSettings: vi.fn(async () => {}),
    perfStart: vi.fn(async () => [sample(1_000_000)]),
    perfStop: vi.fn(async () => {}),
    appKill: vi.fn(async () => ({ killed: 1, errors: [], log_error: null })),
    revealPath: vi.fn(async () => {}),
    appIcon: vi.fn(async () => null),
    copyText: vi.fn(async () => {}),
    ...over,
  };
}

export function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}
```

- [ ] **Step 4: Chạy test, thấy qua**

Run: `npx vitest run src/features/kham-may` rồi `npm run typecheck`
Expected: `Tests  11 passed`; typecheck không lỗi; `npm test` toàn bộ `0 failed`.

- [ ] **Step 5: Commit**

```bash
rtk git add src/features/kham-may/vi.json
rtk git add src/features/kham-may/i18n.ts
rtk git add src/features/kham-may/i18n.test.ts
rtk git add src/features/kham-may/api/types.ts
rtk git add src/features/kham-may/api/tauri.ts
rtk git add src/features/kham-may/api/tauri.test.ts
rtk git add src/features/kham-may/fmt.ts
rtk git add src/features/kham-may/fmt.test.ts
rtk git add src/features/kham-may/ui/Spin.tsx
rtk git add src/features/kham-may/km.css
rtk git add src/features/kham-may/testing/fakeApi.ts
rtk git commit -m "feat(kham-may): khung giao diện — chuỗi tiếng Việt, kiểu dữ liệu, cầu nối Tauri, bản giả

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Cây thư mục gọn trong bộ nhớ (`DiskTree`)

**Files:**
- Modify (thay dòng khung): `crates/winfreeup-diagnose/src/disk/tree.rs`
- Test: `tree.rs` (module `tests`)

**Interfaces:**
- Consumes: không (chỉ `serde`).
- Produces:
  - `type NodeId = u32`, `NO_PARENT: NodeId = u32::MAX`, `MAX_CHILDREN: usize = 200`, cờ `FLAG_DIR = 1`, `FLAG_UNREADABLE = 2`, `FLAG_LINK = 4`, `FLAG_DELETED = 8` (`u16`).
  - `TreeBuilder::new()`, `with_capacity(nodes)`, `add(&mut self, parent: NodeId, name: &str, flags: u16, bytes: u64, modified: u32) -> NodeId`, `set_parent(id, parent)`, `add_flags(id, flags)`, `len()`, `finish(self, root: NodeId) -> DiskTree` (cộng dồn byte/số file/ngày sửa lên tổ tiên; cha thêm sau con vẫn đúng; con xếp byte giảm dần).
  - `DiskTree`: `root()`, `len()`, `contains(id)`, `name(id)`, `bytes(id)`, `files(id)`, `is_dir(id)`, `parent_of(id) -> Option<NodeId>`, `path(id) -> String` (`C:\a\b`), `view(id) -> NodeView`, `children(id, limit) -> Option<ChildrenPage>`, `remove(id) -> Option<Removed>` (trừ khỏi mọi tổ tiên, không xóa gốc), `child_named(id, name) -> Option<NodeId>`.
  - `NodeView { id, name, path, bytes, files, modified, is_dir, is_link, unreadable, has_children, protected }`, `RestSummary { count, bytes }`, `ChildrenPage { parent: NodeView, items: Vec<NodeView>, rest: Option<RestSummary> }`, `Removed { bytes, files }` — đều `Serialize`.

- [ ] **Step 1: Viết test hỏng** — thay dòng khung của `tree.rs` bằng:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// C:\ ─ big (dir) ─ a.bin 300, b.bin 100 ; small.txt 5
    fn sample() -> (DiskTree, NodeId, NodeId, NodeId) {
        let mut b = TreeBuilder::new();
        let root = b.add(NO_PARENT, "C:\\", FLAG_DIR, 0, 0);
        let big = b.add(root, "big", FLAG_DIR, 0, 0);
        let a = b.add(big, "a.bin", 0, 300, 1_700_000_000);
        b.add(big, "b.bin", 0, 100, 1_600_000_000);
        b.add(root, "small.txt", 0, 5, 1_500_000_000);
        (b.finish(root), root, big, a)
    }

    #[test]
    fn totals_roll_up_to_every_ancestor() {
        let (t, root, big, _) = sample();
        assert_eq!(t.bytes(root), 405);
        assert_eq!(t.files(root), 3);
        assert_eq!(t.bytes(big), 400);
        assert_eq!(t.view(big).modified, 1_700_000_000);
    }

    #[test]
    fn children_are_sorted_largest_first_with_paths() {
        let (t, root, big, _) = sample();
        let page = t.children(root, MAX_CHILDREN).unwrap();
        assert_eq!(page.items.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), vec!["big", "small.txt"]);
        assert_eq!(page.items[0].path, "C:\\big");
        assert!(page.items[0].has_children && page.items[0].is_dir);
        assert!(page.rest.is_none());
        assert_eq!(t.children(big, MAX_CHILDREN).unwrap().items[0].path, "C:\\big\\a.bin");
    }

    #[test]
    fn more_than_the_limit_collapses_into_one_summary_row() {
        let mut b = TreeBuilder::new();
        let root = b.add(NO_PARENT, "D:\\", FLAG_DIR, 0, 0);
        for i in 1..=205u64 {
            b.add(root, &format!("f{i}"), 0, i, 0);
        }
        let t = b.finish(root);
        let page = t.children(root, MAX_CHILDREN).unwrap();
        assert_eq!(page.items.len(), 200);
        assert_eq!(page.items[0].bytes, 205);
        assert_eq!(page.items[199].bytes, 6);
        assert_eq!(page.rest, Some(RestSummary { count: 5, bytes: 1 + 2 + 3 + 4 + 5 }));
    }

    #[test]
    fn exactly_the_limit_has_no_summary_row() {
        let mut b = TreeBuilder::new();
        let root = b.add(NO_PARENT, "D:\\", FLAG_DIR, 0, 0);
        for i in 0..200u64 {
            b.add(root, &format!("f{i}"), 0, i, 0);
        }
        let t = b.finish(root);
        let page = t.children(root, MAX_CHILDREN).unwrap();
        assert_eq!(page.items.len(), 200);
        assert!(page.rest.is_none());
    }

    #[test]
    fn remove_subtracts_from_ancestors_and_hides_the_node() {
        let (mut t, root, big, a) = sample();
        assert_eq!(t.remove(a), Some(Removed { bytes: 300, files: 1 }));
        assert_eq!(t.bytes(big), 100);
        assert_eq!(t.bytes(root), 105);
        assert_eq!(t.files(root), 2);
        assert!(!t.contains(a));
        assert_eq!(t.children(big, MAX_CHILDREN).unwrap().items.len(), 1);
        assert_eq!(t.remove(a), None, "xóa lần hai không trừ thêm");
        assert_eq!(t.remove(root), None, "không xóa gốc");
        assert!(t.children(a, MAX_CHILDREN).is_none());
    }

    #[test]
    fn parents_added_after_children_still_roll_up() {
        // Thứ tự bản ghi MFT: con có thể đứng trước cha.
        let mut b = TreeBuilder::new();
        let file = b.add(NO_PARENT, "x.dat", 0, 50, 0);
        let dir = b.add(NO_PARENT, "dir", FLAG_DIR, 0, 0);
        let root = b.add(NO_PARENT, "E:\\", FLAG_DIR, 0, 0);
        b.set_parent(file, dir);
        b.set_parent(dir, root);
        let t = b.finish(root);
        assert_eq!(t.bytes(root), 50);
        assert_eq!(t.path(file), "E:\\dir\\x.dat");
    }

    #[test]
    fn links_are_not_counted_as_files() {
        let mut b = TreeBuilder::new();
        let root = b.add(NO_PARENT, "C:\\", FLAG_DIR, 0, 0);
        b.add(root, "junction", FLAG_DIR | FLAG_LINK, 0, 0);
        b.add(root, "f", 0, 1, 0);
        let t = b.finish(root);
        assert_eq!(t.files(root), 1);
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run (trong `crates/winfreeup-diagnose`): `cargo test disk::tree`
Expected: FAIL biên dịch — `cannot find type TreeBuilder`.

- [ ] **Step 3: Viết mã (trên khối test)**

```rust
//! Cây thư mục gọn trong bộ nhớ: mảng nút + bảng tên chung, con xếp kiểu CSR sau khi `finish`.
use serde::Serialize;

pub type NodeId = u32;
pub const NO_PARENT: NodeId = u32::MAX;
/// Spec 3.1: giao diện chỉ nhận tối đa 200 con lớn nhất mỗi lần mở.
pub const MAX_CHILDREN: usize = 200;

pub const FLAG_DIR: u16 = 1;
pub const FLAG_UNREADABLE: u16 = 2;
pub const FLAG_LINK: u16 = 4;
pub const FLAG_DELETED: u16 = 8;

#[derive(Debug, Clone, Copy)]
struct Node {
    name_off: u32,
    name_len: u16,
    flags: u16,
    parent: NodeId,
    /// Số file trong cây con (file thường tính 1 chính nó).
    files: u32,
    /// Giây Unix, lớn nhất trong cây con.
    modified: u32,
    /// Byte thực chiếm trên đĩa của cả cây con.
    bytes: u64,
}

/// Dựng cây trong lúc quét. Nút cha có thể thêm sau nút con (MFT) — `finish` tự sắp lại.
#[derive(Default)]
pub struct TreeBuilder {
    nodes: Vec<Node>,
    names: String,
}

impl TreeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(nodes: usize) -> Self {
        TreeBuilder { nodes: Vec::with_capacity(nodes), names: String::with_capacity(nodes * 16) }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Thêm một nút; `bytes`, `modified` là của riêng nút (file) — thư mục truyền 0.
    pub fn add(&mut self, parent: NodeId, name: &str, flags: u16, bytes: u64, modified: u32) -> NodeId {
        let id = self.nodes.len() as NodeId;
        let name = truncate_name(name);
        let name_off = self.names.len() as u32;
        self.names.push_str(name);
        let is_file = flags & FLAG_DIR == 0;
        self.nodes.push(Node {
            name_off,
            name_len: name.len() as u16,
            flags,
            parent,
            files: u32::from(is_file && flags & FLAG_LINK == 0),
            modified,
            bytes,
        });
        id
    }

    pub fn set_parent(&mut self, id: NodeId, parent: NodeId) {
        self.nodes[id as usize].parent = parent;
    }

    pub fn add_flags(&mut self, id: NodeId, flags: u16) {
        self.nodes[id as usize].flags |= flags;
    }

    /// Cộng dồn byte/số file/ngày sửa lên mọi tổ tiên và xếp con. `root` là gốc (ổ đĩa).
    pub fn finish(self, root: NodeId) -> DiskTree {
        let TreeBuilder { mut nodes, names } = self;
        let n = nodes.len();
        // CSR: đếm con của từng nút.
        let mut start = vec![0u32; n + 1];
        for (i, node) in nodes.iter().enumerate() {
            if i as NodeId != root && (node.parent as usize) < n {
                start[node.parent as usize + 1] += 1;
            }
        }
        for i in 0..n {
            start[i + 1] += start[i];
        }
        let mut fill = start.clone();
        let mut list = vec![0u32; start[n] as usize];
        for (i, node) in nodes.iter().enumerate() {
            if i as NodeId != root && (node.parent as usize) < n {
                let p = node.parent as usize;
                list[fill[p] as usize] = i as NodeId;
                fill[p] += 1;
            }
        }
        // Hậu thứ tự lặp từ gốc: con xong mới tới cha. Nút không nối được về gốc bị bỏ qua.
        let mut order = Vec::with_capacity(n);
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            order.push(id);
            let (a, b) = (start[id as usize] as usize, start[id as usize + 1] as usize);
            stack.extend_from_slice(&list[a..b]);
        }
        for &id in order.iter().rev() {
            let node = nodes[id as usize];
            if node.parent != NO_PARENT && id != root {
                let p = &mut nodes[node.parent as usize];
                p.bytes += node.bytes;
                p.files += node.files;
                p.modified = p.modified.max(node.modified);
            }
        }
        // Mỗi danh sách con xếp sẵn theo byte giảm dần, cùng byte thì theo tên.
        for id in 0..n {
            let (a, b) = (start[id] as usize, start[id + 1] as usize);
            list[a..b].sort_by(|&x, &y| {
                let (nx, ny) = (&nodes[x as usize], &nodes[y as usize]);
                ny.bytes.cmp(&nx.bytes).then_with(|| name_of(&names, nx).cmp(name_of(&names, ny)))
            });
        }
        DiskTree { nodes, names, child_start: start, child_list: list, root }
    }
}

fn truncate_name(name: &str) -> &str {
    if name.len() <= u16::MAX as usize {
        return name;
    }
    let mut end = u16::MAX as usize;
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    &name[..end]
}

fn name_of<'a>(names: &'a str, n: &Node) -> &'a str {
    &names[n.name_off as usize..n.name_off as usize + n.name_len as usize]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NodeView {
    pub id: NodeId,
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub files: u64,
    pub modified: u32,
    pub is_dir: bool,
    pub is_link: bool,
    pub unreadable: bool,
    pub has_children: bool,
    /// Điền ở tầng dịch vụ (cần luật bảo vệ); cây để `false`.
    pub protected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RestSummary {
    pub count: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChildrenPage {
    pub parent: NodeView,
    pub items: Vec<NodeView>,
    /// "(+N mục nhỏ khác, X GB)" khi có hơn 200 con.
    pub rest: Option<RestSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Removed {
    pub bytes: u64,
    pub files: u64,
}

pub struct DiskTree {
    nodes: Vec<Node>,
    names: String,
    child_start: Vec<u32>,
    child_list: Vec<NodeId>,
    root: NodeId,
}

impl DiskTree {
    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn contains(&self, id: NodeId) -> bool {
        (id as usize) < self.nodes.len() && self.nodes[id as usize].flags & FLAG_DELETED == 0
    }

    pub fn name(&self, id: NodeId) -> &str {
        name_of(&self.names, &self.nodes[id as usize])
    }

    pub fn bytes(&self, id: NodeId) -> u64 {
        self.nodes[id as usize].bytes
    }

    pub fn files(&self, id: NodeId) -> u64 {
        u64::from(self.nodes[id as usize].files)
    }

    pub fn is_dir(&self, id: NodeId) -> bool {
        self.nodes[id as usize].flags & FLAG_DIR != 0
    }

    pub fn parent_of(&self, id: NodeId) -> Option<NodeId> {
        let p = self.nodes[id as usize].parent;
        (id != self.root && p != NO_PARENT).then_some(p)
    }

    /// Đường dẫn đầy đủ: tên gốc (vd `C:\`) nối các tên con bằng `\`.
    pub fn path(&self, id: NodeId) -> String {
        let mut parts = Vec::new();
        let mut cur = id;
        while cur != NO_PARENT && cur != self.root {
            parts.push(self.name(cur));
            cur = self.nodes[cur as usize].parent;
        }
        let mut out = self.name(self.root).to_string();
        for p in parts.iter().rev() {
            if !out.ends_with('\\') {
                out.push('\\');
            }
            out.push_str(p);
        }
        out
    }

    fn live_children(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let (a, b) = (self.child_start[id as usize] as usize, self.child_start[id as usize + 1] as usize);
        self.child_list[a..b].iter().copied().filter(|&c| self.nodes[c as usize].flags & FLAG_DELETED == 0)
    }

    pub fn view(&self, id: NodeId) -> NodeView {
        let n = &self.nodes[id as usize];
        NodeView {
            id,
            name: self.name(id).to_string(),
            path: self.path(id),
            bytes: n.bytes,
            files: u64::from(n.files),
            modified: n.modified,
            is_dir: n.flags & FLAG_DIR != 0,
            is_link: n.flags & FLAG_LINK != 0,
            unreadable: n.flags & FLAG_UNREADABLE != 0,
            has_children: self.live_children(id).next().is_some(),
            protected: false,
        }
    }

    /// Tối đa `limit` con lớn nhất (đã xếp sẵn), phần còn lại gộp thành một dòng tổng.
    pub fn children(&self, id: NodeId, limit: usize) -> Option<ChildrenPage> {
        if !self.contains(id) {
            return None;
        }
        let mut items = Vec::new();
        let mut rest = RestSummary { count: 0, bytes: 0 };
        for c in self.live_children(id) {
            if items.len() < limit {
                items.push(self.view(c));
            } else {
                rest.count += 1;
                rest.bytes += self.nodes[c as usize].bytes;
            }
        }
        Some(ChildrenPage { parent: self.view(id), items, rest: (rest.count > 0).then_some(rest) })
    }

    /// Sau khi xóa vào Thùng rác: đánh dấu nút đã xóa và trừ byte/số file khỏi mọi tổ tiên (không quét lại).
    pub fn remove(&mut self, id: NodeId) -> Option<Removed> {
        if !self.contains(id) || id == self.root {
            return None;
        }
        let (bytes, files) = (self.nodes[id as usize].bytes, self.nodes[id as usize].files);
        self.nodes[id as usize].flags |= FLAG_DELETED;
        let mut cur = self.nodes[id as usize].parent;
        while cur != NO_PARENT {
            let p = &mut self.nodes[cur as usize];
            p.bytes = p.bytes.saturating_sub(bytes);
            p.files = p.files.saturating_sub(files);
            cur = if cur == self.root { NO_PARENT } else { p.parent };
        }
        Some(Removed { bytes, files: u64::from(files) })
    }

    /// Tìm con theo tên (không phân biệt hoa thường) — dùng trong test và khi đối chiếu hai bộ quét.
    pub fn child_named(&self, id: NodeId, name: &str) -> Option<NodeId> {
        self.live_children(id).find(|&c| self.name(c).eq_ignore_ascii_case(name))
    }
}
```

- [ ] **Step 4: Chạy test và clippy**

Run: `cargo test disk::tree` rồi `cargo test` rồi `cargo clippy --all-targets -- -D warnings`
Expected: `7 passed`; toàn bộ `0 failed`; clippy sạch.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-diagnose/src/disk/tree.rs
rtk git commit -m "feat(diagnose): cây thư mục gọn — cộng dồn, 200 con lớn nhất + dòng tổng, trừ sau khi xóa

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Danh sách đường dẫn được bảo vệ

**Files:**
- Modify (thay dòng khung): `crates/winfreeup-diagnose/src/disk/protect.rs`
- Test: `protect.rs`

**Interfaces:**
- Consumes: không. Dev-dependency `junction` (Task 1).
- Produces: `ProtectRules::new(system_dirs: &[PathBuf], user_profile: &Path)`, `ProtectRules::from_env() -> Result<Self, String>` (`SystemRoot`, `ProgramData`, `ProgramFiles`, `ProgramFiles(x86)`, `ProgramW6432`, `USERPROFILE` và thư mục cha của nó), `is_protected(&self, path: &Path) -> bool` (kiểm cả đường gõ vào lẫn đường sau `canonicalize`; một trong hai bị chặn là chặn), `is_protected_lexical(&self, path: &Path) -> bool` (không chạm đĩa).

- [ ] **Step 1: Viết test hỏng** — thay dòng khung bằng:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> ProtectRules {
        ProtectRules::new(
            &[
                PathBuf::from(r"C:\Windows"),
                PathBuf::from(r"C:\Program Files"),
                PathBuf::from(r"C:\Program Files (x86)"),
                PathBuf::from(r"C:\ProgramData"),
                PathBuf::from(r"C:\Users"),
            ],
            Path::new(r"C:\Users\an"),
        )
    }

    #[test]
    fn roots_and_system_dirs_themselves_are_blocked() {
        let r = rules();
        for p in [r"C:\", r"D:\", r"C:\Windows", r"c:\windows\", r"C:\Program Files", r"C:\Program Files (x86)", r"C:\ProgramData", r"C:\Users", r"C:\Users\an"] {
            assert!(r.is_protected_lexical(Path::new(p)), "{p}");
        }
    }

    #[test]
    fn inside_system_dirs_is_blocked_but_inside_the_profile_is_allowed() {
        let r = rules();
        assert!(r.is_protected_lexical(Path::new(r"C:\Windows\System32\drivers")));
        assert!(r.is_protected_lexical(Path::new(r"C:\Program Files\Adobe")));
        assert!(r.is_protected_lexical(Path::new(r"C:\Users\Public\Videos")), "hồ sơ người khác vẫn thuộc C:\\Users");
        assert!(!r.is_protected_lexical(Path::new(r"C:\Users\an\Downloads\abc")));
        assert!(!r.is_protected_lexical(Path::new(r"C:\Users\an\AppData\Local\Temp")));
        assert!(!r.is_protected_lexical(Path::new(r"D:\Games\old")));
        assert!(!r.is_protected_lexical(Path::new(r"C:\Windows.old")), "tên bắt đầu giống nhưng không nằm trong C:\\Windows");
    }

    #[test]
    fn special_files_at_any_drive_root_are_blocked() {
        let r = rules();
        for p in [r"C:\pagefile.sys", r"D:\hiberfil.sys", r"C:\swapfile.sys", r"E:\System Volume Information", r"E:\System Volume Information\x", r"C:\$Recycle.Bin\S-1-5", r"C:\$MFT"] {
            assert!(r.is_protected_lexical(Path::new(p)), "{p}");
        }
        assert!(!r.is_protected_lexical(Path::new(r"D:\data\pagefile.sys")), "chỉ chặn ở gốc ổ");
    }

    #[test]
    fn dot_dot_and_verbatim_prefix_cannot_sneak_past() {
        let r = rules();
        assert!(r.is_protected_lexical(Path::new(r"C:\Users\an\Downloads\..\..\..\Windows\Temp")));
        assert!(r.is_protected_lexical(Path::new(r"\\?\C:\Windows\Temp")));
        assert!(r.is_protected_lexical(Path::new(r"C:\Users\an\Downloads\..")));
    }

    #[test]
    fn junction_into_a_protected_dir_is_blocked_after_canonicalize() {
        let t = tempfile::tempdir().unwrap();
        let sys = t.path().join("Windows");
        let profile = t.path().join("Users").join("an");
        std::fs::create_dir_all(sys.join("System32")).unwrap();
        std::fs::create_dir_all(profile.join("Downloads")).unwrap();
        let link = profile.join("Downloads").join("link");
        junction::create(sys.join("System32"), &link).unwrap();
        let r = ProtectRules::new(std::slice::from_ref(&sys), &profile);
        assert!(!r.is_protected_lexical(&link));
        assert!(r.is_protected(&link));
        assert!(!r.is_protected(&profile.join("Downloads")));
    }

    #[test]
    fn rules_from_this_machine_block_windir() {
        let r = ProtectRules::from_env().unwrap();
        let windir = std::env::var_os("SystemRoot").unwrap();
        assert!(r.is_protected(Path::new(&windir)));
        assert!(!r.is_protected(&std::env::temp_dir().join("khong-co-9d1f")));
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test disk::protect`
Expected: FAIL biên dịch — `cannot find type ProtectRules`.

- [ ] **Step 3: Viết mã (trên khối test)**

```rust
//! Danh sách đường dẫn được bảo vệ (spec 3.3): hiện 🔒, mở xem được nhưng không xóa được.
use std::path::{Component, Path, PathBuf};

/// Tên đặc biệt ngay dưới gốc bất kỳ ổ nào.
const ROOT_SPECIAL: [&str; 5] = ["pagefile.sys", "hiberfil.sys", "swapfile.sys", "system volume information", "$recycle.bin"];

#[derive(Debug, Clone)]
pub struct ProtectRules {
    /// Chặn chính nó và mọi thứ bên trong: `%WINDIR%`, `Program Files`, `Program Files (x86)`, `ProgramData`, `C:\Users`.
    system_dirs: Vec<String>,
    /// `%USERPROFILE%`: chặn chính nó; con bên trong được xóa.
    user_profile: String,
}

impl ProtectRules {
    pub fn new(system_dirs: &[PathBuf], user_profile: &Path) -> Self {
        ProtectRules { system_dirs: system_dirs.iter().map(|p| key(p)).collect(), user_profile: key(user_profile) }
    }

    /// Luật của máy đang chạy, đọc từ biến môi trường.
    pub fn from_env() -> Result<Self, String> {
        fn var(name: &str) -> Result<PathBuf, String> {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .ok_or_else(|| format!("environment variable {name} is not set"))
        }
        let profile = var("USERPROFILE")?;
        let mut dirs = vec![var("SystemRoot").or_else(|_| var("windir"))?, var("ProgramData")?];
        dirs.extend(["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"].iter().filter_map(|v| var(v).ok()));
        if let Some(users) = profile.parent() {
            dirs.push(users.to_path_buf());
        }
        Ok(ProtectRules::new(&dirs, &profile))
    }

    /// Có bị chặn xóa không. Kiểm cả đường dẫn gõ vào lẫn đường dẫn thật sau `canonicalize`
    /// (junction trỏ vào thư mục hệ thống cũng bị chặn), bị chặn ở một trong hai là chặn.
    pub fn is_protected(&self, path: &Path) -> bool {
        if self.is_protected_lexical(path) {
            return true;
        }
        match std::fs::canonicalize(path) {
            Ok(real) => self.is_protected_lexical(&real),
            Err(_) => false,
        }
    }

    /// Chỉ so chuỗi (đã chuẩn hóa `..`, `.`, hoa thường, tiền tố `\\?\`) — không chạm đĩa.
    pub fn is_protected_lexical(&self, path: &Path) -> bool {
        let k = key(path);
        let parts: Vec<&str> = k.split('\\').filter(|s| !s.is_empty()).collect();
        // Gốc ổ đĩa ("c:") hoặc gốc UNC.
        if parts.len() <= 1 {
            return true;
        }
        if ROOT_SPECIAL.contains(&parts[1]) || parts[1].starts_with('$') {
            return true;
        }
        if k == self.user_profile {
            return true;
        }
        if is_within(&k, &self.user_profile) {
            return false;
        }
        self.system_dirs.iter().any(|d| k == *d || is_within(&k, d))
    }
}

fn is_within(k: &str, dir: &str) -> bool {
    k.len() > dir.len() && k.starts_with(dir) && k.as_bytes()[dir.len()] == b'\\'
}

/// Dạng so sánh: bỏ `\\?\`, gộp `.`/`..`, chữ thường, không có `\` cuối (trừ gốc ổ đĩa `c:`).
fn key(path: &Path) -> String {
    let s = path.to_string_lossy();
    let s = s.strip_prefix(r"\\?\UNC\").map(|r| format!(r"\\{r}")).unwrap_or_else(|| s.strip_prefix(r"\\?\").unwrap_or(&s).to_string());
    let mut out: Vec<String> = Vec::new();
    for c in Path::new(&s).components() {
        match c {
            Component::Prefix(p) => out.push(p.as_os_str().to_string_lossy().to_lowercase()),
            Component::RootDir | Component::CurDir => {}
            Component::ParentDir => {
                if out.len() > 1 {
                    out.pop();
                }
            }
            Component::Normal(n) => out.push(n.to_string_lossy().to_lowercase()),
        }
    }
    out.join("\\")
}
```

- [ ] **Step 4: Chạy test và clippy**

Run: `cargo test disk::protect`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
Expected: `6 passed`; toàn bộ `0 failed`; clippy sạch.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-diagnose/src/disk/protect.rs
rtk git commit -m "feat(diagnose): luật bảo vệ đường dẫn — gốc ổ, thư mục hệ thống, hồ sơ; kiểm cả sau canonicalize

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Danh sách ổ đĩa, chọn bộ quét, xóa vào Thùng rác

**Files:**
- Modify (thay dòng khung): `crates/winfreeup-diagnose/src/disk/volumes.rs`, `crates/winfreeup-diagnose/src/disk/recycle.rs`
- Test: `volumes.rs`, `recycle.rs`

**Interfaces:**
- Consumes (Task 1): `util::{wide, from_wide}`; crate `windows` 0.61 (COM).
- Produces:
  - `volumes::DriveKind { Fixed, Removable, Network, Cdrom, Ram, Unknown }` (serde chữ thường), `VolumeInfo { root: String /* "C:\\" */, label, fs, total: u64, free: u64, kind }`, `list_volumes() -> Vec<VolumeInfo>`, `volume_info(root: &str) -> Option<VolumeInfo>`, `drive_kind(code: u32) -> DriveKind`.
  - `volumes::ScanPlan` — serde `{"mode":"mft"}` | `{"mode":"walk","reason":WalkReason}`; `WalkReason` — `{"code":"not_ntfs","fs"}` | `{"code":"not_local","kind"}` | `{"code":"mft_failed","message"}`; `plan_for(v: &VolumeInfo) -> ScanPlan` (NTFS + Fixed/Removable ⇒ Mft).
  - `recycle::recycle(path: &Path) -> winfreeup_core::Result<()>` (luồng STA riêng; người dùng bấm Hủy ở cảnh báo xóa hẳn ⇒ `Err(System("aborted"))`; xong mà đường dẫn vẫn còn ⇒ lỗi), `recycle::recycle_flags() -> u32`.

- [ ] **Step 1: Viết test hỏng** — thay dòng khung của hai file bằng phần test:

`volumes.rs`:

```rust
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
        let sys = format!("{}\\", std::env::var("SystemDrive").unwrap());
        let all = list_volumes();
        let c = all.iter().find(|v| v.root.eq_ignore_ascii_case(&sys)).expect("ổ hệ thống");
        assert!(c.total > 0 && c.free <= c.total);
        assert_eq!(c.kind, DriveKind::Fixed);
        assert!(!c.fs.is_empty());
    }
}
```

`recycle.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_recycle_and_still_warn_before_permanent_delete() {
        let f = recycle_flags();
        assert_ne!(f & FOFX_RECYCLEONDELETE.0, 0);
        assert_ne!(f & FOF_ALLOWUNDO.0, 0);
        assert_ne!(f & FOF_WANTNUKEWARNING.0, 0);
    }

    #[test]
    fn missing_path_is_an_error_not_a_panic() {
        let t = tempfile::tempdir().unwrap();
        assert!(recycle(&t.path().join("khong-co.txt")).is_err());
    }

    /// Chạm Thùng rác thật của máy — chạy tay: `cargo test -- --ignored recycle_moves`.
    #[test]
    #[ignore]
    fn recycle_moves_a_folder_to_the_recycle_bin() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("WinFreeUp-thu-thung-rac");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "x").unwrap();
        recycle(&dir).unwrap();
        assert!(!dir.exists());
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test disk::volumes` rồi `cargo test disk::recycle`
Expected: FAIL biên dịch — `cannot find function plan_for`, `cannot find function recycle_flags`.

- [ ] **Step 3: Viết mã (trên khối test)**

`volumes.rs`:

```rust
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
        kind: drive_kind(unsafe { GetDriveTypeW(w.as_ptr()) }),
    })
}

/// Mọi ổ có ký tự, theo thứ tự chữ cái.
pub fn list_volumes() -> Vec<VolumeInfo> {
    let mask = unsafe { GetLogicalDrives() };
    (0..26u8)
        .filter(|i| mask & (1 << i) != 0)
        .filter_map(|i| volume_info(&format!("{}:\\", (b'A' + i) as char)))
        .collect()
}
```

`recycle.rs`:

```rust
//! Xóa vào Thùng rác bằng `IFileOperation` (spec 3.3).
use std::path::{Path, PathBuf};

use windows::core::HSTRING;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
use windows::Win32::UI::Shell::{
    FileOperation, IFileOperation, IShellItem, SHCreateItemFromParsingName, FILEOPERATION_FLAGS, FOFX_RECYCLEONDELETE,
    FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FOF_WANTNUKEWARNING,
};
use winfreeup_core::{CoreError, Result};

/// Không hỏi «có chắc không» (giao diện đã hỏi), không hiện hộp tiến độ, nhưng **vẫn cảnh báo** nếu
/// Windows định xóa hẳn thay vì cho vào Thùng rác (file quá lớn, ổ USB/mạng không có Thùng rác).
pub fn recycle_flags() -> u32 {
    (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI | FOF_WANTNUKEWARNING).0 | FOFX_RECYCLEONDELETE.0
}

/// Cho `path` vào Thùng rác. Chạy trên một luồng STA riêng (yêu cầu của shell), chờ xong mới trả.
/// Người dùng bấm Hủy ở cảnh báo xóa hẳn ⇒ `Err("aborted")`, không có gì bị xóa.
pub fn recycle(path: &Path) -> Result<()> {
    let p: PathBuf = path.to_path_buf();
    let joined = std::thread::spawn(move || -> std::result::Result<(), String> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok().map_err(|e| e.to_string())?;
            let result = (|| -> windows::core::Result<bool> {
                let op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)?;
                op.SetOperationFlags(FILEOPERATION_FLAGS(recycle_flags()))?;
                let item: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(p.as_path()), None)?;
                op.DeleteItem(&item, None)?;
                op.PerformOperations()?;
                Ok(op.GetAnyOperationsAborted()?.as_bool())
            })();
            CoUninitialize();
            match result {
                Ok(false) => Ok(()),
                Ok(true) => Err("aborted".to_string()),
                Err(e) => Err(format!("{} (0x{:08X})", e.message(), e.code().0 as u32)),
            }
        }
    })
    .join()
    .map_err(|_| CoreError::System("recycle thread panicked".into()))?;
    joined.map_err(CoreError::System)?;
    if std::fs::symlink_metadata(path).is_ok() {
        return Err(CoreError::System(format!("still exists after recycle: {}", path.display())));
    }
    Ok(())
}
```

- [ ] **Step 4: Chạy test và clippy**

Run: `cargo test disk::volumes`, `cargo test disk::recycle`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
Expected: volumes `4 passed`; recycle `2 passed; 1 ignored`; toàn bộ `0 failed`.

- [ ] **Step 5: Thử tay một lần test bị `#[ignore]` (chạm Thùng rác thật của máy dev)**

Run: `cargo test -- --ignored recycle_moves`
Expected: `1 passed`; thư mục `WinFreeUp-thu-thung-rac` xuất hiện trong Thùng rác (đã đo khi viết kế hoạch: có mặt ở `C:\$Recycle.Bin\<SID>\$R…`). Xóa hẳn mục đó khỏi Thùng rác sau khi xem.

- [ ] **Step 6: Commit**

```bash
rtk git add crates/winfreeup-diagnose/src/disk/volumes.rs
rtk git add crates/winfreeup-diagnose/src/disk/recycle.rs
rtk git commit -m "feat(diagnose): danh sách ổ, chọn MFT/duyệt thư mục, xóa vào Thùng rác bằng IFileOperation

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Hàm thuần hiệu năng — cơn giật, hạ xung, nhiệt độ, giãn chu kỳ

**Files:**
- Modify (thay dòng khung): `crates/winfreeup-diagnose/src/perf/detect.rs`
- Test: `detect.rs`

**Interfaces:**
- Consumes: không.
- Produces:
  - Hằng: `STUTTER_PCT = 90.0`, `STUTTER_MIN_SAMPLES = 3`, `MERGE_GAP_MS = 2000`, `THROTTLE_LOAD_PCT = 80.0`, `THROTTLE_PERF_PCT = 70.0`, `THROTTLE_MIN_MS = 10_000`, `TEMP_MIN_C = 20.0`, `TEMP_MAX_C = 110.0`, `TEMP_STUCK_MS = 60_000`, `SELF_CPU_BUDGET_PCT = 2.0`, `SLOW_INTERVAL_MS = 2000`.
  - `Point { t_ms: u64, dur_ms: u64, cpu: f32, disk: f32, perf: Option<f32> }` — mẫu kết thúc lúc `t_ms`, đại diện `dur_ms` trước đó.
  - `Span { start_ms, end_ms, first, last, cpu: bool, disk: bool }`; `find_stutters(&[Point]) -> Vec<Span>`; `find_throttle(&[Point]) -> Vec<Span>`; `throttled_now(&[Point]) -> Option<bool>` (`None` khi mẫu mới nhất không có `perf`).
  - `AppLoad { key: Arc<str>, name: Arc<str>, cpu: f32, disk_bps: f64 }`, `TopApp { key, name, avg: f64 }`, `top_apps(frames: &[&[AppLoad]], by_cpu: bool, n: usize) -> Vec<TopApp>`.
  - `TempState` — serde `{"state":"ok","celsius"}` | `{"state":"unavailable","code","detail"}`; `TempTracker::default()`, `push(&mut self, t_ms: u64, reading: Result<f32, String>) -> TempState`.
  - `next_interval_ms(current_ms: u32, own_cpu_pct: f32) -> u32`.

- [ ] **Step 1: Viết test hỏng** — thay dòng khung bằng:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Chuỗi mẫu 1 giây/mẫu: `c` = CPU nóng, `d` = đĩa nóng, `.` = bình thường.
    fn series(pattern: &str) -> Vec<Point> {
        pattern
            .chars()
            .enumerate()
            .map(|(i, ch)| Point {
                t_ms: (i as u64 + 1) * 1000,
                dur_ms: 1000,
                cpu: if ch == 'c' { 95.0 } else { 10.0 },
                disk: if ch == 'd' { 99.0 } else { 5.0 },
                perf: Some(100.0),
            })
            .collect()
    }

    #[test]
    fn exactly_three_seconds_is_a_stutter_two_is_not() {
        let s = find_stutters(&series("..ccc.."));
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].start_ms, s[0].end_ms), (2000, 5000));
        assert!(s[0].cpu && !s[0].disk);
        assert!(find_stutters(&series("..cc...")).is_empty());
    }

    #[test]
    fn exactly_ninety_percent_is_not_hot() {
        let mut p = series("ccc");
        for x in &mut p {
            x.cpu = 90.0;
        }
        assert!(find_stutters(&p).is_empty());
    }

    #[test]
    fn cpu_or_disk_counts_and_both_flags_are_kept() {
        let s = find_stutters(&series("cdc"));
        assert_eq!(s.len(), 1);
        assert!(s[0].cpu && s[0].disk);
    }

    #[test]
    fn two_stutters_one_second_apart_merge() {
        let s = find_stutters(&series("ccc.ddd"));
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].first, s[0].last), (0, 6));
        assert!(s[0].cpu && s[0].disk);
    }

    #[test]
    fn exactly_two_seconds_apart_stay_separate_and_three_too() {
        assert_eq!(find_stutters(&series("ccc..ccc")).len(), 2);
        assert_eq!(find_stutters(&series("ccc...ccc")).len(), 2);
    }

    #[test]
    fn a_gap_in_sampling_breaks_the_run() {
        let mut p = series("cccc");
        // Rời mục 30 giây rồi quay lại: mẫu thứ 3 không liền với mẫu thứ 2.
        p[2].t_ms += 30_000;
        p[3].t_ms += 30_000;
        assert!(find_stutters(&p).is_empty());
    }

    #[test]
    fn slower_two_second_sampling_still_needs_three_samples() {
        let p: Vec<Point> = (0..3).map(|i| Point { t_ms: (i + 1) * 2000, dur_ms: 2000, cpu: 99.0, disk: 0.0, perf: None }).collect();
        assert_eq!(find_stutters(&p).len(), 1);
        assert_eq!(find_stutters(&p[..2]).len(), 0);
    }

    fn throttle_series(n: usize, load: f32, perf: Option<f32>) -> Vec<Point> {
        (0..n).map(|i| Point { t_ms: (i as u64 + 1) * 1000, dur_ms: 1000, cpu: load, disk: 0.0, perf }).collect()
    }

    #[test]
    fn throttle_needs_ten_full_seconds() {
        assert_eq!(find_throttle(&throttle_series(10, 95.0, Some(50.0))).len(), 1);
        assert!(find_throttle(&throttle_series(9, 95.0, Some(50.0))).is_empty());
        assert_eq!(throttled_now(&throttle_series(10, 95.0, Some(50.0))), Some(true));
        assert_eq!(throttled_now(&throttle_series(9, 95.0, Some(50.0))), Some(false));
    }

    #[test]
    fn throttle_boundaries_and_missing_counter() {
        assert!(find_throttle(&throttle_series(12, 80.0, Some(50.0))).is_empty(), "tải đúng 80% chưa tính");
        assert!(find_throttle(&throttle_series(12, 95.0, Some(70.0))).is_empty(), "hiệu năng đúng 70% chưa tính");
        assert_eq!(throttled_now(&throttle_series(12, 95.0, None)), None, "không có bộ đếm ⇒ ẩn chỉ báo");
        assert_eq!(throttled_now(&[]), None);
    }

    #[test]
    fn top_three_apps_by_the_metric_that_spiked() {
        let a = |k: &str, cpu: f32, disk: f64| AppLoad { key: k.into(), name: k.to_uppercase().into(), cpu, disk_bps: disk };
        let f1 = vec![a("chrome", 60.0, 0.0), a("defender", 30.0, 9e6), a("x", 1.0, 0.0), a("y", 2.0, 0.0)];
        let f2 = vec![a("chrome", 40.0, 0.0), a("defender", 50.0, 7e6), a("y", 2.0, 0.0)];
        let frames: Vec<&[AppLoad]> = vec![&f1, &f2];
        let cpu = top_apps(&frames, true, 3);
        assert_eq!(cpu.iter().map(|t| t.key.as_str()).collect::<Vec<_>>(), vec!["chrome", "defender", "y"]);
        assert_eq!(cpu[0].avg, 50.0);
        assert_eq!(cpu[0].name, "CHROME");
        let disk = top_apps(&frames, false, 3);
        assert_eq!(disk.len(), 1, "app không có đĩa thì không liệt kê");
        assert_eq!(disk[0].avg, 8e6);
    }

    #[test]
    fn temperature_is_trusted_only_in_range_and_while_moving() {
        let mut t = TempTracker::default();
        assert_eq!(t.push(0, Ok(55.0)), TempState::Ok { celsius: 55.0 });
        assert_eq!(t.push(59_000, Ok(55.0)), TempState::Ok { celsius: 55.0 });
        assert_eq!(t.push(60_000, Ok(55.0)), TempState::Unavailable { code: "stuck".into(), detail: "55.0".into() });
        assert_eq!(t.push(61_000, Ok(56.0)), TempState::Ok { celsius: 56.0 });
        assert!(matches!(t.push(62_000, Ok(19.9)), TempState::Unavailable { ref code, .. } if code == "out_of_range"));
        assert!(matches!(t.push(63_000, Ok(110.1)), TempState::Unavailable { ref code, .. } if code == "out_of_range"));
        assert_eq!(t.push(64_000, Ok(110.0)), TempState::Ok { celsius: 110.0 });
        assert_eq!(
            t.push(65_000, Err("Invalid class".into())),
            TempState::Unavailable { code: "no_sensor".into(), detail: "Invalid class".into() }
        );
    }

    #[test]
    fn sampling_slows_down_only_when_over_budget() {
        assert_eq!(next_interval_ms(1000, 1.5), 1000);
        assert_eq!(next_interval_ms(1000, 2.0), 1000);
        assert_eq!(next_interval_ms(1000, 2.1), 2000);
        assert_eq!(next_interval_ms(2000, 0.1), 2000);
    }

    #[test]
    fn spans_serialize_for_the_ui() {
        let v = serde_json::to_value(TempState::Ok { celsius: 50.0 }).unwrap();
        assert_eq!(v, serde_json::json!({"state": "ok", "celsius": 50.0}));
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test perf::detect`
Expected: FAIL biên dịch — `cannot find function find_stutters`.

- [ ] **Step 3: Viết mã (trên khối test)**

```rust
//! Hàm thuần trên chuỗi mẫu (spec 5.3–5.5): cơn giật, hạ xung, nhiệt độ đáng tin, giãn chu kỳ lấy mẫu.
use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;

/// Cơn giật: CPU hoặc Hoạt động đĩa vượt ngưỡng này…
pub const STUTTER_PCT: f32 = 90.0;
/// …trong ít nhất chừng này mẫu liên tiếp.
pub const STUTTER_MIN_SAMPLES: usize = 3;
/// Hai cơn cách nhau dưới 2 giây thì gộp.
pub const MERGE_GAP_MS: u64 = 2_000;
/// Hạ xung: tải CPU > 80% và hiệu năng < 70% liên tục ≥ 10 giây.
pub const THROTTLE_LOAD_PCT: f32 = 80.0;
pub const THROTTLE_PERF_PCT: f32 = 70.0;
pub const THROTTLE_MIN_MS: u64 = 10_000;
/// Nhiệt độ hợp lệ 20–110 °C; đứng yên suốt 60 giây ⇒ cảm biến giả.
pub const TEMP_MIN_C: f32 = 20.0;
pub const TEMP_MAX_C: f32 = 110.0;
pub const TEMP_STUCK_MS: u64 = 60_000;
/// Chi phí của chính WinFreeUp vượt 2% CPU ⇒ giãn chu kỳ còn 2 giây.
pub const SELF_CPU_BUDGET_PCT: f32 = 2.0;
pub const SLOW_INTERVAL_MS: u32 = 2_000;

/// Một mẫu: thời điểm kết thúc `t_ms`, mẫu đại diện cho `dur_ms` trước đó.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub t_ms: u64,
    pub dur_ms: u64,
    pub cpu: f32,
    pub disk: f32,
    /// % Processor Performance; `None` khi bộ đếm không có.
    pub perf: Option<f32>,
}

impl Point {
    fn start_ms(&self) -> u64 {
        self.t_ms.saturating_sub(self.dur_ms)
    }
}

/// Hai mẫu kề nhau có liền mạch không (không có khoảng dừng lấy mẫu ở giữa).
fn contiguous(prev: &Point, cur: &Point) -> bool {
    cur.t_ms.saturating_sub(prev.t_ms) <= cur.dur_ms + cur.dur_ms / 2
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Span {
    pub start_ms: u64,
    pub end_ms: u64,
    /// Chỉ số mẫu đầu/cuối (gồm cả hai) trong chuỗi đã đưa vào.
    pub first: usize,
    pub last: usize,
    pub cpu: bool,
    pub disk: bool,
}

/// Các đoạn liên tiếp thỏa `hot`, dài ít nhất `min_samples` mẫu.
fn runs(points: &[Point], min_samples: usize, hot: impl Fn(&Point) -> bool) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for i in 0..=points.len() {
        let continues = i < points.len() && hot(&points[i]) && start.is_none_or(|_| contiguous(&points[i - 1], &points[i]));
        match (start, continues) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                if i - s >= min_samples {
                    out.push((s, i - 1));
                }
                start = if i < points.len() && hot(&points[i]) { Some(i) } else { None };
            }
            _ => {}
        }
    }
    out
}

/// Spec 5.3: cơn giật = CPU hoặc Đĩa > 90% trong ≥ 3 mẫu liên tiếp; hai cơn cách < 2 giây gộp làm một.
pub fn find_stutters(points: &[Point]) -> Vec<Span> {
    let hot = |p: &Point| p.cpu > STUTTER_PCT || p.disk > STUTTER_PCT;
    let mut out: Vec<Span> = Vec::new();
    for (a, b) in runs(points, STUTTER_MIN_SAMPLES, hot) {
        let slice = &points[a..=b];
        let span = Span {
            start_ms: points[a].start_ms(),
            end_ms: points[b].t_ms,
            first: a,
            last: b,
            cpu: slice.iter().any(|p| p.cpu > STUTTER_PCT),
            disk: slice.iter().any(|p| p.disk > STUTTER_PCT),
        };
        match out.last_mut() {
            Some(prev) if span.start_ms.saturating_sub(prev.end_ms) < MERGE_GAP_MS => {
                prev.end_ms = span.end_ms;
                prev.last = span.last;
                prev.cpu |= span.cpu;
                prev.disk |= span.disk;
            }
            _ => out.push(span),
        }
    }
    out
}

/// Spec 5.4: các đoạn hạ xung (tải > 80% và hiệu năng < 70% liên tục ≥ 10 giây).
pub fn find_throttle(points: &[Point]) -> Vec<Span> {
    let hot = |p: &Point| p.cpu > THROTTLE_LOAD_PCT && p.perf.is_some_and(|v| v < THROTTLE_PERF_PCT);
    runs(points, 1, hot)
        .into_iter()
        .map(|(a, b)| Span { start_ms: points[a].start_ms(), end_ms: points[b].t_ms, first: a, last: b, cpu: true, disk: false })
        .filter(|s| s.end_ms - s.start_ms >= THROTTLE_MIN_MS)
        .collect()
}

/// Trạng thái hạ xung ở mẫu mới nhất: `None` khi bộ đếm hiệu năng không có (ẩn chỉ báo).
pub fn throttled_now(points: &[Point]) -> Option<bool> {
    let last = points.last()?;
    last.perf?;
    Some(find_throttle(points).last().is_some_and(|s| s.last == points.len() - 1))
}

/// Tải của một app trong một mẫu (dùng để chọn 3 app ngốn nhất trong cơn).
#[derive(Debug, Clone, PartialEq)]
pub struct AppLoad {
    pub key: Arc<str>,
    pub name: Arc<str>,
    pub cpu: f32,
    pub disk_bps: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TopApp {
    pub key: String,
    pub name: String,
    /// Trung bình trong cơn: % CPU hoặc byte/giây đĩa.
    pub avg: f64,
}

/// 3 app ngốn nhất trong các khung mẫu của cơn, theo trung bình (khung vắng app tính 0).
pub fn top_apps(frames: &[&[AppLoad]], by_cpu: bool, n: usize) -> Vec<TopApp> {
    let mut sum: HashMap<&str, (f64, &str)> = HashMap::new();
    for f in frames {
        for a in f.iter() {
            let v = if by_cpu { f64::from(a.cpu) } else { a.disk_bps };
            let e = sum.entry(&a.key).or_insert((0.0, &a.name));
            e.0 += v;
        }
    }
    let count = frames.len().max(1) as f64;
    let mut v: Vec<TopApp> = sum
        .into_iter()
        .filter(|(_, (s, _))| *s > 0.0)
        .map(|(k, (s, name))| TopApp { key: k.to_string(), name: name.to_string(), avg: s / count })
        .collect();
    v.sort_by(|a, b| b.avg.total_cmp(&a.avg).then_with(|| a.key.cmp(&b.key)));
    v.truncate(n);
    v
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TempState {
    Ok { celsius: f32 },
    /// `code`: `no_sensor` (không có lớp WMI / không đọc được — `detail` nguyên văn), `out_of_range`, `stuck`.
    Unavailable { code: String, detail: String },
}

/// Spec 5.5: loại số đo ngoài 20–110 °C và cảm biến đứng yên suốt 60 giây.
#[derive(Debug, Default)]
pub struct TempTracker {
    last: Option<f32>,
    same_since: u64,
}

impl TempTracker {
    pub fn push(&mut self, t_ms: u64, reading: std::result::Result<f32, String>) -> TempState {
        let c = match reading {
            Err(detail) => {
                self.last = None;
                return TempState::Unavailable { code: "no_sensor".into(), detail };
            }
            Ok(c) => c,
        };
        if !(TEMP_MIN_C..=TEMP_MAX_C).contains(&c) {
            self.last = None;
            return TempState::Unavailable { code: "out_of_range".into(), detail: format!("{c:.1}") };
        }
        match self.last {
            Some(prev) if (prev - c).abs() < 0.05 => {}
            _ => {
                self.last = Some(c);
                self.same_since = t_ms;
            }
        }
        if t_ms.saturating_sub(self.same_since) >= TEMP_STUCK_MS {
            TempState::Unavailable { code: "stuck".into(), detail: format!("{c:.1}") }
        } else {
            TempState::Ok { celsius: c }
        }
    }
}

/// Spec 5: chi phí của chính mình vượt 2% CPU ⇒ giãn chu kỳ còn 2 giây (không tự rút ngắn lại trong phiên).
pub fn next_interval_ms(current_ms: u32, own_cpu_pct: f32) -> u32 {
    if own_cpu_pct > SELF_CPU_BUDGET_PCT {
        SLOW_INTERVAL_MS.max(current_ms)
    } else {
        current_ms
    }
}
```

- [ ] **Step 4: Chạy test và clippy**

Run: `cargo test perf::detect`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
Expected: `13 passed`; toàn bộ `0 failed`.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-diagnose/src/perf/detect.rs
rtk git commit -m "feat(diagnose): phát hiện cơn giật, hạ xung, nhiệt độ đáng tin, giãn chu kỳ — hàm thuần

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Tiến trình, gộp theo app, luật tiến trình thiết yếu, biểu tượng

**Files:**
- Modify (thay dòng khung): `crates/winfreeup-diagnose/src/perf/procs.rs`, `perf/apps.rs`, `perf/icon.rs`
- Test: ba file trên

**Interfaces:**
- Consumes (Task 1): `util::{wide, from_wide}`; `winfreeup_core::{CoreError, Result}`.
- Produces:
  - `procs::ProcInfo { pid, parent_pid, name, create_time: i64, private_ws: u64, cpu_100ns: u64, io_bytes: u64 }`, `snapshot() -> Result<Vec<ProcInfo>>` (một lời gọi `NtQuerySystemInformation`), `exe_path(pid) -> Option<String>`, `PathCache { get(&mut self, pid, create_time) -> Option<String>, retain(&mut self, alive: &HashSet<(u32, i64)>) }`, `kill(pid, create_time) -> Result<()>` (từ chối nếu giờ tạo khác — pid đã bị dùng lại), `own_cpu_100ns() -> u64`.
  - `apps::ESSENTIAL_NAMES`, `EssentialRules::new(system32: &Path, self_pid: u32)`, `is_essential(&self, pid, parent_pid, name, path: Option<&str>) -> bool`; `ProcSample { pid, parent_pid, name, path, description, create_time, private_ws, cpu_100ns, io_bytes, net: Option<(u64, u64)> }`; `ProcRow`, `AppRow { key, name, path, ram, cpu, disk_bps, net_up_bps, net_down_bps, essential, procs }` (Serialize); `type PrevTotals = HashMap<(u32, i64), (u64, u64)>`; `aggregate(prev, cur: &[ProcSample], dt_ms, cpus, rules) -> (Vec<AppRow>, PrevTotals)` (sắp RAM giảm dần); `app_key(name, path) -> String`.
  - `icon::file_description(path: &Path) -> String`, `icon::icon_data_url(path: &Path) -> Option<String>` (`data:image/png;base64,…`, 32×32), `icon::base64(&[u8]) -> String`, `icon::bgra_to_png(w, h, bgra, mask) -> Option<Vec<u8>>`.

- [ ] **Step 1: Viết test hỏng** — thay dòng khung của ba file bằng phần test:

`procs.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;

    #[test]
    fn layout_offsets_match_the_documented_ones() {
        assert_eq!(std::mem::offset_of!(SpiEntry, image_name), 0x38);
        assert_eq!(std::mem::offset_of!(SpiEntry, unique_process_id), 0x50);
        assert_eq!(std::mem::offset_of!(SpiEntry, read_transfer_count), 0xE8);
        assert_eq!(std::mem::size_of::<SpiEntry>(), 0x100);
    }

    #[test]
    fn snapshot_contains_this_test_process_with_sane_numbers() {
        let me = std::process::id();
        let all = snapshot().unwrap();
        assert!(all.len() > 10);
        assert!(all.iter().any(|p| p.pid == 4 && p.name == "System"));
        let p = all.iter().find(|p| p.pid == me).expect("tiến trình test");
        let exe = std::env::current_exe().unwrap();
        assert!(p.name.eq_ignore_ascii_case(&exe.file_name().unwrap().to_string_lossy()));
        assert!(p.private_ws > 0);
        assert!(p.create_time > 0);
        assert_eq!(exe_path(me).unwrap().to_lowercase(), exe.display().to_string().to_lowercase());
    }

    #[test]
    fn io_counter_grows_when_we_write() {
        let me = std::process::id();
        let before = snapshot().unwrap().into_iter().find(|p| p.pid == me).unwrap().io_bytes;
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join("x.bin"), vec![7u8; 2_000_000]).unwrap();
        let after = snapshot().unwrap().into_iter().find(|p| p.pid == me).unwrap().io_bytes;
        assert!(after >= before + 2_000_000, "{before} -> {after}");
    }

    #[test]
    fn kill_ends_our_own_child_and_refuses_a_stale_create_time() {
        let mut child = std::process::Command::new("ping")
            .args(["-n", "60", "127.0.0.1"])
            .creation_flags(0x0800_0000)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        let info = snapshot().unwrap().into_iter().find(|p| p.pid == pid).unwrap();
        assert!(kill(pid, info.create_time + 1).is_err(), "giờ tạo khác ⇒ không phải tiến trình đã thấy");
        kill(pid, info.create_time).unwrap();
        let status = child.wait().unwrap();
        assert!(!status.success());
    }

    #[test]
    fn own_cpu_time_is_counted() {
        let a = own_cpu_100ns();
        let mut x = 0u64;
        for i in 0..20_000_000u64 {
            x = x.wrapping_add(i * i);
        }
        std::hint::black_box(x);
        assert!(own_cpu_100ns() > a);
    }

    #[test]
    fn path_cache_remembers_and_forgets() {
        let mut c = PathCache::default();
        let me = std::process::id();
        assert!(c.get(me, 1).is_some());
        c.retain(&HashSet::new());
        assert!(c.map.is_empty());
    }
}
```

`apps.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> EssentialRules {
        EssentialRules::new(Path::new(r"C:\Windows\System32"), 999)
    }

    #[test]
    fn essential_by_name_and_system32_location() {
        let r = rules();
        assert!(r.is_essential(700, 600, "svchost.exe", Some(r"C:\Windows\System32\svchost.exe")));
        assert!(r.is_essential(700, 600, "LSASS.EXE", Some(r"c:\windows\system32\lsass.exe")));
        assert!(r.is_essential(88, 4, "Registry", None), "tiến trình nhân không có đường dẫn");
        assert!(r.is_essential(4, 0, "System", None));
        assert!(r.is_essential(710, 600, "csrss.exe", None), "không đọc được đường dẫn ⇒ chặn cho chắc");
    }

    #[test]
    fn same_name_outside_system32_can_be_ended() {
        let r = rules();
        assert!(!r.is_essential(701, 600, "svchost.exe", Some(r"C:\Temp\svchost.exe")));
        assert!(!r.is_essential(702, 600, "dwm.exe", Some(r"C:\Windows\System32\sub\dwm.exe")), "phải nằm ngay trong System32");
        assert!(!r.is_essential(703, 600, "chrome.exe", Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe")));
    }

    #[test]
    fn winfreeup_itself_and_its_webview_are_protected() {
        let r = rules();
        assert!(r.is_essential(999, 1, "WinFreeUp.exe", Some(r"D:\WinFreeUp.exe")));
        assert!(r.is_essential(1234, 999, "msedgewebview2.exe", Some(r"C:\Program Files (x86)\Microsoft\EdgeWebView\msedgewebview2.exe")));
    }

    fn sample(pid: u32, name: &str, path: Option<&str>, ram: u64, cpu: u64, io: u64) -> ProcSample {
        ProcSample {
            pid,
            parent_pid: 1,
            name: name.into(),
            path: path.map(Into::into),
            description: String::new(),
            create_time: 10,
            private_ws: ram,
            cpu_100ns: cpu,
            io_bytes: io,
            net: None,
        }
    }

    #[test]
    fn processes_group_by_exe_path_and_rates_come_from_deltas() {
        let chrome = r"C:\Program Files\Google\Chrome\Application\chrome.exe";
        let mut prev = PrevTotals::new();
        prev.insert((1, 10), (0, 0));
        prev.insert((2, 10), (0, 1_000_000));
        let mut a = sample(1, "chrome.exe", Some(chrome), 300, 5_000_000, 0);
        a.description = "Google Chrome".into();
        let b = sample(2, "chrome.exe", Some(&chrome.to_uppercase()), 200, 5_000_000, 3_000_000);
        let c = sample(3, "notepad.exe", Some(r"C:\Windows\notepad.exe"), 900, 1, 1);
        // 1 giây, 2 lõi ⇒ năng lực 2e7 × 100 ns; mỗi tiến trình chrome dùng 5e6 ⇒ 25%.
        let (rows, next) = aggregate(&prev, &[a, b, c], 1000, 2, &rules());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "notepad", "sắp theo RAM giảm dần; không có FileDescription thì dùng tên file");
        assert_eq!(rows[0].cpu, 0.0, "tiến trình mới chưa có số lần trước");
        let c_row = &rows[1];
        assert_eq!(c_row.name, "Google Chrome");
        assert_eq!(c_row.ram, 500);
        assert_eq!(c_row.cpu, 50.0);
        assert_eq!(c_row.disk_bps, 2_000_000.0);
        assert_eq!(c_row.procs.len(), 2);
        assert_eq!(c_row.procs[0].pid, 1);
        assert_eq!(next.get(&(2, 10)), Some(&(5_000_000, 3_000_000)));
    }

    #[test]
    fn a_reused_pid_is_not_mistaken_for_the_old_process() {
        let mut prev = PrevTotals::new();
        prev.insert((5, 1), (9_000_000, 9_000_000));
        let mut p = sample(5, "x.exe", Some(r"C:\x.exe"), 1, 100, 100);
        p.create_time = 2;
        let (rows, _) = aggregate(&prev, &[p], 1000, 1, &rules());
        assert_eq!(rows[0].cpu, 0.0);
        assert_eq!(rows[0].disk_bps, 0.0);
    }

    #[test]
    fn network_columns_stay_empty_without_etw_and_sum_with_it() {
        let (rows, _) = aggregate(&PrevTotals::new(), &[sample(1, "a.exe", None, 1, 0, 0)], 1000, 1, &rules());
        assert_eq!(rows[0].net_down_bps, None);
        assert_eq!(rows[0].key, "name:a.exe");
        let mut p1 = sample(1, "a.exe", Some(r"C:\a.exe"), 1, 0, 0);
        let mut p2 = sample(2, "a.exe", Some(r"C:\a.exe"), 1, 0, 0);
        p1.net = Some((1000, 4000));
        p2.net = Some((0, 2000));
        let (rows, _) = aggregate(&PrevTotals::new(), &[p1, p2], 2000, 1, &rules());
        assert_eq!(rows[0].net_up_bps, Some(500.0));
        assert_eq!(rows[0].net_down_bps, Some(3000.0));
    }

    #[test]
    fn one_essential_process_locks_the_whole_app() {
        let s32 = r"C:\Windows\System32\svchost.exe";
        let (rows, _) = aggregate(&PrevTotals::new(), &[sample(1, "svchost.exe", Some(s32), 1, 0, 0)], 1000, 1, &rules());
        assert!(rows[0].essential && rows[0].procs[0].essential);
    }
}
```

`icon.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn notepad() -> std::path::PathBuf {
        Path::new(&std::env::var("SystemRoot").unwrap()).join("System32").join("notepad.exe")
    }

    #[test]
    fn base64_matches_rfc4648_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn png_encoding_keeps_alpha_or_falls_back_to_mask() {
        let px = [10u8, 20, 30, 0, 40, 50, 60, 0];
        let png = bgra_to_png(2, 1, &px, Some(&[true, false])).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        let mut dec = png::Decoder::new(std::io::Cursor::new(png)).read_info().unwrap();
        let mut buf = vec![0; dec.output_buffer_size().unwrap()];
        dec.next_frame(&mut buf).unwrap();
        assert_eq!(&buf[..8], &[30, 20, 10, 255, 60, 50, 40, 0]);
    }

    #[test]
    fn system_exe_has_description_and_icon() {
        assert!(!file_description(&notepad()).is_empty());
        let url = icon_data_url(&notepad()).unwrap();
        assert!(url.starts_with("data:image/png;base64,iVBORw0KGgo"));
    }

    #[test]
    fn missing_file_gives_empty_description() {
        assert_eq!(file_description(Path::new(r"C:\khong-co\x.exe")), "");
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test perf::`
Expected: FAIL biên dịch — `cannot find function snapshot`, `cannot find type EssentialRules`, `cannot find function base64`.

- [ ] **Step 3: Viết mã (trên khối test)**

`procs.rs`:

```rust
//! Ảnh chụp mọi tiến trình trong MỘT lời gọi `NtQuerySystemInformation(SystemProcessInformation)` —
//! cách Task Manager làm: có RAM riêng (private working set), thời gian CPU, byte vào/ra, không phải mở
//! từng tiến trình. Đường dẫn exe mới cần mở tiến trình (quyền tối thiểu), nên được nhớ theo (pid, giờ tạo).
use std::collections::{HashMap, HashSet};

use windows_sys::Wdk::System::SystemInformation::{NtQuerySystemInformation, SystemProcessInformation};
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, TerminateProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
};
use winfreeup_core::{CoreError, Result};

const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;

#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *const u16,
}

/// Bố cục đầy đủ của SYSTEM_PROCESS_INFORMATION (x64). Bản tài liệu công khai giấu các trường này dưới
/// tên `Reserved…` nhưng vị trí không đổi từ Windows 7; test `layout_offsets_match_the_documented_ones`
/// giữ lại hai mốc công khai (`ImageName` 0x38, `UniqueProcessId` 0x50).
#[repr(C)]
struct SpiEntry {
    next_entry_offset: u32,
    number_of_threads: u32,
    working_set_private_size: i64,
    hard_fault_count: u32,
    number_of_threads_high_watermark: u32,
    cycle_time: u64,
    create_time: i64,
    user_time: i64,
    kernel_time: i64,
    image_name: UnicodeString,
    base_priority: i32,
    unique_process_id: usize,
    inherited_from_unique_process_id: usize,
    handle_count: u32,
    session_id: u32,
    unique_process_key: usize,
    peak_virtual_size: usize,
    virtual_size: usize,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
    private_page_count: usize,
    read_operation_count: i64,
    write_operation_count: i64,
    other_operation_count: i64,
    read_transfer_count: i64,
    write_transfer_count: i64,
    other_transfer_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcInfo {
    pub pid: u32,
    pub parent_pid: u32,
    /// Tên ảnh (vd `chrome.exe`; `System`, `Registry` với tiến trình nhân).
    pub name: String,
    /// FILETIME lúc tạo — cùng pid để phân biệt tiến trình đã chết và pid bị dùng lại.
    pub create_time: i64,
    /// Byte RAM riêng (private working set) — cột RAM của spec.
    pub private_ws: u64,
    /// Tổng thời gian CPU (user + kernel), đơn vị 100 ns.
    pub cpu_100ns: u64,
    /// Tổng byte đọc + ghi (file, thiết bị) từ lúc tiến trình chạy.
    pub io_bytes: u64,
}

/// Mọi tiến trình đang chạy, bỏ tiến trình Idle (pid 0).
pub fn snapshot() -> Result<Vec<ProcInfo>> {
    let mut buf: Vec<u64> = vec![0; 256 * 1024];
    loop {
        let mut needed = 0u32;
        let status = unsafe {
            NtQuerySystemInformation(SystemProcessInformation, buf.as_mut_ptr().cast(), (buf.len() * 8) as u32, &mut needed)
        };
        if status == STATUS_INFO_LENGTH_MISMATCH {
            buf.resize((needed as usize / 8) + 16 * 1024, 0);
            continue;
        }
        if status < 0 {
            return Err(CoreError::System(format!("NtQuerySystemInformation failed: 0x{:08X}", status as u32)));
        }
        break;
    }
    let base = buf.as_ptr() as *const u8;
    let mut out = Vec::with_capacity(512);
    let mut off = 0usize;
    loop {
        // SAFETY: hệ điều hành ghi chuỗi bản ghi nối nhau bằng next_entry_offset trong `buf`.
        let e = unsafe { &*(base.add(off) as *const SpiEntry) };
        let pid = e.unique_process_id as u32;
        if pid != 0 {
            let name = if e.image_name.buffer.is_null() {
                String::new()
            } else {
                let s = unsafe { std::slice::from_raw_parts(e.image_name.buffer, e.image_name.length as usize / 2) };
                String::from_utf16_lossy(s)
            };
            out.push(ProcInfo {
                pid,
                parent_pid: e.inherited_from_unique_process_id as u32,
                name: if pid == 4 && name.is_empty() { "System".into() } else { name },
                create_time: e.create_time,
                private_ws: e.working_set_private_size.max(0) as u64,
                cpu_100ns: (e.user_time + e.kernel_time).max(0) as u64,
                io_bytes: (e.read_transfer_count + e.write_transfer_count).max(0) as u64,
            });
        }
        if e.next_entry_offset == 0 {
            break;
        }
        off += e.next_entry_offset as usize;
    }
    Ok(out)
}

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

fn open(pid: u32, access: u32) -> Option<Handle> {
    let h = unsafe { OpenProcess(access, 0, pid) };
    (!h.is_null()).then_some(Handle(h))
}

fn creation_time(h: &Handle) -> Option<i64> {
    let (mut c, mut e, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    let ok = unsafe { GetProcessTimes(h.0, &mut c, &mut e, &mut k, &mut u) };
    (ok != 0).then_some(((c.dwHighDateTime as i64) << 32) | c.dwLowDateTime as i64)
}

/// Đường dẫn exe đầy đủ; `None` khi không mở được (tiến trình nhân, đã thoát…).
pub fn exe_path(pid: u32) -> Option<String> {
    let h = open(pid, PROCESS_QUERY_LIMITED_INFORMATION)?;
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(h.0, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) };
    (ok != 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
}

/// Nhớ đường dẫn exe theo (pid, giờ tạo) — không mở lại tiến trình mỗi giây.
#[derive(Default)]
pub struct PathCache {
    map: HashMap<(u32, i64), Option<String>>,
}

impl PathCache {
    pub fn get(&mut self, pid: u32, create_time: i64) -> Option<String> {
        self.map.entry((pid, create_time)).or_insert_with(|| exe_path(pid)).clone()
    }

    /// Bỏ các tiến trình đã thoát.
    pub fn retain(&mut self, alive: &HashSet<(u32, i64)>) {
        self.map.retain(|k, _| alive.contains(k));
    }
}

/// Kết thúc một tiến trình, chỉ khi nó vẫn là đúng tiến trình đã thấy (pid không bị dùng lại).
pub fn kill(pid: u32, create_time: i64) -> Result<()> {
    let h = open(pid, PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION)
        .ok_or_else(|| CoreError::System(format!("OpenProcess({pid}): {}", std::io::Error::last_os_error())))?;
    if creation_time(&h) != Some(create_time) {
        return Err(CoreError::System(format!("process {pid} has already exited")));
    }
    if unsafe { TerminateProcess(h.0, 1) } == 0 {
        return Err(CoreError::System(format!("TerminateProcess({pid}): {}", std::io::Error::last_os_error())));
    }
    Ok(())
}

/// Tổng thời gian CPU của chính WinFreeUp (100 ns) — để giữ chi phí dưới 2%.
pub fn own_cpu_100ns() -> u64 {
    let (mut c, mut e, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    let ok = unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) };
    if ok == 0 {
        return 0;
    }
    let ft = |f: FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
    ft(k) + ft(u)
}
```

`apps.rs`:

```rust
//! Gộp tiến trình thành "app" theo đường dẫn exe (spec 5.2) và luật chặn kết thúc tiến trình thiết yếu.
use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

/// Tên (không phân biệt hoa thường, bỏ `.exe`) của tiến trình thiết yếu — spec 5.2.
pub const ESSENTIAL_NAMES: [&str; 11] =
    ["system", "registry", "smss", "csrss", "wininit", "winlogon", "services", "lsass", "dwm", "svchost", "fontdrvhost"];

pub struct EssentialRules {
    /// `%WINDIR%\System32`, chữ thường.
    system32: String,
    self_pid: u32,
}

impl EssentialRules {
    pub fn new(system32: &Path, self_pid: u32) -> Self {
        EssentialRules { system32: system32.to_string_lossy().trim_end_matches('\\').to_lowercase(), self_pid }
    }

    /// Chặn khi: là chính WinFreeUp hoặc tiến trình con của nó (WebView2); hoặc tên nằm trong danh sách
    /// **và** exe nằm ngay trong System32 (hoặc không đọc được đường dẫn — chặn cho chắc).
    /// Exe trùng tên đặt ở chỗ khác (vd `C:\Temp\svchost.exe`) thì vẫn cho kết thúc.
    pub fn is_essential(&self, pid: u32, parent_pid: u32, name: &str, path: Option<&str>) -> bool {
        if pid == self.self_pid || parent_pid == self.self_pid || pid == 4 {
            return true;
        }
        let lower = name.to_lowercase();
        let stem = lower.strip_suffix(".exe").unwrap_or(&lower);
        if !ESSENTIAL_NAMES.contains(&stem) {
            return false;
        }
        match path {
            None => true,
            Some(p) => {
                let p = p.to_lowercase();
                Path::new(&p).parent().is_some_and(|d| d.to_string_lossy().trim_end_matches('\\') == self.system32)
            }
        }
    }
}

/// Một tiến trình trong mẫu hiện tại, đã kèm đường dẫn và tên thân thiện.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcSample {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    pub path: Option<String>,
    /// Tên từ `FileDescription` của exe; rỗng ⇒ dùng tên file.
    pub description: String,
    pub create_time: i64,
    pub private_ws: u64,
    pub cpu_100ns: u64,
    pub io_bytes: u64,
    /// Byte mạng (lên, xuống) trong chu kỳ vừa rồi — từ ETW; `None` khi ETW không chạy.
    pub net: Option<(u64, u64)>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProcRow {
    pub pid: u32,
    pub create_time: i64,
    pub name: String,
    pub ram: u64,
    pub cpu: f32,
    pub disk_bps: f64,
    pub net_up_bps: Option<f64>,
    pub net_down_bps: Option<f64>,
    pub essential: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppRow {
    /// Khóa gộp: đường dẫn exe chữ thường, hoặc `name:<tên>` khi không đọc được đường dẫn.
    pub key: String,
    pub name: String,
    pub path: Option<String>,
    pub ram: u64,
    pub cpu: f32,
    pub disk_bps: f64,
    pub net_up_bps: Option<f64>,
    pub net_down_bps: Option<f64>,
    /// Có ít nhất một tiến trình thiết yếu ⇒ không cho kết thúc cả app.
    pub essential: bool,
    pub procs: Vec<ProcRow>,
}

pub fn app_key(name: &str, path: Option<&str>) -> String {
    match path {
        Some(p) => p.to_lowercase(),
        None => format!("name:{}", name.to_lowercase()),
    }
}

fn display_name(p: &ProcSample) -> String {
    if !p.description.trim().is_empty() {
        return p.description.trim().to_string();
    }
    match &p.path {
        Some(path) => Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| p.name.clone()),
        None => p.name.clone(),
    }
}

/// Số liệu cộng dồn của lần trước, theo (pid, giờ tạo): (thời gian CPU, byte vào/ra).
pub type PrevTotals = HashMap<(u32, i64), (u64, u64)>;

/// Tính một mẫu bảng ứng dụng. Tiến trình mới xuất hiện (chưa có số lần trước) tính 0 cho CPU/đĩa.
/// Kết quả sắp theo RAM giảm dần (mặc định của spec); `next` trả về số cộng dồn cho lần sau.
pub fn aggregate(prev: &PrevTotals, cur: &[ProcSample], dt_ms: u64, cpus: u32, rules: &EssentialRules) -> (Vec<AppRow>, PrevTotals) {
    let dt_s = (dt_ms.max(1)) as f64 / 1000.0;
    let cpu_capacity = dt_ms.max(1) as f64 * 10_000.0 * f64::from(cpus.max(1));
    let mut next = PrevTotals::with_capacity(cur.len());
    let mut apps: HashMap<String, AppRow> = HashMap::new();
    for p in cur {
        next.insert((p.pid, p.create_time), (p.cpu_100ns, p.io_bytes));
        let (cpu, disk_bps) = match prev.get(&(p.pid, p.create_time)) {
            Some(&(c0, io0)) => (
                (p.cpu_100ns.saturating_sub(c0) as f64 / cpu_capacity * 100.0).min(100.0) as f32,
                p.io_bytes.saturating_sub(io0) as f64 / dt_s,
            ),
            None => (0.0, 0.0),
        };
        let (up, down) = match p.net {
            Some((u, d)) => (Some(u as f64 / dt_s), Some(d as f64 / dt_s)),
            None => (None, None),
        };
        let essential = rules.is_essential(p.pid, p.parent_pid, &p.name, p.path.as_deref());
        let key = app_key(&p.name, p.path.as_deref());
        let row = apps.entry(key.clone()).or_insert_with(|| AppRow {
            key,
            name: display_name(p),
            path: p.path.clone(),
            ram: 0,
            cpu: 0.0,
            disk_bps: 0.0,
            net_up_bps: up.map(|_| 0.0),
            net_down_bps: down.map(|_| 0.0),
            essential: false,
            procs: Vec::new(),
        });
        row.ram += p.private_ws;
        row.cpu += cpu;
        row.disk_bps += disk_bps;
        row.net_up_bps = row.net_up_bps.zip(up).map(|(a, b)| a + b);
        row.net_down_bps = row.net_down_bps.zip(down).map(|(a, b)| a + b);
        row.essential |= essential;
        row.procs.push(ProcRow {
            pid: p.pid,
            create_time: p.create_time,
            name: p.name.clone(),
            ram: p.private_ws,
            cpu,
            disk_bps,
            net_up_bps: up,
            net_down_bps: down,
            essential,
        });
    }
    let mut rows: Vec<AppRow> = apps.into_values().collect();
    for r in &mut rows {
        r.cpu = r.cpu.min(100.0);
        r.procs.sort_by(|a, b| b.ram.cmp(&a.ram));
    }
    rows.sort_by(|a, b| b.ram.cmp(&a.ram).then_with(|| a.key.cmp(&b.key)));
    (rows, next)
}
```

`icon.rs`:

```rust
//! Tên thân thiện (`FileDescription`) và biểu tượng của một exe cho bảng ứng dụng.
use std::path::Path;

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS,
};
use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows_sys::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

use crate::util::{from_wide, wide};

/// `FileDescription` trong tài nguyên phiên bản của exe (vd «Google Chrome»); không có ⇒ chuỗi rỗng.
pub fn file_description(path: &Path) -> String {
    let w = wide(path);
    let size = unsafe { GetFileVersionInfoSizeW(w.as_ptr(), std::ptr::null_mut()) };
    if size == 0 {
        return String::new();
    }
    let mut data = vec![0u8; size as usize];
    if unsafe { GetFileVersionInfoW(w.as_ptr(), 0, size, data.as_mut_ptr().cast()) } == 0 {
        return String::new();
    }
    let query = |sub: &str| -> Option<(*const u8, u32)> {
        let q = wide(sub);
        let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let ok = unsafe { VerQueryValueW(data.as_ptr().cast(), q.as_ptr(), &mut ptr, &mut len) };
        (ok != 0 && !ptr.is_null() && len > 0).then_some((ptr as *const u8, len))
    };
    let mut langs = Vec::new();
    if let Some((p, len)) = query(r"\VarFileInfo\Translation") {
        let pairs = unsafe { std::slice::from_raw_parts(p as *const u16, len as usize / 2) };
        langs.extend(pairs.chunks_exact(2).map(|c| format!("{:04x}{:04x}", c[0], c[1])));
    }
    // Nhiều exe khai sai bảng dịch: thử thêm tiếng Anh Mỹ với hai bảng mã hay gặp.
    langs.extend(["040904b0".to_string(), "040904e4".to_string()]);
    for l in langs {
        if let Some((p, len)) = query(&format!(r"\StringFileInfo\{l}\FileDescription")) {
            let s = from_wide(unsafe { std::slice::from_raw_parts(p as *const u16, len as usize) });
            if !s.trim().is_empty() {
                return s.trim().to_string();
            }
        }
    }
    String::new()
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= c.len() {
                out.push(B64[((n >> shift) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Đổi điểm ảnh BGRA (Windows) sang PNG RGBA. Icon kiểu cũ không có kênh alpha ⇒ dùng mặt nạ AND.
pub fn bgra_to_png(width: u32, height: u32, bgra: &[u8], mask_opaque: Option<&[bool]>) -> Option<Vec<u8>> {
    let has_alpha = bgra.chunks_exact(4).any(|p| p[3] != 0);
    let mut rgba = Vec::with_capacity(bgra.len());
    for (i, p) in bgra.chunks_exact(4).enumerate() {
        let a = if has_alpha { p[3] } else if mask_opaque.is_some_and(|m| m.get(i).copied().unwrap_or(true)) { 255 } else { 0 };
        rgba.extend_from_slice(&[p[2], p[1], p[0], a]);
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().ok()?;
        w.write_image_data(&rgba).ok()?;
    }
    Some(out)
}

/// Đọc điểm ảnh một bitmap GDI dạng 32 bit từ trên xuống.
unsafe fn read_bitmap(hbm: windows_sys::Win32::Graphics::Gdi::HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    let mut bm: BITMAP = std::mem::zeroed();
    if GetObjectW(hbm, std::mem::size_of::<BITMAP>() as i32, (&mut bm as *mut BITMAP).cast()) == 0 {
        return None;
    }
    let (w, h) = (bm.bmWidth, bm.bmHeight);
    if w <= 0 || h <= 0 || w > 256 || h > 256 {
        return None;
    }
    let mut info: BITMAPINFO = std::mem::zeroed();
    info.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w,
        biHeight: -h,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        ..std::mem::zeroed()
    };
    let mut px = vec![0u8; (w * h * 4) as usize];
    let dc = CreateCompatibleDC(std::ptr::null_mut());
    let lines = GetDIBits(dc, hbm, 0, h as u32, px.as_mut_ptr().cast(), &mut info, DIB_RGB_COLORS);
    DeleteDC(dc);
    (lines == h).then_some((w as u32, h as u32, px))
}

/// Biểu tượng 32×32 của exe dạng `data:image/png;base64,…`; không lấy được ⇒ `None` (giao diện hiện ô trống).
pub fn icon_data_url(path: &Path) -> Option<String> {
    // SHGetFileInfo không nhận dấu `/`.
    let path = std::path::PathBuf::from(path.to_string_lossy().replace('/', "\\"));
    std::thread::spawn(move || unsafe {
        // SHGetFileInfo cần COM đã khởi tạo trên luồng gọi.
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let result = (|| {
            let w = wide(&path);
            let mut sfi: SHFILEINFOW = std::mem::zeroed();
            let r = SHGetFileInfoW(w.as_ptr(), 0, &mut sfi, std::mem::size_of::<SHFILEINFOW>() as u32, SHGFI_ICON | SHGFI_LARGEICON);
            if r == 0 || sfi.hIcon.is_null() {
                return None;
            }
            let mut ii: ICONINFO = std::mem::zeroed();
            let ok = GetIconInfo(sfi.hIcon, &mut ii);
            DestroyIcon(sfi.hIcon);
            if ok == 0 {
                return None;
            }
            let color = read_bitmap(ii.hbmColor);
            let mask = read_bitmap(ii.hbmMask);
            DeleteObject(ii.hbmColor);
            DeleteObject(ii.hbmMask);
            let (w, h, px) = color?;
            // Mặt nạ AND: điểm đen (0) là phần hình, trắng là nền trong suốt.
            let opaque: Option<Vec<bool>> = mask.map(|(_, _, m)| m.chunks_exact(4).map(|p| p[0] == 0).collect());
            let png = bgra_to_png(w, h, &px, opaque.as_deref())?;
            Some(format!("data:image/png;base64,{}", base64(&png)))
        })();
        if com {
            CoUninitialize();
        }
        result
    })
    .join()
    .ok()
    .flatten()
}
```

- [ ] **Step 4: Chạy test và clippy**

Run: `cargo test perf::procs`, `cargo test perf::apps`, `cargo test perf::icon`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
Expected: procs `6`, apps `7`, icon `4 passed`; toàn bộ `0 failed`. Đã đo khi viết kế hoạch: RAM riêng đọc được khớp đúng từng byte với bộ đếm `\Process(explorer#8)\Working Set - Private` (19 066 880); biểu tượng Chrome/Explorer ra PNG đúng hình.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-diagnose/src/perf/procs.rs
rtk git add crates/winfreeup-diagnose/src/perf/apps.rs
rtk git add crates/winfreeup-diagnose/src/perf/icon.rs
rtk git commit -m "feat(diagnose): ảnh chụp tiến trình, gộp theo app, chặn tiến trình thiết yếu, tên và biểu tượng app

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Bộ đếm hệ thống, WMI, nhiệt độ, mạng theo app (ETW)

**Files:**
- Modify (thay dòng khung): `crates/winfreeup-diagnose/src/perf/counters.rs`, `src/wmiq.rs`, `perf/thermal.rs`, `perf/netetw.rs`
- Test: bốn file trên

**Interfaces:**
- Consumes (Task 1): `util::wide`.
- Produces:
  - `counters::Pdh`, `Counter`, `CpuDiskCounters::open() -> Result<Self>` (CPU bắt buộc; `% Idle Time` đĩa và `% Processor Performance` thiếu thì ghi lý do vào `disk_error`/`perf_error`), `read(&self) -> Result<CpuDiskReading { cpu, disk_active, perf: Option<f32> }>`; `MemoryStatus { load_pct, total, available, commit_used, commit_limit }` (Serialize), `memory() -> Result<MemoryStatus>`; `net_totals() -> Result<(u64 /*nhận*/, u64 /*gửi*/)>`.
  - `wmiq::NS_CIMV2`, `NS_WMI`, `NS_STORAGE`, `connect(ns) -> Result<WMIConnection, String>`, `query<T>(ns, wql) -> Result<Vec<T>, String>`, `query_on<T>(&conn, wql)`.
  - `thermal::decikelvin_to_celsius(u32) -> f32`, `hottest_celsius(&[u32]) -> Option<f32>`, `ThermalReader::new()`, `read(&self) -> Result<f32, String>`.
  - `netetw::SESSION_NAME = "WinFreeUp-Net"`, `direction(event_id: u16) -> Option<Direction>`, `NetCounts`, `NetTrace::start() -> Result<NetTrace, String>`, `take(&self) -> HashMap<u32, (u64 /*lên*/, u64 /*xuống*/)>`, `stop(self)`, `stop_stale_session()`.

- [ ] **Step 1: Viết test hỏng** — thay dòng khung của bốn file bằng phần test:

`counters.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_and_disk_counters_give_percentages_after_two_reads() {
        let c = CpuDiskCounters::open().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let r = c.read().unwrap();
        let cpu = r.cpu.expect("CPU luôn có");
        assert!((0.0..=100.0).contains(&cpu), "{cpu}");
        if let Some(d) = r.disk_active {
            assert!((0.0..=100.0).contains(&d), "{d}");
        } else {
            assert!(c.disk_error.is_some());
        }
        if let Some(p) = r.perf {
            assert!(p > 0.0 && p < 400.0, "{p}");
        }
    }

    #[test]
    fn unknown_counter_is_a_clean_error() {
        let pdh = Pdh::open().unwrap();
        let e = pdh.add(r"\Khong Co(_Total)\% Gi Ca").unwrap_err().to_string();
        assert!(e.starts_with("PDH "), "{e}");
    }

    #[test]
    fn memory_numbers_are_consistent() {
        let m = memory().unwrap();
        assert!(m.load_pct <= 100);
        assert!(m.available <= m.total);
        assert!(m.commit_used <= m.commit_limit && m.commit_limit >= m.total / 2);
    }

    #[test]
    fn network_totals_do_not_go_backwards() {
        let (rx1, tx1) = net_totals().unwrap();
        let (rx2, tx2) = net_totals().unwrap();
        assert!(rx2 >= rx1 && tx2 >= tx1);
    }
}
```

`wmiq.rs`:

```rust
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
```

`thermal.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decikelvin_conversion() {
        assert!((decikelvin_to_celsius(3232) - 50.05).abs() < 0.01);
    }

    #[test]
    fn hottest_zone_wins_and_zero_is_ignored() {
        assert_eq!(hottest_celsius(&[]), None);
        assert_eq!(hottest_celsius(&[0]), None);
        let c = hottest_celsius(&[3032, 3232, 0]).unwrap();
        assert!((c - 50.05).abs() < 0.01);
    }

    #[test]
    fn reading_on_this_machine_is_a_value_or_a_message() {
        // Không Admin thường bị từ chối; máy ảo thường không có lớp — cả hai phải là lỗi có chữ, không panic.
        match ThermalReader::new().read() {
            Ok(c) => assert!(c > -100.0 && c < 200.0),
            Err(e) => assert!(!e.is_empty()),
        }
    }
}
```

`netetw.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_ids_map_to_directions() {
        for id in [10, 26, 42, 58] {
            assert_eq!(direction(id), Some(Direction::Up), "{id}");
        }
        for id in [11, 27, 43, 59] {
            assert_eq!(direction(id), Some(Direction::Down), "{id}");
        }
        for id in [12, 13, 14, 15, 18, 0] {
            assert_eq!(direction(id), None, "{id}");
        }
    }

    #[test]
    fn counts_accumulate_per_pid_and_reset_on_take() {
        let c = NetCounts::default();
        c.add(7, Direction::Up, 100);
        c.add(7, Direction::Down, 40);
        c.add(7, Direction::Down, 60);
        c.add(9, Direction::Up, 1);
        let m = c.take();
        assert_eq!(m[&7], (100, 100));
        assert_eq!(m[&9], (1, 0));
        assert!(c.take().is_empty());
    }

    #[test]
    fn starting_without_admin_fails_with_a_message_and_admin_captures_traffic() {
        match NetTrace::start() {
            Err(e) => assert!(e.contains(SESSION_NAME), "{e}"),
            Ok(t) => {
                // Có Admin: tạo chút lưu lượng tới chính máy rồi xem có PID nào được ghi.
                let _ = std::net::TcpStream::connect_timeout(&"1.1.1.1:443".parse().unwrap(), std::time::Duration::from_secs(2));
                std::thread::sleep(std::time::Duration::from_secs(2));
                let _ = t.take();
                t.stop();
            }
        }
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test`
Expected: FAIL biên dịch — `cannot find type CpuDiskCounters`, `cannot find function query`, `cannot find type ThermalReader`, `cannot find function direction`.

- [ ] **Step 3: Viết mã (trên khối test)**

`counters.rs`:

```rust
//! Số liệu toàn máy: CPU, Hoạt động đĩa, % hiệu năng CPU (PDH), RAM (GlobalMemoryStatusEx),
//! tổng mạng (GetIfTable2). Bộ đếm PDH thêm bằng tên TIẾNG ANH (`PdhAddEnglishCounterW`) để chạy được
//! trên Windows tiếng Việt, nơi tên bộ đếm hiển thị đã được dịch.
use serde::Serialize;
use windows_sys::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
use windows_sys::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue, PDH_CSTATUS_NEW_DATA,
    PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
};
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use winfreeup_core::{CoreError, Result};

use crate::util::wide;

/// Không cắt giá trị ở 100 (tiện ích và hiệu năng CPU vượt 100% khi turbo).
const PDH_FMT_NOCAP100: u32 = 0x8000;
const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
const IF_OPER_STATUS_UP: i32 = 1;

pub const CPU_COUNTERS: [&str; 2] = [r"\Processor Information(_Total)\% Processor Utility", r"\Processor(_Total)\% Processor Time"];
pub const DISK_IDLE_COUNTER: &str = r"\PhysicalDisk(_Total)\% Idle Time";
pub const PERF_COUNTER: &str = r"\Processor Information(_Total)\% Processor Performance";

fn pdh_err(what: &str, code: u32) -> CoreError {
    CoreError::System(format!("PDH {what}: 0x{code:08X}"))
}

pub struct Pdh {
    query: PDH_HQUERY,
}

/// Một bộ đếm trong truy vấn `Pdh` (bọc con trỏ thô để hàm công khai không nhận con trỏ).
#[derive(Debug, Clone, Copy)]
pub struct Counter(PDH_HCOUNTER);

// PDH_HQUERY là con trỏ thô; truy vấn chỉ dùng trên một luồng tại một thời điểm (luồng lấy mẫu).
unsafe impl Send for Pdh {}

impl Pdh {
    pub fn open() -> Result<Pdh> {
        let mut q: PDH_HQUERY = std::ptr::null_mut();
        let rc = unsafe { windows_sys::Win32::System::Performance::PdhOpenQueryW(std::ptr::null(), 0, &mut q) };
        if rc != 0 {
            return Err(pdh_err("open", rc));
        }
        Ok(Pdh { query: q })
    }

    pub fn add(&self, path: &str) -> Result<Counter> {
        let w = wide(path);
        let mut c: PDH_HCOUNTER = std::ptr::null_mut();
        let rc = unsafe { PdhAddEnglishCounterW(self.query, w.as_ptr(), 0, &mut c) };
        if rc != 0 {
            return Err(pdh_err(path, rc));
        }
        Ok(Counter(c))
    }

    pub fn collect(&self) -> Result<()> {
        let rc = unsafe { PdhCollectQueryData(self.query) };
        if rc != 0 {
            return Err(pdh_err("collect", rc));
        }
        Ok(())
    }

    /// Giá trị đã định dạng; `None` khi chưa đủ hai lần thu (bộ đếm tốc độ) hoặc dữ liệu không hợp lệ.
    pub fn value(&self, c: Counter) -> Option<f64> {
        let mut v = PDH_FMT_COUNTERVALUE::default();
        let rc = unsafe { PdhGetFormattedCounterValue(c.0, PDH_FMT_DOUBLE | PDH_FMT_NOCAP100, std::ptr::null_mut(), &mut v) };
        if rc != 0 || !(v.CStatus == PDH_CSTATUS_VALID_DATA || v.CStatus == PDH_CSTATUS_NEW_DATA) {
            return None;
        }
        Some(unsafe { v.Anonymous.doubleValue })
    }
}

impl Drop for Pdh {
    fn drop(&mut self) {
        unsafe { PdhCloseQuery(self.query) };
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuDiskReading {
    pub cpu: Option<f32>,
    pub disk_active: Option<f32>,
    pub perf: Option<f32>,
}

/// Ba bộ đếm của mục Hiệu năng. CPU bắt buộc; đĩa và % hiệu năng thiếu thì `None` (spec 5.4: ẩn chỉ báo).
pub struct CpuDiskCounters {
    pdh: Pdh,
    cpu: Counter,
    disk_idle: Option<Counter>,
    perf: Option<Counter>,
    /// Lý do bộ đếm hiệu năng CPU không có (nguyên văn), để ghi «Không đo được».
    pub perf_error: Option<String>,
    pub disk_error: Option<String>,
}

unsafe impl Send for CpuDiskCounters {}

impl CpuDiskCounters {
    pub fn open() -> Result<Self> {
        let pdh = Pdh::open()?;
        let cpu = pdh.add(CPU_COUNTERS[0]).or_else(|_| pdh.add(CPU_COUNTERS[1]))?;
        let (disk_idle, disk_error) = match pdh.add(DISK_IDLE_COUNTER) {
            Ok(c) => (Some(c), None),
            Err(e) => (None, Some(e.to_string())),
        };
        let (perf, perf_error) = match pdh.add(PERF_COUNTER) {
            Ok(c) => (Some(c), None),
            Err(e) => (None, Some(e.to_string())),
        };
        pdh.collect()?;
        Ok(CpuDiskCounters { pdh, cpu, disk_idle, perf, perf_error, disk_error })
    }

    pub fn read(&self) -> Result<CpuDiskReading> {
        self.pdh.collect()?;
        Ok(CpuDiskReading {
            cpu: self.pdh.value(self.cpu).map(|v| v.clamp(0.0, 100.0) as f32),
            disk_active: self.disk_idle.and_then(|c| self.pdh.value(c)).map(|idle| (100.0 - idle).clamp(0.0, 100.0) as f32),
            perf: self.perf.and_then(|c| self.pdh.value(c)).map(|v| v.max(0.0) as f32),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MemoryStatus {
    /// % RAM đang dùng (Windows tự tính).
    pub load_pct: u32,
    pub total: u64,
    pub available: u64,
    /// Commit charge đang dùng và giới hạn (RAM + pagefile).
    pub commit_used: u64,
    pub commit_limit: u64,
}

pub fn memory() -> Result<MemoryStatus> {
    let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    if unsafe { GlobalMemoryStatusEx(&mut m) } == 0 {
        return Err(CoreError::System(format!("GlobalMemoryStatusEx: {}", std::io::Error::last_os_error())));
    }
    Ok(MemoryStatus {
        load_pct: m.dwMemoryLoad,
        total: m.ullTotalPhys,
        available: m.ullAvailPhys,
        commit_used: m.ullTotalPageFile.saturating_sub(m.ullAvailPageFile),
        commit_limit: m.ullTotalPageFile,
    })
}

/// Tổng byte (nhận, gửi) từ lúc khởi động qua các card mạng thật đang bật.
/// Bỏ loopback và card ảo (Hyper-V, VPN…) để lưu lượng không bị đếm hai lần.
pub fn net_totals() -> Result<(u64, u64)> {
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    let rc = unsafe { GetIfTable2(&mut table) };
    if rc != 0 {
        return Err(CoreError::System(format!("GetIfTable2: {}", std::io::Error::from_raw_os_error(rc as i32))));
    }
    let (mut rx, mut tx) = (0u64, 0u64);
    unsafe {
        let n = (*table).NumEntries as usize;
        let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), n);
        for r in rows {
            let hardware = r.InterfaceAndOperStatusFlags._bitfield & 1 != 0;
            if hardware && r.Type != IF_TYPE_SOFTWARE_LOOPBACK && r.OperStatus == IF_OPER_STATUS_UP {
                rx += r.InOctets;
                tx += r.OutOctets;
            }
        }
        FreeMibTable(table.cast());
    }
    Ok((rx, tx))
}
```

`wmiq.rs`:

```rust
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
```

`thermal.rs`:

```rust
//! Nhiệt độ qua ACPI (`MSAcpi_ThermalZoneTemperature`, WMI `root\wmi`) — spec 5.5. Không dùng driver.
//! Nhiều máy không có lớp này, hoặc trả số cố định; `detect::TempTracker` loại các số đó.
use serde::Deserialize;
use wmi::WMIConnection;

use crate::wmiq;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Zone {
    current_temperature: u32,
}

/// Đổi số đo `MSAcpi_ThermalZoneTemperature` (phần mười Kelvin) sang °C.
pub fn decikelvin_to_celsius(v: u32) -> f32 {
    v as f32 / 10.0 - 273.15
}

/// Nhiệt độ cao nhất trong các vùng nhiệt (°C) từ danh sách phần mười Kelvin.
pub fn hottest_celsius(decikelvins: &[u32]) -> Option<f32> {
    decikelvins.iter().copied().filter(|&v| v > 0).map(decikelvin_to_celsius).reduce(f32::max)
}

/// Giữ kết nối WMI trên luồng lấy mẫu để không phải mở lại mỗi lần đọc.
pub struct ThermalReader {
    conn: Result<WMIConnection, String>,
}

impl ThermalReader {
    pub fn new() -> Self {
        ThermalReader { conn: wmiq::connect(wmiq::NS_WMI) }
    }

    /// °C của vùng nóng nhất; lỗi là thông điệp nguyên văn (không có lớp, bị từ chối quyền…).
    pub fn read(&self) -> Result<f32, String> {
        let conn = self.conn.as_ref().map_err(|e| e.clone())?;
        let zones: Vec<Zone> = wmiq::query_on(conn, "SELECT CurrentTemperature FROM MSAcpi_ThermalZoneTemperature")?;
        let raw: Vec<u32> = zones.iter().map(|z| z.current_temperature).collect();
        hottest_celsius(&raw).ok_or_else(|| "MSAcpi_ThermalZoneTemperature: no instances".to_string())
    }
}

impl Default for ThermalReader {
    fn default() -> Self {
        Self::new()
    }
}
```

`netetw.rs`:

```rust
//! Mạng theo app (spec 5.6): phiên ETW thời gian thực `WinFreeUp-Net`, provider
//! `Microsoft-Windows-Kernel-Network`. Cần Admin. Sự kiện gửi/nhận TCP/UDP (IPv4/IPv6) mang `PID` và `size`.
//! Khởi động thấy phiên cùng tên còn sót (lần trước bị tắt ngang) ⇒ dừng phiên đó rồi tạo lại.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ferrisetw::parser::Parser;
use ferrisetw::provider::Provider;
use ferrisetw::trace::{stop_trace_by_name, UserTrace};
use ferrisetw::{EventRecord, SchemaLocator};

pub const SESSION_NAME: &str = "WinFreeUp-Net";
pub const KERNEL_NETWORK_GUID: &str = "7DD42A49-5329-4832-8DFD-43D979153A88";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

/// Mã sự kiện ⇒ chiều. 10/11 TCPv4, 26/27 TCPv6, 42/43 UDPv4, 58/59 UDPv6 (gửi/nhận) —
/// đã đối chiếu bằng `wevtutil gp Microsoft-Windows-Kernel-Network /ge /gm:true`.
pub fn direction(event_id: u16) -> Option<Direction> {
    match event_id {
        10 | 26 | 42 | 58 => Some(Direction::Up),
        11 | 27 | 43 | 59 => Some(Direction::Down),
        _ => None,
    }
}

/// Byte (lên, xuống) theo PID, cộng dồn giữa hai lần `take`.
#[derive(Default)]
pub struct NetCounts(Mutex<HashMap<u32, (u64, u64)>>);

impl NetCounts {
    pub fn add(&self, pid: u32, dir: Direction, bytes: u64) {
        if let Ok(mut m) = self.0.lock() {
            let e = m.entry(pid).or_default();
            match dir {
                Direction::Up => e.0 += bytes,
                Direction::Down => e.1 += bytes,
            }
        }
    }

    /// Lấy ra và xóa số đã cộng — gọi một lần mỗi mẫu.
    pub fn take(&self) -> HashMap<u32, (u64, u64)> {
        self.0.lock().map(|mut m| std::mem::take(&mut *m)).unwrap_or_default()
    }
}

fn on_event(counts: &NetCounts, record: &EventRecord, schemas: &SchemaLocator) {
    let Some(dir) = direction(record.event_id()) else { return };
    let Ok(schema) = schemas.event_schema(record) else { return };
    let parser = Parser::create(record, &schema);
    if let (Ok(pid), Ok(size)) = (parser.try_parse::<u32>("PID"), parser.try_parse::<u32>("size")) {
        counts.add(pid, dir, u64::from(size));
    }
}

pub struct NetTrace {
    trace: Option<UserTrace>,
    counts: Arc<NetCounts>,
}

impl NetTrace {
    /// Mở phiên. Lỗi là thông điệp nguyên văn (không Admin ⇒ bị từ chối) — cột Mạng hiện «–».
    pub fn start() -> Result<NetTrace, String> {
        let _ = stop_trace_by_name(SESSION_NAME);
        let counts = Arc::new(NetCounts::default());
        let sink = counts.clone();
        let provider = Provider::by_guid(KERNEL_NETWORK_GUID)
            .add_callback(move |record: &EventRecord, schemas: &SchemaLocator| on_event(&sink, record, schemas))
            .build();
        let trace = UserTrace::new()
            .named(SESSION_NAME.to_string())
            .enable(provider)
            .start_and_process()
            .map_err(|e| format!("ETW {SESSION_NAME}: {e:?}"))?;
        Ok(NetTrace { trace: Some(trace), counts })
    }

    pub fn take(&self) -> HashMap<u32, (u64, u64)> {
        self.counts.take()
    }

    pub fn stop(mut self) {
        if let Some(t) = self.trace.take() {
            let _ = t.stop();
        }
    }
}

impl Drop for NetTrace {
    fn drop(&mut self) {
        if let Some(t) = self.trace.take() {
            let _ = t.stop();
        }
    }
}

/// Dừng phiên còn sót nếu có — gọi khi đóng ứng dụng.
pub fn stop_stale_session() {
    let _ = stop_trace_by_name(SESSION_NAME);
}
```

- [ ] **Step 4: Chạy test và clippy**

Run: `cargo test perf::counters`, `cargo test wmiq`, `cargo test perf::thermal`, `cargo test perf::netetw`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
Expected: counters `4`, wmiq `2`, thermal `3`, netetw `3 passed`; toàn bộ `0 failed`. Không Admin thì ETW trả `ETW WinFreeUp-Net: … Access is denied.` và nhiệt độ trả lỗi WMI nguyên văn (máy dev: `HRESULT Call failed with: 0x8004100C`) — test chấp nhận cả hai nhánh. Mã sự kiện 10/11/26/27/42/43/58/59 đã đối chiếu bằng `wevtutil gp Microsoft-Windows-Kernel-Network /ge /gm:true` (trường `PID` rồi `size`).

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-diagnose/src/perf/counters.rs
rtk git add crates/winfreeup-diagnose/src/wmiq.rs
rtk git add crates/winfreeup-diagnose/src/perf/thermal.rs
rtk git add crates/winfreeup-diagnose/src/perf/netetw.rs
rtk git commit -m "feat(diagnose): bộ đếm PDH/RAM/mạng, truy vấn WMI, nhiệt độ ACPI, phiên ETW WinFreeUp-Net

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Danh sách app khởi động (StartupApproved)

**Files:**
- Modify (thay dòng khung): `crates/winfreeup-diagnose/src/health/startup.rs`
- Test: `startup.rs`

**Interfaces:**
- Consumes (Task 1): `actlog::ActionLog`, `util::{wide, from_wide}`.
- Produces: `Hive { CurrentUser, LocalMachine }`, `StartupSource { HkcuRun, HklmRun, HklmRun32, UserFolder, CommonFolder }` (serde `hkcu_run`…), `code()`, `approved() -> (Hive, String)`; `is_enabled(Option<&[u8]>) -> bool`; `approved_bytes(enabled, now_filetime) -> [u8; 12]`; trait `Registry` (`string_values`, `binary_value`, `set_binary`) + `WinRegistry`; `StartupEntry { id /* "<nguồn>:<tên>" */, source, name, command, enabled }`, `StartupList { entries, errors }`; `Startup::new(reg, user_folder, common_folder, log)`, `Startup::from_env(log) -> Result<Self, String>`, `list() -> StartupList`, `enabled_count() -> Result<u32, String>` (có nguồn lỗi ⇒ `Err`), `set_enabled(id, enabled) -> Result<StartupEntry, String>` (`unknown_entry` khi không có; ghi nhật ký `STARTUP_ENABLE|STARTUP_DISABLE <nguồn> <tên>`).

- [ ] **Step 1: Viết test hỏng** — thay dòng khung bằng:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeReg {
        strings: HashMap<(Hive, String), Vec<(String, String)>>,
        binaries: Mutex<HashMap<(Hive, String, String), Vec<u8>>>,
        fail_key: Option<String>,
    }

    impl Registry for FakeReg {
        fn string_values(&self, hive: Hive, key: &str) -> Result<Vec<(String, String)>, String> {
            if self.fail_key.as_deref() == Some(key) {
                return Err(format!("RegOpenKeyExW {key}: Access is denied. (os error 5)"));
            }
            Ok(self.strings.get(&(hive, key.to_string())).cloned().unwrap_or_default())
        }
        fn binary_value(&self, hive: Hive, key: &str, name: &str) -> Result<Option<Vec<u8>>, String> {
            Ok(self.binaries.lock().unwrap().get(&(hive, key.to_string(), name.to_string())).cloned())
        }
        fn set_binary(&self, hive: Hive, key: &str, name: &str, data: &[u8]) -> Result<(), String> {
            self.binaries.lock().unwrap().insert((hive, key.to_string(), name.to_string()), data.to_vec());
            Ok(())
        }
    }

    fn setup(reg: FakeReg) -> (tempfile::TempDir, Startup) {
        let t = tempfile::tempdir().unwrap();
        let user = t.path().join("user");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(user.join("Zalo.lnk"), "x").unwrap();
        std::fs::write(user.join("desktop.ini"), "x").unwrap();
        let s = Startup::new(Box::new(reg), user, t.path().join("common-khong-co"), Arc::new(ActionLog::new(t.path().join("logs"))));
        (t, s)
    }

    fn reg_with_run() -> FakeReg {
        let mut r = FakeReg::default();
        r.strings.insert((Hive::CurrentUser, RUN_KEY.into()), vec![("OneDrive".into(), r#""C:\OneDrive.exe" /background"#.into())]);
        r.strings.insert((Hive::LocalMachine, RUN32_KEY.into()), vec![("Unikey".into(), r"C:\Unikey\UniKeyNT.exe".into())]);
        r.binaries
            .lock()
            .unwrap()
            .insert((Hive::LocalMachine, format!(r"{APPROVED_KEY}\Run32"), "Unikey".into()), vec![3, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]);
        r
    }

    #[test]
    fn approved_flag_parity_decides_enabled() {
        assert!(is_enabled(None));
        assert!(is_enabled(Some(&[])));
        assert!(is_enabled(Some(&[2, 0, 0, 0])));
        assert!(is_enabled(Some(&[6])));
        assert!(!is_enabled(Some(&[3])));
        assert!(!is_enabled(Some(&[7])));
        let off = approved_bytes(false, 0x0102_0304_0506_0708);
        assert_eq!(off[0], 3);
        assert_eq!(&off[4..], &0x0102_0304_0506_0708u64.to_le_bytes());
        assert_eq!(approved_bytes(true, 99), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn lists_every_source_with_its_state_and_skips_desktop_ini() {
        let (_t, s) = setup(reg_with_run());
        let l = s.list();
        assert!(l.errors.is_empty());
        let ids: Vec<_> = l.entries.iter().map(|e| (e.id.as_str(), e.enabled)).collect();
        assert_eq!(ids, vec![("hkcu_run:OneDrive", true), ("hklm_run32:Unikey", false), ("user_folder:Zalo.lnk", true)]);
        assert_eq!(s.enabled_count().unwrap(), 2);
    }

    #[test]
    fn toggling_writes_startup_approved_like_task_manager_and_logs() {
        let (t, s) = setup(reg_with_run());
        let e = s.set_enabled("hkcu_run:OneDrive", false).unwrap();
        assert!(!e.enabled);
        assert!(!s.list().entries.iter().find(|e| e.id == "hkcu_run:OneDrive").unwrap().enabled);
        s.set_enabled("user_folder:Zalo.lnk", false).unwrap();
        s.set_enabled("hkcu_run:OneDrive", true).unwrap();
        assert_eq!(s.enabled_count().unwrap(), 1);
        let log = std::fs::read_dir(t.path().join("logs")).unwrap().next().unwrap().unwrap().path();
        let text = std::fs::read_to_string(log).unwrap();
        assert!(text.contains("STARTUP_DISABLE hkcu_run OneDrive"));
        assert!(text.contains("STARTUP_ENABLE hkcu_run OneDrive"));
        assert_eq!(s.set_enabled("hkcu_run:KhongCo", false).unwrap_err(), "unknown_entry");
    }

    #[test]
    fn one_unreadable_source_does_not_hide_the_others() {
        let mut r = reg_with_run();
        r.fail_key = Some(RUN32_KEY.into());
        let (_t, s) = setup(r);
        let l = s.list();
        assert_eq!(l.errors.len(), 1);
        assert_eq!(l.entries.len(), 2);
        assert!(s.enabled_count().is_err(), "đếm thiếu thì phải là «không đo được», không phải một con số sai");
    }

    #[test]
    fn real_registry_listing_reads_without_error() {
        let log = Arc::new(ActionLog::new(std::env::temp_dir().join("wfu-khong-ghi")));
        let s = Startup::from_env(log).unwrap();
        let l = s.list();
        assert!(l.errors.is_empty(), "{:?}", l.errors);
        assert!(l.entries.iter().all(|e| !e.name.is_empty()));
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test health::startup`
Expected: FAIL biên dịch — `cannot find type Startup`.

- [ ] **Step 3: Viết mã (trên khối test)**

```rust
//! Danh sách app khởi động cùng máy (spec 4): khóa `Run` của HKCU/HKLM (cả nhánh 32 bit) và hai thư mục
//! Startup. Bật/tắt ghi vào `Explorer\StartupApproved\{Run,Run32,StartupFolder}` — đúng cơ chế Task Manager,
//! không xóa gì nên bật lại được bất cứ lúc nào.
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegEnumValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE, KEY_WOW64_64KEY, REG_BINARY, REG_EXPAND_SZ, REG_OPTION_NON_VOLATILE, REG_SZ,
};

use crate::actlog::ActionLog;
use crate::util::{from_wide, wide};

pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const RUN32_KEY: &str = r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";
pub const APPROVED_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Hive {
    CurrentUser,
    LocalMachine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupSource {
    HkcuRun,
    HklmRun,
    HklmRun32,
    UserFolder,
    CommonFolder,
}

pub const ALL_SOURCES: [StartupSource; 5] =
    [StartupSource::HkcuRun, StartupSource::HklmRun, StartupSource::HklmRun32, StartupSource::UserFolder, StartupSource::CommonFolder];

impl StartupSource {
    pub fn code(self) -> &'static str {
        match self {
            StartupSource::HkcuRun => "hkcu_run",
            StartupSource::HklmRun => "hklm_run",
            StartupSource::HklmRun32 => "hklm_run32",
            StartupSource::UserFolder => "user_folder",
            StartupSource::CommonFolder => "common_folder",
        }
    }

    /// Khóa StartupApproved tương ứng, đúng như Task Manager dùng.
    pub fn approved(self) -> (Hive, String) {
        let (hive, sub) = match self {
            StartupSource::HkcuRun => (Hive::CurrentUser, "Run"),
            StartupSource::HklmRun => (Hive::LocalMachine, "Run"),
            StartupSource::HklmRun32 => (Hive::LocalMachine, "Run32"),
            StartupSource::UserFolder => (Hive::CurrentUser, "StartupFolder"),
            StartupSource::CommonFolder => (Hive::LocalMachine, "StartupFolder"),
        };
        (hive, format!(r"{APPROVED_KEY}\{sub}"))
    }
}

/// Byte đầu chẵn (02, 06) = bật, lẻ (03, 07) = tắt. Không có giá trị = bật (chưa ai tắt lần nào).
pub fn is_enabled(approved: Option<&[u8]>) -> bool {
    approved.and_then(|d| d.first()).is_none_or(|b| b & 1 == 0)
}

/// Giá trị 12 byte Task Manager ghi: 4 byte cờ + FILETIME lúc tắt (bật thì 0).
pub fn approved_bytes(enabled: bool, now_filetime: u64) -> [u8; 12] {
    let mut v = [0u8; 12];
    v[0] = if enabled { 0x02 } else { 0x03 };
    if !enabled {
        v[4..12].copy_from_slice(&now_filetime.to_le_bytes());
    }
    v
}

fn filetime_now() -> u64 {
    let unix_100ns = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() / 100).unwrap_or(0) as u64;
    unix_100ns + 116_444_736_000_000_000
}

/// Thao tác registry cần cho danh sách khởi động — bản thật `WinRegistry`, test dùng bản giả trong bộ nhớ.
pub trait Registry: Send + Sync {
    /// Các giá trị chuỗi (REG_SZ/REG_EXPAND_SZ) của một khóa. Khóa không có ⇒ danh sách rỗng.
    fn string_values(&self, hive: Hive, key: &str) -> Result<Vec<(String, String)>, String>;
    fn binary_value(&self, hive: Hive, key: &str, name: &str) -> Result<Option<Vec<u8>>, String>;
    /// Ghi REG_BINARY, tạo khóa nếu chưa có.
    fn set_binary(&self, hive: Hive, key: &str, name: &str, data: &[u8]) -> Result<(), String>;
}

pub struct WinRegistry;

fn root(h: Hive) -> HKEY {
    match h {
        Hive::CurrentUser => HKEY_CURRENT_USER,
        Hive::LocalMachine => HKEY_LOCAL_MACHINE,
    }
}

fn reg_err(what: &str, key: &str, code: u32) -> String {
    format!("{what} {key}: {}", std::io::Error::from_raw_os_error(code as i32))
}

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        unsafe { RegCloseKey(self.0) };
    }
}

fn open_read(hive: Hive, key: &str) -> Result<Option<Key>, String> {
    let w = wide(key);
    let mut h: HKEY = std::ptr::null_mut();
    let rc = unsafe { RegOpenKeyExW(root(hive), w.as_ptr(), 0, KEY_READ | KEY_WOW64_64KEY, &mut h) };
    match rc {
        ERROR_SUCCESS => Ok(Some(Key(h))),
        ERROR_FILE_NOT_FOUND => Ok(None),
        rc => Err(reg_err("RegOpenKeyExW", key, rc)),
    }
}

impl Registry for WinRegistry {
    fn string_values(&self, hive: Hive, key: &str) -> Result<Vec<(String, String)>, String> {
        let Some(k) = open_read(hive, key)? else { return Ok(Vec::new()) };
        let mut out = Vec::new();
        for i in 0.. {
            let mut name = vec![0u16; 16_384];
            let mut name_len = name.len() as u32;
            let mut ty = 0u32;
            let mut data = vec![0u8; 64 * 1024];
            let mut data_len = data.len() as u32;
            let rc = unsafe {
                RegEnumValueW(k.0, i, name.as_mut_ptr(), &mut name_len, std::ptr::null(), &mut ty, data.as_mut_ptr(), &mut data_len)
            };
            if rc == ERROR_NO_MORE_ITEMS {
                break;
            }
            if rc != ERROR_SUCCESS {
                return Err(reg_err("RegEnumValueW", key, rc));
            }
            if ty == REG_SZ || ty == REG_EXPAND_SZ {
                let units: Vec<u16> = data[..data_len as usize].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                out.push((String::from_utf16_lossy(&name[..name_len as usize]), from_wide(&units)));
            }
        }
        Ok(out)
    }

    fn binary_value(&self, hive: Hive, key: &str, name: &str) -> Result<Option<Vec<u8>>, String> {
        let Some(k) = open_read(hive, key)? else { return Ok(None) };
        let n = wide(name);
        let mut data = vec![0u8; 256];
        let mut len = data.len() as u32;
        let rc = unsafe { RegQueryValueExW(k.0, n.as_ptr(), std::ptr::null(), std::ptr::null_mut(), data.as_mut_ptr(), &mut len) };
        match rc {
            ERROR_SUCCESS => {
                data.truncate(len as usize);
                Ok(Some(data))
            }
            ERROR_FILE_NOT_FOUND => Ok(None),
            rc => Err(reg_err("RegQueryValueExW", key, rc)),
        }
    }

    fn set_binary(&self, hive: Hive, key: &str, name: &str, data: &[u8]) -> Result<(), String> {
        let w = wide(key);
        let mut h: HKEY = std::ptr::null_mut();
        let rc = unsafe {
            RegCreateKeyExW(
                root(hive),
                w.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE | KEY_WOW64_64KEY,
                std::ptr::null(),
                &mut h,
                std::ptr::null_mut(),
            )
        };
        if rc != ERROR_SUCCESS {
            return Err(reg_err("RegCreateKeyExW", key, rc));
        }
        let k = Key(h);
        let n = wide(name);
        let rc = unsafe { RegSetValueExW(k.0, n.as_ptr(), 0, REG_BINARY, data.as_ptr(), data.len() as u32) };
        if rc != ERROR_SUCCESS {
            return Err(reg_err("RegSetValueExW", key, rc));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StartupEntry {
    /// `<nguồn>:<tên>` — duy nhất trong danh sách.
    pub id: String,
    pub source: StartupSource,
    pub name: String,
    /// Dòng lệnh (khóa Run) hoặc đường dẫn file (thư mục Startup).
    pub command: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StartupList {
    pub entries: Vec<StartupEntry>,
    /// Nguồn không đọc được (nguyên văn) — các nguồn khác vẫn liệt kê.
    pub errors: Vec<String>,
}

pub struct Startup {
    reg: Box<dyn Registry>,
    user_folder: PathBuf,
    common_folder: PathBuf,
    log: Arc<ActionLog>,
}

impl Startup {
    pub fn new(reg: Box<dyn Registry>, user_folder: PathBuf, common_folder: PathBuf, log: Arc<ActionLog>) -> Self {
        Startup { reg, user_folder, common_folder, log }
    }

    /// Hai thư mục Startup của máy đang chạy.
    pub fn from_env(log: Arc<ActionLog>) -> Result<Self, String> {
        let appdata = std::env::var_os("APPDATA").ok_or("environment variable APPDATA is not set")?;
        let pdata = std::env::var_os("ProgramData").ok_or("environment variable ProgramData is not set")?;
        let tail = [r"Microsoft", r"Windows", r"Start Menu", r"Programs"];
        let mut user = PathBuf::from(appdata);
        let mut common = PathBuf::from(pdata);
        for t in tail {
            user.push(t);
            common.push(t);
        }
        user.push("Startup");
        common.push("StartUp");
        Ok(Startup::new(Box::new(WinRegistry), user, common, log))
    }

    fn raw(&self, source: StartupSource) -> Result<Vec<(String, String)>, String> {
        match source {
            StartupSource::HkcuRun => self.reg.string_values(Hive::CurrentUser, RUN_KEY),
            StartupSource::HklmRun => self.reg.string_values(Hive::LocalMachine, RUN_KEY),
            StartupSource::HklmRun32 => self.reg.string_values(Hive::LocalMachine, RUN32_KEY),
            StartupSource::UserFolder | StartupSource::CommonFolder => {
                let dir = if source == StartupSource::UserFolder { &self.user_folder } else { &self.common_folder };
                let rd = match std::fs::read_dir(dir) {
                    Ok(rd) => rd,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
                    Err(e) => return Err(format!("{}: {e}", dir.display())),
                };
                let mut v = Vec::new();
                for e in rd.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    if e.file_type().map(|t| t.is_file()).unwrap_or(false) && !name.eq_ignore_ascii_case("desktop.ini") {
                        v.push((name, e.path().display().to_string()));
                    }
                }
                v.sort();
                Ok(v)
            }
        }
    }

    pub fn list(&self) -> StartupList {
        let mut out = StartupList { entries: Vec::new(), errors: Vec::new() };
        for source in ALL_SOURCES {
            let (hive, key) = source.approved();
            match self.raw(source) {
                Err(e) => out.errors.push(e),
                Ok(items) => {
                    for (name, command) in items {
                        let approved = self.reg.binary_value(hive, &key, &name).unwrap_or(None);
                        out.entries.push(StartupEntry {
                            id: format!("{}:{name}", source.code()),
                            source,
                            name,
                            command,
                            enabled: is_enabled(approved.as_deref()),
                        });
                    }
                }
            }
        }
        out
    }

    /// Số app đang bật (cho dòng `startup_apps` của khám nhanh). Nguồn nào lỗi thì cả số là «không đo được».
    pub fn enabled_count(&self) -> Result<u32, String> {
        let l = self.list();
        if let Some(e) = l.errors.first() {
            return Err(e.clone());
        }
        Ok(l.entries.iter().filter(|e| e.enabled).count() as u32)
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<StartupEntry, String> {
        let entry = self.list().entries.into_iter().find(|e| e.id == id).ok_or("unknown_entry")?;
        let (hive, key) = entry.source.approved();
        self.reg.set_binary(hive, &key, &entry.name, &approved_bytes(enabled, filetime_now()))?;
        let _ = self.log.line(&format!(
            "{} {} {}",
            if enabled { "STARTUP_ENABLE" } else { "STARTUP_DISABLE" },
            entry.source.code(),
            entry.name
        ));
        Ok(StartupEntry { enabled, ..entry })
    }
}
```

- [ ] **Step 4: Chạy test và clippy**

Run: `cargo test health::startup`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
Expected: `5 passed`; toàn bộ `0 failed`. Test `real_registry_listing_reads_without_error` chỉ đọc registry thật; mọi test ghi dùng `FakeReg` trong bộ nhớ.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-diagnose/src/health/startup.rs
rtk git commit -m "feat(diagnose): danh sách app khởi động, bật/tắt qua StartupApproved như Task Manager

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Giao diện mục Ổ đĩa

**Files:**
- Create: `src/features/kham-may/disk/treeModel.ts`, `disk/treeModel.test.ts`, `disk/DiskView.tsx`, `disk/DiskView.test.tsx`

**Interfaces:**
- Consumes (Task 2): kiểu `KhamMayApi`, `NodeView`, `ChildrenPage`, `Removed`, `RestSummary`, `ScanPlan`, `ScanStatus`, `VolumeInfo`, `Notify`; `tk`; `formatBytes`, `formatCount`, `formatDate`, `formatSeconds`, `friendly`; `Spin`; lớp CSS `km-*`; `testing/fakeApi` (`fakeApi`, `PAGES`, `ROOT`, `node`, `deferred`, `GB`).
- Produces:
  - `treeModel.ts`: `SortKey = 'size' | 'name' | 'modified'`, `TreeModel`, `emptyTree`, `startTree(root)`, `withPage(m, page)`, `collapse(m, id)`, `reopen(m, id) -> TreeModel | null`, `Row`, `visibleRows(m, key?)`, `share(bytes, parentBytes)`, `applyDelete(m, id, removed)`, `isHiberfil(n)`.
  - `DiskView.tsx`: `DiskView({ api: KhamMayApi; notify: Notify })`, `walkReasonText(plan: ScanPlan): string | null`.

- [ ] **Step 1: Viết test hỏng**

`src/features/kham-may/disk/treeModel.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { GB, PAGES, ROOT, node } from '../testing/fakeApi';
import { applyDelete, collapse, isHiberfil, reopen, share, startTree, visibleRows, withPage } from './treeModel';

function opened() {
  let m = startTree(ROOT);
  for (const id of [0, 1, 2, 3]) m = withPage(m, PAGES[id]);
  return m;
}

describe('cây phía giao diện', () => {
  it('trải các tầng đang mở, dòng tổng ở cuối tầng', () => {
    const rows = visibleRows(withPage(startTree(ROOT), PAGES[0]));
    expect(rows.map((r) => (r.kind === 'node' ? r.node.name : 'rest'))).toEqual(['Users', 'Windows', 'hiberfil.sys', 'rest']);
    const rest = rows[3];
    expect(rest.kind === 'rest' && rest.rest.count).toBe(3);
  });

  it('mở sâu thì thụt lề, đóng thì ẩn con nhưng nhớ để mở lại', () => {
    const m = opened();
    const rows = visibleRows(m);
    const film = rows.find((r) => r.kind === 'node' && r.node.name === 'film.mkv');
    expect(film && film.depth).toBe(3);
    const closed = collapse(m, 1);
    expect(visibleRows(closed).some((r) => r.kind === 'node' && r.node.name === 'an')).toBe(false);
    const again = reopen(closed, 1);
    expect(again && visibleRows(again).some((r) => r.kind === 'node' && r.node.name === 'an')).toBe(true);
    expect(reopen(m, 99)).toBeNull();
  });

  it('sắp theo tên hoặc ngày sửa, mặc định theo dung lượng', () => {
    let m = startTree(ROOT);
    m = withPage(m, PAGES[0]);
    const byName = visibleRows(m, 'name').filter((r) => r.kind === 'node').map((r) => (r.kind === 'node' ? r.node.name : ''));
    expect(byName).toEqual(['hiberfil.sys', 'Users', 'Windows']);
  });

  it('% so với cha an toàn với cha 0 byte', () => {
    expect(share(5, 10)).toBe(50);
    expect(share(5, 0)).toBe(0);
    expect(share(20, 10)).toBe(100);
  });

  it('xóa: bỏ dòng và trừ dung lượng khỏi mọi tổ tiên tới gốc', () => {
    const m = applyDelete(opened(), 3, { bytes: 30 * GB, files: 4 });
    const names = visibleRows(m).map((r) => (r.kind === 'node' ? r.node.name : 'rest'));
    expect(names).not.toContain('Downloads');
    expect(names).not.toContain('film.mkv');
    expect(m.root?.bytes).toBe(30 * GB);
    expect(m.pages[0].items.find((i) => i.name === 'Users')?.bytes).toBe(0);
    expect(m.pages[1].parent.bytes).toBe(0);
    expect(m.pages[2].parent.files).toBe(0);
    expect(m.pages[3]).toBeUndefined();
  });

  it('xóa một id không có trên màn thì không đổi gì', () => {
    const m = opened();
    expect(applyDelete(m, 999, { bytes: 1, files: 1 })).toBe(m);
  });

  it('nhận ra hiberfil.sys ở gốc ổ, không nhận ở chỗ khác', () => {
    expect(isHiberfil(node(6, 'hiberfil.sys', 1, { path: 'C:\\hiberfil.sys' }))).toBe(true);
    expect(isHiberfil(node(7, 'hiberfil.sys', 1, { path: 'C:\\old\\hiberfil.sys' }))).toBe(false);
  });
});
```

`src/features/kham-may/disk/DiskView.test.tsx`:

```tsx
import { describe, expect, it, vi } from 'vitest';
import { act, configure, fireEvent, render, screen, within } from '@testing-library/react';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import type { ReactNode } from 'react';
import type { KhamMayApi, ScanSummary } from '../api/types';
import { deferred, fakeApi, ROOT } from '../testing/fakeApi';
import { DiskView, walkReasonText } from './DiskView';

// Hộp thoại Fluent vẽ chậm khi nhiều file test chạy song song trên máy yếu: nới thời gian chờ findBy*.
configure({ asyncUtilTimeout: 5000 });

function setup(over: Partial<KhamMayApi> = {}) {
  const api = fakeApi(over);
  const notify = vi.fn();
  const wrap = (ui: ReactNode) => render(<FluentProvider theme={webLightTheme}>{ui}</FluentProvider>);
  wrap(<DiskView api={api} notify={notify} />);
  return { api, notify };
}

async function scanned(over: Partial<KhamMayApi> = {}) {
  const s = setup(over);
  fireEvent.click(await screen.findByRole('button', { name: 'Quét' }));
  await screen.findByText('Users');
  return s;
}

/** Dòng bảng chứa `name`. Nút trong dòng và trong hộp thoại tra với `hidden: true`: bộ quản lý modal của
 *  Fluent (tabster) trong jsdom có lúc gắn `aria-hidden` nhầm chỗ khi nhiều file test chạy song song. */
function row(name: string): HTMLElement {
  return screen.getByText(name).closest('tr') as HTMLElement;
}

async function openTo(...names: string[]) {
  for (const n of names) {
    fireEvent.click(await screen.findByRole('button', { name: `Mở ${n}` }));
  }
}

describe('Ổ đĩa', () => {
  it('liệt kê ổ, chọn sẵn C:, quét rồi hiện tầng đầu có 🔒 và dòng tổng', async () => {
    const { api } = await scanned();
    expect(api.diskScan).toHaveBeenCalledWith('C:\\', expect.any(Function));
    expect(api.treeChildren).toHaveBeenCalledWith(0);
    expect(within(row('Windows')).getByText('🔒')).toBeTruthy();
    expect(within(row('Users')).getByText('🔒')).toBeTruthy();
    expect(screen.getByText('(+3 mục nhỏ khác, 2 GB)')).toBeTruthy();
    expect(screen.getByText('Quét xong trong 4,2 giây.')).toBeTruthy();
  });

  it('quét chậm thì băng hổ phách nêu lý do nguyên văn', async () => {
    const plan = { mode: 'walk' as const, reason: { code: 'mft_failed' as const, message: 'Access is denied. (os error 5)' } };
    const { notify } = await scanned({ diskScan: vi.fn(async () => ({ root: ROOT, plan, elapsed_ms: 1 })) });
    expect(notify).toHaveBeenCalledWith('warning', 'Đang dùng chế độ quét chậm vì không đọc được bảng MFT (Access is denied. (os error 5)).');
    expect(walkReasonText({ mode: 'walk', reason: { code: 'not_ntfs', fs: 'FAT32' } })).toContain('FAT32');
  });

  it('đang quét có vòng quay, thanh tiến độ, số file và nút Hủy', async () => {
    const run = deferred<ScanSummary>();
    let report: (s: { files: number; bytes: number; current: string; percent: number | null }) => void = () => {};
    const { api } = setup({
      diskScan: vi.fn((_r: string, cb) => {
        report = cb;
        return run.promise;
      }),
    });
    fireEvent.click(await screen.findByRole('button', { name: 'Quét' }));
    act(() => report({ files: 12345, bytes: 3 * 1024 ** 3, current: 'C:\\Windows\\WinSxS', percent: 40 }));
    expect(screen.getAllByRole('progressbar').some((e) => e.getAttribute('aria-valuenow') === '0.4')).toBe(true);
    expect(screen.getByText(/12\.345 file · 3 GB/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Hủy' }));
    expect(api.diskScanCancel).toHaveBeenCalled();
    expect(await screen.findByRole('button', { name: 'Đang hủy…' })).toBeTruthy();
    await act(async () => run.reject('cancelled'));
    expect(await screen.findByRole('button', { name: 'Quét' })).toBeTruthy();
  });

  it('hủy giữa chừng không báo lỗi; lỗi thật thì băng đỏ nguyên văn', async () => {
    const { notify } = setup({ diskScan: vi.fn(async () => Promise.reject('cancelled')) });
    fireEvent.click(await screen.findByRole('button', { name: 'Quét' }));
    await screen.findByRole('button', { name: 'Quét' });
    expect(notify).not.toHaveBeenCalled();
  });

  it('lỗi quét thật lên băng đỏ', async () => {
    const { notify } = setup({ diskScan: vi.fn(async () => Promise.reject('The device is not ready.')) });
    fireEvent.click(await screen.findByRole('button', { name: 'Quét' }));
    await vi.waitFor(() => expect(notify).toHaveBeenCalledWith('error', 'Không quét được: The device is not ready.'));
  });

  it('mở tầng con, xóa vào Thùng rác: hộp xác nhận ghi tên, dung lượng, số file; xong trừ ngay', async () => {
    const { api } = await scanned();
    await openTo('Users', 'an');
    await screen.findByText('Downloads');
    fireEvent.click(within(row('Downloads')).getByRole('button', { name: 'Xóa vào Thùng rác', hidden: true }));
    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByText(/«Downloads» · 30 GB · 4 file/)).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Xóa vào Thùng rác', hidden: true }));
    await screen.findByText('Đã chuyển «Downloads» (30 GB) vào Thùng rác.');
    expect(api.diskDelete).toHaveBeenCalledWith(3);
    expect(screen.queryByText('Downloads')).toBeNull();
    expect(within(row('Users')).getByText('0 MB')).toBeTruthy();
    expect(within(row('C:\\')).getByText('30 GB')).toBeTruthy();
  });

  it('dòng được bảo vệ khóa nút xóa; đang xóa thì khóa mọi nút xóa khác', async () => {
    const del = deferred<never>();
    await scanned({ diskDelete: vi.fn(() => del.promise) });
    expect((within(row('Windows')).getByRole('button', { name: 'Xóa vào Thùng rác', hidden: true }) as HTMLButtonElement).disabled).toBe(true);
    await openTo('Users', 'an');
    await screen.findByText('Downloads');
    fireEvent.click(within(row('Downloads')).getByRole('button', { name: 'Xóa vào Thùng rác', hidden: true }));
    fireEvent.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Xóa vào Thùng rác', hidden: true }));
    expect(await screen.findByText('Đang xóa…')).toBeTruthy();
    const others = screen.getAllByRole('button', { name: 'Xóa vào Thùng rác', hidden: true });
    expect(others.every((b) => (b as HTMLButtonElement).disabled)).toBe(true);
  });

  it('xóa bị từ chối vì bảo vệ thì băng đỏ câu dễ hiểu, cây giữ nguyên', async () => {
    const { notify } = await scanned({ diskDelete: vi.fn(async () => Promise.reject('protected')) });
    await openTo('Users', 'an');
    await screen.findByText('Downloads');
    fireEvent.click(within(row('Downloads')).getByRole('button', { name: 'Xóa vào Thùng rác', hidden: true }));
    fireEvent.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Xóa vào Thùng rác', hidden: true }));
    await vi.waitFor(() =>
      expect(notify).toHaveBeenCalledWith('error', 'Không xóa được «Downloads»: Đây là thư mục của Windows hoặc của chương trình — không xóa được.'),
    );
    expect(screen.getByText('Downloads')).toBeTruthy();
  });

  it('mở tầng hỏng thì băng hổ phách, cây cũ vẫn còn', async () => {
    const { notify } = await scanned({
      treeChildren: vi.fn(async (id: number) => {
        if (id === 0) return (await import('../testing/fakeApi')).PAGES[0];
        throw 'unknown_node';
      }),
    });
    fireEvent.click(screen.getByRole('button', { name: 'Mở Users' }));
    await vi.waitFor(() => expect(notify).toHaveBeenCalledWith('warning', 'Không mở được thư mục: Cây thư mục đã cũ, hãy quét lại.'));
    expect(screen.getByText('Windows')).toBeTruthy();
  });

  it('hiberfil.sys có gợi ý tắt ngủ đông, chỉ là chữ', async () => {
    const { api } = await scanned();
    expect(screen.getByText(/Tắt ngủ đông để lấy lại 8 GB/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Cách làm' }));
    expect(await screen.findByText(/powercfg \/hibernate off/)).toBeTruthy();
    expect(api.diskDelete).not.toHaveBeenCalled();
  });

  it('mở Explorer và sao chép đường dẫn', async () => {
    const { api } = await scanned();
    fireEvent.click(within(row('Users')).getByRole('button', { name: 'Mở trong Explorer', hidden: true }));
    fireEvent.click(within(row('Users')).getByRole('button', { name: 'Sao chép đường dẫn', hidden: true }));
    expect(api.diskReveal).toHaveBeenCalledWith(1);
    await vi.waitFor(() => expect(api.copyText).toHaveBeenCalledWith('C:\\Users'));
    expect(await screen.findByText('Đã chép đường dẫn.')).toBeTruthy();
  });

  it('không đọc được danh sách ổ thì băng đỏ', async () => {
    const { notify } = setup({ diskVolumes: vi.fn(async () => Promise.reject('RPC failed')) });
    await vi.waitFor(() => expect(notify).toHaveBeenCalledWith('error', 'Không đọc được danh sách ổ đĩa: RPC failed'));
  });
});
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `npx vitest run src/features/kham-may/disk`
Expected: FAIL — `Failed to resolve import "./treeModel"`, `"./DiskView"`.

- [ ] **Step 3: Viết mã**

`src/features/kham-may/disk/treeModel.ts`:

```ts
import type { ChildrenPage, NodeView, Removed, RestSummary } from '../api/types';

export type SortKey = 'size' | 'name' | 'modified';

/** Trạng thái cây phía giao diện: chỉ giữ những tầng người dùng đã mở (lõi giữ cả cây). */
export interface TreeModel {
  root: NodeView | null;
  pages: Record<number, ChildrenPage>;
  expanded: Record<number, boolean>;
  parentOf: Record<number, number>;
}

export const emptyTree: TreeModel = { root: null, pages: {}, expanded: {}, parentOf: {} };

export function startTree(root: NodeView): TreeModel {
  return { root, pages: {}, expanded: { [root.id]: true }, parentOf: {} };
}

/** Lưu một tầng vừa tải và mở nó. */
export function withPage(m: TreeModel, page: ChildrenPage): TreeModel {
  const parentOf = { ...m.parentOf };
  for (const it of page.items) parentOf[it.id] = page.parent.id;
  return { ...m, pages: { ...m.pages, [page.parent.id]: page }, expanded: { ...m.expanded, [page.parent.id]: true }, parentOf };
}

export function collapse(m: TreeModel, id: number): TreeModel {
  return { ...m, expanded: { ...m.expanded, [id]: false } };
}

/** Đã có tầng con trong bộ nhớ thì mở lại không cần gọi lõi. */
export function reopen(m: TreeModel, id: number): TreeModel | null {
  return m.pages[id] ? { ...m, expanded: { ...m.expanded, [id]: true } } : null;
}

export type Row =
  | { kind: 'node'; node: NodeView; depth: number; parentBytes: number; expanded: boolean }
  | { kind: 'rest'; parentId: number; rest: RestSummary; depth: number };

function sorted(items: NodeView[], key: SortKey): NodeView[] {
  if (key === 'size') return items;
  const copy = [...items];
  if (key === 'name') copy.sort((a, b) => a.name.localeCompare(b.name, 'vi'));
  else copy.sort((a, b) => b.modified - a.modified);
  return copy;
}

/** Trải cây đang mở thành các dòng bảng, gồm dòng tổng «(+N mục nhỏ khác)» cuối mỗi tầng. */
export function visibleRows(m: TreeModel, key: SortKey = 'size'): Row[] {
  const out: Row[] = [];
  if (!m.root) return out;
  const walk = (parent: NodeView, depth: number) => {
    const page = m.pages[parent.id];
    if (!page || !m.expanded[parent.id]) return;
    for (const n of sorted(page.items, key)) {
      const expanded = !!m.expanded[n.id] && !!m.pages[n.id];
      out.push({ kind: 'node', node: n, depth, parentBytes: page.parent.bytes, expanded });
      if (expanded) walk(n, depth + 1);
    }
    if (page.rest) out.push({ kind: 'rest', parentId: parent.id, rest: page.rest, depth });
  };
  walk(m.root, 0);
  return out;
}

/** % so với cha, 0–100, không bao giờ NaN. */
export function share(bytes: number, parentBytes: number): number {
  if (!(parentBytes > 0)) return 0;
  return Math.min(100, Math.max(0, (bytes / parentBytes) * 100));
}

function minus(n: NodeView, r: Removed): NodeView {
  return { ...n, bytes: Math.max(0, n.bytes - r.bytes), files: Math.max(0, n.files - r.files) };
}

/**
 * Sau khi xóa vào Thùng rác: bỏ dòng đó khỏi tầng cha và trừ dung lượng/số file khỏi MỌI tổ tiên
 * đang hiện trên màn (kể cả gốc) — không quét lại (spec 3.3).
 */
export function applyDelete(m: TreeModel, id: number, removed: Removed): TreeModel {
  const parentId = m.parentOf[id];
  if (parentId === undefined) return m;
  const ancestors = new Set<number>();
  for (let cur: number | undefined = parentId; cur !== undefined; cur = m.parentOf[cur]) ancestors.add(cur);
  const pages: Record<number, ChildrenPage> = {};
  for (const [k, page] of Object.entries(m.pages)) {
    const pid = Number(k);
    if (pid === id) continue;
    let items = page.items.map((it) => (ancestors.has(it.id) ? minus(it, removed) : it));
    if (pid === parentId) items = items.filter((it) => it.id !== id);
    pages[pid] = { ...page, parent: ancestors.has(pid) ? minus(page.parent, removed) : page.parent, items };
  }
  const root = m.root && ancestors.has(m.root.id) ? minus(m.root, removed) : m.root;
  const parentOf = { ...m.parentOf };
  delete parentOf[id];
  return { ...m, root, pages, parentOf };
}

/** hiberfil.sys ngay dưới gốc một ổ ⇒ hiện gợi ý tắt ngủ đông. */
export function isHiberfil(n: NodeView): boolean {
  return /^[a-z]:\\hiberfil\.sys$/i.test(n.path);
}
```

`src/features/kham-may/disk/DiskView.tsx`:

```tsx
import { useEffect, useMemo, useRef, useState } from 'react';
import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Field,
  ProgressBar,
  Select,
} from '@fluentui/react-components';
import type { KhamMayApi, NodeView, Notify, ScanPlan, ScanStatus, VolumeInfo } from '../api/types';
import { formatBytes, formatCount, formatDate, formatSeconds, friendly } from '../fmt';
import { tk } from '../i18n';
import { Spin } from '../ui/Spin';
import { applyDelete, collapse, emptyTree, isHiberfil, reopen, share, startTree, visibleRows, withPage, type SortKey, type TreeModel } from './treeModel';

type Phase = 'idle' | 'scanning' | 'cancelling' | 'done';

export function walkReasonText(plan: ScanPlan): string | null {
  if (plan.mode === 'mft') return null;
  const r = plan.reason;
  const reason =
    r.code === 'not_ntfs'
      ? tk('km.disk.reason.not_ntfs', { fs: r.fs })
      : r.code === 'not_local'
        ? tk('km.disk.reason.not_local')
        : tk('km.disk.reason.mft_failed', { message: r.message });
  return tk('km.disk.slowMode', { reason });
}

function pickDefault(vols: VolumeInfo[]): string {
  return (vols.find((v) => v.root.toUpperCase() === 'C:\\') ?? vols[0])?.root ?? '';
}

export function DiskView({ api, notify }: { api: KhamMayApi; notify: Notify }) {
  const [volumes, setVolumes] = useState<VolumeInfo[] | null>(null);
  const [selected, setSelected] = useState('');
  const [phase, setPhase] = useState<Phase>('idle');
  const [progress, setProgress] = useState<ScanStatus | null>(null);
  const [tree, setTree] = useState<TreeModel>(emptyTree);
  const [elapsed, setElapsed] = useState<number | null>(null);
  const [loadingNode, setLoadingNode] = useState<number | null>(null);
  const [sort, setSort] = useState<SortKey>('size');
  const [confirm, setConfirm] = useState<NodeView | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [status, setStatus] = useState('');
  const [guide, setGuide] = useState(false);
  const alive = useRef(true);

  useEffect(() => {
    alive.current = true;
    api
      .diskVolumes()
      .then((v) => {
        if (!alive.current) return;
        setVolumes(v);
        setSelected(pickDefault(v));
      })
      .catch((e) => {
        if (!alive.current) return;
        setVolumes([]);
        notify('error', tk('km.disk.volumesFailed', { message: friendly(e) }));
      });
    return () => {
      alive.current = false;
    };
  }, [api, notify]);

  const rows = useMemo(() => visibleRows(tree, sort), [tree, sort]);

  async function loadChildren(id: number, base: TreeModel): Promise<void> {
    setLoadingNode(id);
    try {
      const page = await api.treeChildren(id);
      if (alive.current) setTree(withPage(base, page));
    } catch (e) {
      notify('warning', tk('km.disk.loadFailed', { message: friendly(e) }));
    } finally {
      if (alive.current) setLoadingNode(null);
    }
  }

  async function scan() {
    setPhase('scanning');
    setProgress(null);
    setStatus('');
    setElapsed(null);
    try {
      const sum = await api.diskScan(selected, (s) => alive.current && setProgress(s));
      if (!alive.current) return;
      const slow = walkReasonText(sum.plan);
      if (slow) notify('warning', slow);
      const base = startTree(sum.root);
      setTree(base);
      setElapsed(sum.elapsed_ms);
      setPhase('done');
      await loadChildren(sum.root.id, base);
    } catch (e) {
      if (!alive.current) return;
      setPhase(tree.root ? 'done' : 'idle');
      if (friendly(e) !== 'cancelled') notify('error', tk('km.disk.scanFailed', { message: friendly(e) }));
    }
  }

  async function cancel() {
    setPhase('cancelling');
    try {
      await api.diskScanCancel();
    } catch (e) {
      notify('warning', friendly(e));
    }
  }

  function toggle(n: NodeView, expanded: boolean) {
    if (expanded) {
      setTree(collapse(tree, n.id));
      return;
    }
    const again = reopen(tree, n.id);
    if (again) setTree(again);
    else void loadChildren(n.id, tree);
  }

  async function reveal(n: NodeView) {
    try {
      await api.diskReveal(n.id);
    } catch (e) {
      notify('warning', tk('km.disk.revealFailed', { message: friendly(e) }));
    }
  }

  async function copy(n: NodeView) {
    try {
      await api.copyText(n.path);
      setStatus(tk('km.disk.copied'));
    } catch (e) {
      notify('warning', tk('km.disk.copyFailed', { message: friendly(e) }));
    }
  }

  async function doDelete() {
    const n = confirm;
    if (!n) return;
    setDeleting(true);
    try {
      const r = await api.diskDelete(n.id);
      setTree((t) => applyDelete(t, n.id, r.removed));
      setStatus(tk('km.disk.deleted', { name: n.name, size: formatBytes(r.removed.bytes) }));
      if (r.log_error) notify('warning', tk('km.err.logWrite', { message: r.log_error }));
    } catch (e) {
      notify('error', tk('km.disk.deleteFailed', { name: n.name, message: friendly(e) }));
    } finally {
      setDeleting(false);
      setConfirm(null);
    }
  }

  const busyScan = phase === 'scanning' || phase === 'cancelling';

  return (
    <section className="km-root" aria-label={tk('km.tab.disk')}>
      <div className="km-toolbar">
        <Field label={tk('km.disk.volume')}>
          {volumes === null ? (
            <Spin />
          ) : (
            <Select value={selected} disabled={busyScan} onChange={(_, d) => setSelected(d.value)}>
              {volumes.map((v) => (
                <option key={v.root} value={v.root}>
                  {tk('km.disk.volumeOption', { root: v.root, label: v.label, free: formatBytes(v.free), total: formatBytes(v.total) })}
                </option>
              ))}
            </Select>
          )}
        </Field>
        {busyScan ? (
          <Button onClick={() => void cancel()} disabled={phase === 'cancelling'}>
            {phase === 'cancelling' ? tk('km.disk.cancelling') : tk('km.disk.cancel')}
          </Button>
        ) : (
          <Button appearance="primary" disabled={!selected || deleting} onClick={() => void scan()}>
            {tree.root ? tk('km.disk.rescan') : tk('km.disk.scan')}
          </Button>
        )}
        {tree.root && !busyScan && (
          <Field label={tk('km.disk.sort')}>
            <Select value={sort} onChange={(_, d) => setSort(d.value as SortKey)}>
              <option value="size">{tk('km.disk.sort.size')}</option>
              <option value="name">{tk('km.disk.sort.name')}</option>
              <option value="modified">{tk('km.disk.sort.modified')}</option>
            </Select>
          </Field>
        )}
      </div>

      {busyScan && (
        <div className="km-root" aria-live="polite">
          <Spin label={tk('km.disk.scanning', { root: selected })} />
          <ProgressBar value={progress?.percent != null ? progress.percent / 100 : undefined} />
          {progress && (
            <span className="km-muted">
              {tk('km.disk.progress', { files: formatCount(progress.files), size: formatBytes(progress.bytes) })} · {progress.current}
            </span>
          )}
        </div>
      )}

      {!busyScan && !tree.root && <p className="km-muted">{tk('km.disk.hint')}</p>}
      {!busyScan && elapsed !== null && <p className="km-muted">{tk('km.disk.done', { seconds: formatSeconds(elapsed) })}</p>}
      <p className="km-muted" aria-live="polite">
        {status}
      </p>

      {tree.root && !busyScan && (
        <div className="km-overlay-host">
          {loadingNode !== null && (
            <div className="km-overlay">
              <Spin size="small" />
            </div>
          )}
          <table className="km-tree">
            <thead>
              <tr>
                <th>{tk('km.disk.col.name')}</th>
                <th className="km-num">{tk('km.disk.col.size')}</th>
                <th>{tk('km.disk.col.share')}</th>
                <th>{tk('km.disk.col.modified')}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              <tr>
                <td>
                  <strong>{tree.root.name}</strong> 🔒
                </td>
                <td className="km-num">{formatBytes(tree.root.bytes)}</td>
                <td />
                <td />
                <td />
              </tr>
              {rows.map((r) =>
                r.kind === 'rest' ? (
                  <tr key={`rest-${r.parentId}`}>
                    <td className="km-muted" style={{ paddingLeft: 24 + r.depth * 18 }}>
                      {tk('km.disk.rest', { count: formatCount(r.rest.count), size: formatBytes(r.rest.bytes) })}
                    </td>
                    <td className="km-num km-muted">{formatBytes(r.rest.bytes)}</td>
                    <td />
                    <td />
                    <td />
                  </tr>
                ) : (
                  <tr key={r.node.id}>
                    <td style={{ paddingLeft: 6 + r.depth * 18 }}>
                      <div className="km-tree-name">
                        {r.node.is_dir && r.node.has_children && !r.node.unreadable ? (
                          <Button
                            size="small"
                            appearance="transparent"
                            aria-label={tk(r.expanded ? 'km.disk.collapse' : 'km.disk.expand', { name: r.node.name })}
                            disabled={loadingNode !== null}
                            onClick={() => toggle(r.node, r.expanded)}
                          >
                            {r.expanded ? '▾' : '▸'}
                          </Button>
                        ) : (
                          <span style={{ width: 24, display: 'inline-block' }} />
                        )}
                        <span className="km-tree-label" title={r.node.path}>
                          {r.node.name}
                        </span>
                        {r.node.protected && <span title={tk('km.disk.protected')}>🔒</span>}
                        {r.node.unreadable && <span className="km-muted">{tk('km.disk.unreadable')}</span>}
                        {r.node.is_link && <span className="km-muted" title={tk('km.disk.link')}>↪</span>}
                        {loadingNode === r.node.id && <Spin size="extra-tiny" />}
                      </div>
                      {isHiberfil(r.node) && (
                        <div className="km-muted">
                          {tk('km.disk.hiberfil', { size: formatBytes(r.node.bytes) })}{' '}
                          <Button size="small" appearance="transparent" onClick={() => setGuide(true)}>
                            {tk('km.disk.hiberfilGuide')}
                          </Button>
                        </div>
                      )}
                    </td>
                    <td className="km-num">{formatBytes(r.node.bytes)}</td>
                    <td className="km-share">
                      <div className="km-share-bar" role="img" aria-label={`${Math.round(share(r.node.bytes, r.parentBytes))}%`}>
                        <div className="km-share-fill" style={{ width: `${share(r.node.bytes, r.parentBytes)}%` }} />
                      </div>
                    </td>
                    <td className="km-muted">{formatDate(r.node.modified)}</td>
                    <td>
                      <div className="km-row-actions">
                        <Button size="small" appearance="subtle" onClick={() => void reveal(r.node)}>
                          {tk('km.disk.reveal')}
                        </Button>
                        <Button size="small" appearance="subtle" onClick={() => void copy(r.node)}>
                          {tk('km.disk.copy')}
                        </Button>
                        <Button
                          size="small"
                          appearance="subtle"
                          disabled={r.node.protected || deleting}
                          title={r.node.protected ? tk('km.disk.protected') : undefined}
                          onClick={() => setConfirm(r.node)}
                        >
                          {tk('km.disk.delete')}
                        </Button>
                      </div>
                    </td>
                  </tr>
                ),
              )}
            </tbody>
          </table>
        </div>
      )}

      <Dialog open={confirm !== null} onOpenChange={(_, d) => !d.open && !deleting && setConfirm(null)}>
        <DialogSurface>
          <DialogBody>
            <DialogTitle>{tk('km.disk.deleteTitle')}</DialogTitle>
            <DialogContent>
              {confirm &&
                tk('km.disk.deleteBody', { name: confirm.name, size: formatBytes(confirm.bytes), files: formatCount(confirm.files) })}
            </DialogContent>
            <DialogActions>
              {deleting ? (
                <Spin label={tk('km.disk.deleting')} />
              ) : (
                <>
                  <Button appearance="primary" onClick={() => void doDelete()}>
                    {tk('km.disk.deleteConfirm')}
                  </Button>
                  <Button onClick={() => setConfirm(null)}>{tk('km.disk.deleteCancel')}</Button>
                </>
              )}
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>

      <Dialog open={guide} onOpenChange={(_, d) => setGuide(d.open)}>
        <DialogSurface>
          <DialogBody>
            <DialogTitle>{tk('km.disk.hiberfilTitle')}</DialogTitle>
            <DialogContent>{tk('km.disk.hiberfilBody')}</DialogContent>
            <DialogActions>
              <Button onClick={() => setGuide(false)}>{tk('km.guide.close')}</Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </section>
  );
}
```

- [ ] **Step 4: Chạy test, thấy qua**

Run: `npx vitest run src/features/kham-may/disk` rồi `npm run typecheck`
Expected: `Tests  19 passed`; typecheck sạch. Chạy thêm `npm test` ba lần liền: không test nào chập chờn (đã đo khi viết kế hoạch: 3/3 lần xanh sau khi tra nút trong dòng/hộp thoại với `hidden: true` — xem chú thích trong test).

- [ ] **Step 5: Commit**

```bash
rtk git add src/features/kham-may/disk/treeModel.ts
rtk git add src/features/kham-may/disk/treeModel.test.ts
rtk git add src/features/kham-may/disk/DiskView.tsx
rtk git add src/features/kham-may/disk/DiskView.test.tsx
rtk git commit -m "feat(kham-may): mục Ổ đĩa — chọn ổ, quét có tiến độ và Hủy, cây mở/đóng, xóa vào Thùng rác

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Giao diện mục Tổng quan và danh sách khởi động

**Files:**
- Create: `src/features/kham-may/overview/findingText.ts`, `findingText.test.ts`, `StartupList.tsx`, `OverviewView.tsx`, `OverviewView.test.tsx`

**Interfaces:**
- Consumes (Task 2): `KhamMayApi`, `Finding`, `FindingId`, `Level`, `StartupEntry`, `Notify`; `tk`, `hasKey`; `friendly`; `Spin`; `fakeApi`, `FINDINGS`, `deferred`.
- Produces:
  - `findingText.ts`: `FINDING_ORDER: FindingId[]` (9 dòng, thứ tự bảng spec), `FindingAction`, `actionsFor(f)`, `levelLabel(level)`, `findingSentence(f)`, `countProblems(findings)`.
  - `StartupList.tsx`: `StartupList({ api, notify, onChanged? })`.
  - `OverviewView.tsx`: `NavTarget = 'clean' | 'disk' | 'perf'`, `OverviewView({ api, notify, onNavigate: (t: NavTarget) => void })` — khám đúng một lần mỗi lần gắn (kể cả StrictMode), dòng `cpu_throttle` đo riêng 10 giây.

- [ ] **Step 1: Viết test hỏng**

`src/features/kham-may/overview/findingText.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import type { Finding, FindingId, Level } from '../api/types';
import { hasKey } from '../i18n';
import { actionsFor, countProblems, FINDING_ORDER, findingSentence, levelLabel } from './findingText';

const f = (id: FindingId, level: Level, value: number | null = null, detail: string | null = null): Finding => ({ id, level, value, detail });

describe('câu giải thích khám nhanh', () => {
  it('mỗi dòng có câu cho mức Ổn và mức vấn đề', () => {
    for (const id of FINDING_ORDER) {
      expect(hasKey(`km.finding.${id}.ok`), id).toBe(true);
      expect(hasKey(`km.finding.${id}.warn`) || hasKey(`km.finding.${id}.critical`), id).toBe(true);
    }
  });

  it('số thập phân kiểu Việt Nam và tên ổ trong câu', () => {
    expect(findingSentence(f('disk_full', 'warn', 9.5))).toBe('Ổ hệ thống chỉ còn 9,5% trống — máy dễ chậm và thiếu chỗ cập nhật.');
    expect(findingSentence(f('disk_health', 'critical', null, 'WDC WD10EZEX'))).toContain('«WDC WD10EZEX»');
    expect(findingSentence(f('uptime', 'warn', 8))).toContain('8 ngày');
    expect(findingSentence(f('uptime', 'warn', 8))).toContain('Fast Startup');
  });

  it('không đo được thì ghi rõ lý do nguyên văn, không bao giờ là Ổn', () => {
    const s = findingSentence(f('cpu_hot', 'unknown', null, 'HRESULT Call failed with: 0x8004100C'));
    expect(s).toBe('Không đo được: HRESULT Call failed with: 0x8004100C');
    expect(levelLabel('unknown')).toBe('⚪ Không đo được');
  });

  it('mức nghiêm trọng không có câu riêng thì dùng câu nên xử lý', () => {
    expect(findingSentence(f('system_hdd', 'critical'))).toContain('HDD');
  });

  it('nút xử lý đúng bảng spec, chỉ khi có vấn đề', () => {
    expect(actionsFor(f('disk_full', 'critical', 3))).toEqual(['clean', 'disk']);
    expect(actionsFor(f('power_saver', 'warn'))).toEqual(['power']);
    expect(actionsFor(f('cpu_hot', 'warn', 95))).toEqual(['cooling']);
    expect(actionsFor(f('disk_full', 'ok', 40))).toEqual([]);
    expect(actionsFor(f('cpu_hot', 'unknown'))).toEqual([]);
  });

  it('đếm vấn đề', () => {
    expect(countProblems([f('disk_full', 'critical'), f('uptime', 'warn'), f('ram_pressure', 'warn'), f('cpu_hot', 'unknown')])).toEqual({ critical: 1, warn: 2 });
  });
});
```

`src/features/kham-may/overview/OverviewView.test.tsx`:

```tsx
import { describe, expect, it, vi } from 'vitest';
import { act, configure, fireEvent, render, screen, within } from '@testing-library/react';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import { StrictMode } from 'react';
import type { Finding, KhamMayApi } from '../api/types';
import { deferred, fakeApi, FINDINGS } from '../testing/fakeApi';
import { OverviewView } from './OverviewView';
import { StartupList } from './StartupList';

configure({ asyncUtilTimeout: 5000 });

function setup(over: Partial<KhamMayApi> = {}) {
  const api = fakeApi(over);
  const notify = vi.fn();
  const onNavigate = vi.fn();
  render(
    <StrictMode>
      <FluentProvider theme={webLightTheme}>
        <OverviewView api={api} notify={notify} onNavigate={onNavigate} />
      </FluentProvider>
    </StrictMode>,
  );
  return { api, notify, onNavigate };
}

function findingRow(id: string): HTMLElement {
  return document.querySelector(`[data-finding="${id}"]`) as HTMLElement;
}

describe('Tổng quan', () => {
  it('khám đúng một lần kể cả StrictMode, hiện 9 dòng theo thứ tự bảng', async () => {
    const { api } = setup();
    await screen.findByText(/Ổ hệ thống chỉ còn 9,5% trống/);
    expect(api.healthCheck).toHaveBeenCalledTimes(1);
    expect(api.healthThrottle).toHaveBeenCalledTimes(1);
    const ids = Array.from(document.querySelectorAll('[data-finding]')).map((e) => e.getAttribute('data-finding'));
    expect(ids).toEqual(['disk_full', 'disk_health', 'system_hdd', 'ram_pressure', 'startup_apps', 'uptime', 'power_saver', 'cpu_throttle', 'cpu_hot']);
    expect(await screen.findByText('0 vấn đề nghiêm trọng · 2 vấn đề nên xử lý')).toBeTruthy();
  });

  it('dòng nào xong hiện dòng đó; dòng chưa xong có vòng quay; hạ xung đo riêng 10 giây', async () => {
    const check = deferred<Finding[]>();
    const throttle = deferred<Finding>();
    let push: (f: Finding) => void = () => {};
    setup({
      healthCheck: vi.fn((cb: (f: Finding) => void) => {
        push = cb;
        return check.promise;
      }),
      healthThrottle: vi.fn(() => throttle.promise),
    });
    expect(await screen.findByText('Đang đo tốc độ CPU trong 10 giây…')).toBeTruthy();
    act(() => push(FINDINGS[0]));
    expect(within(findingRow('disk_full')).getByText('🟠 Nên xử lý')).toBeTruthy();
    expect(within(findingRow('disk_health')).getByRole('progressbar')).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Khám lại' }) as HTMLButtonElement).disabled).toBe(true);
    await act(async () => check.resolve(FINDINGS));
    await act(async () => throttle.resolve({ id: 'cpu_throttle', level: 'warn', value: null, detail: null }));
    expect(within(findingRow('cpu_throttle')).getByText('CPU đang bị hạ xung (thường do nóng hoặc chế độ nguồn).')).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Khám lại' }) as HTMLButtonElement).disabled).toBe(false);
  });

  it('nguồn không đo được ghi lý do nguyên văn', async () => {
    setup();
    expect(await screen.findByText('Không đo được: HRESULT Call failed with: 0x8004100C')).toBeTruthy();
    expect(within(findingRow('cpu_hot')).getByText('⚪ Không đo được')).toBeTruthy();
  });

  it('nút dẫn tới chỗ xử lý', async () => {
    const findings: Finding[] = [
      { id: 'disk_full', level: 'critical', value: 3, detail: null },
      { id: 'power_saver', level: 'warn', value: null, detail: null },
      { id: 'uptime', level: 'warn', value: 9, detail: null },
      { id: 'ram_pressure', level: 'warn', value: 90, detail: null },
    ];
    const { api, onNavigate } = setup({ healthCheck: vi.fn(async () => findings) });
    fireEvent.click(await screen.findByRole('button', { name: 'Dọn dẹp' }));
    expect(onNavigate).toHaveBeenCalledWith('clean');
    fireEvent.click(screen.getByRole('button', { name: 'Xem Ổ đĩa' }));
    expect(onNavigate).toHaveBeenCalledWith('disk');
    fireEvent.click(screen.getByRole('button', { name: 'Xem Bộ nhớ & Hiệu năng' }));
    expect(onNavigate).toHaveBeenCalledWith('perf');
    fireEvent.click(screen.getByRole('button', { name: 'Mở cài đặt Nguồn' }));
    expect(api.openSettings).toHaveBeenCalledWith('power');
    fireEvent.click(screen.getByRole('button', { name: 'Gợi ý' }));
    expect(await screen.findByText('Khởi động lại đúng cách')).toBeTruthy();
  });

  it('khám hỏng hẳn thì băng đỏ; đo hạ xung hỏng thì dòng đó «Không đo được»', async () => {
    const { notify } = setup({
      healthCheck: vi.fn(async () => Promise.reject('busy')),
      healthThrottle: vi.fn(async () => Promise.reject('PDH open: 0xC0000BB8')),
    });
    await vi.waitFor(() => expect(notify).toHaveBeenCalledWith('error', 'Không khám được: WinFreeUp đang bận với thao tác trước, hãy đợi xong rồi thử lại.'));
    expect(await screen.findByText('Không đo được: PDH open: 0xC0000BB8')).toBeTruthy();
  });

  it('khám lại xoá kết quả cũ và chạy lại', async () => {
    const { api } = setup();
    await screen.findByText(/9,5%/);
    await vi.waitFor(() => expect((screen.getByRole('button', { name: 'Khám lại' }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByRole('button', { name: 'Khám lại' }));
    await vi.waitFor(() => expect(api.healthCheck).toHaveBeenCalledTimes(2));
  });
});

describe('Danh sách khởi động', () => {
  function renderList(over: Partial<KhamMayApi> = {}) {
    const api = fakeApi(over);
    const notify = vi.fn();
    render(
      <FluentProvider theme={webLightTheme}>
        <StartupList api={api} notify={notify} />
      </FluentProvider>,
    );
    return { api, notify };
  }

  it('hiện số app đang bật, nguồn và dòng lệnh; bật/tắt ghi qua lõi', async () => {
    const { api } = renderList();
    expect(await screen.findByText('(1/2 đang bật)')).toBeTruthy();
    expect(screen.getByText('· Riêng bạn')).toBeTruthy();
    const sw = screen.getByRole('switch', { name: 'OneDrive' }) as HTMLInputElement;
    expect(sw.checked).toBe(true);
    fireEvent.click(sw);
    await vi.waitFor(() => expect(api.startupSet).toHaveBeenCalledWith('hkcu_run:OneDrive', false));
    expect(await screen.findByText('(0/2 đang bật)')).toBeTruthy();
  });

  it('đổi không được thì băng đỏ nguyên văn, công tắc giữ trạng thái cũ', async () => {
    const { notify } = renderList({ startupSet: vi.fn(async () => Promise.reject('RegSetValueExW: Access is denied. (os error 5)')) });
    fireEvent.click(await screen.findByRole('switch', { name: 'OneDrive' }));
    await vi.waitFor(() =>
      expect(notify).toHaveBeenCalledWith('error', 'Không đổi được «OneDrive»: RegSetValueExW: Access is denied. (os error 5)'),
    );
    expect((screen.getByRole('switch', { name: 'OneDrive' }) as HTMLInputElement).checked).toBe(true);
  });

  it('một nguồn đọc không được thì băng hổ phách, phần còn lại vẫn hiện', async () => {
    const { notify } = renderList({
      startupList: vi.fn(async () => ({ entries: [], errors: ['RegOpenKeyExW Software\\WOW6432Node: Access is denied.'] })),
    });
    expect(await screen.findByText('Không có app nào tự chạy cùng máy.')).toBeTruthy();
    expect(notify).toHaveBeenCalledWith('warning', 'Không đọc được danh sách khởi động: RegOpenKeyExW Software\\WOW6432Node: Access is denied.');
  });
});
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `npx vitest run src/features/kham-may/overview`
Expected: FAIL — `Failed to resolve import "./findingText"`, `"./OverviewView"`, `"./StartupList"`.

- [ ] **Step 3: Viết mã**

`src/features/kham-may/overview/findingText.ts`:

```ts
import type { Finding, FindingId, Level } from '../api/types';
import { hasKey, tk } from '../i18n';

/** Đúng thứ tự bảng spec mục 4. */
export const FINDING_ORDER: FindingId[] = [
  'disk_full',
  'disk_health',
  'system_hdd',
  'ram_pressure',
  'startup_apps',
  'uptime',
  'power_saver',
  'cpu_throttle',
  'cpu_hot',
];

export type FindingAction = 'clean' | 'disk' | 'backup' | 'ssd' | 'perf' | 'startup' | 'restart' | 'power' | 'cooling';

/** Cột «Nút» của bảng spec mục 4. */
const ACTIONS: Record<FindingId, FindingAction[]> = {
  disk_full: ['clean', 'disk'],
  disk_health: ['backup'],
  system_hdd: ['ssd'],
  ram_pressure: ['perf'],
  startup_apps: ['startup'],
  uptime: ['restart'],
  power_saver: ['power'],
  cpu_throttle: ['perf'],
  cpu_hot: ['cooling'],
};

/** Chỉ dòng có vấn đề (🔴/🟠) mới có nút xử lý. */
export function actionsFor(f: Finding): FindingAction[] {
  return f.level === 'critical' || f.level === 'warn' ? ACTIONS[f.id] : [];
}

export function levelLabel(level: Level): string {
  return tk(`km.level.${level}`);
}

const decimal = new Intl.NumberFormat('vi-VN', { maximumFractionDigits: 1 });

/** Câu giải thích dễ hiểu. Nguồn không đọc được ⇒ «Không đo được: <lý do nguyên văn>». */
export function findingSentence(f: Finding): string {
  if (f.level === 'unknown') return tk('km.overview.unknown', { detail: f.detail ?? '' });
  const params = { value: f.value === null ? '' : decimal.format(f.value), detail: f.detail ?? '' };
  const key = `km.finding.${f.id}.${f.level}`;
  if (hasKey(key)) return tk(key, params);
  // Dòng chỉ có một mức vấn đề (vd system_hdd chỉ 🟠) mà lõi trả mức khác: dùng câu 🟠.
  return tk(`km.finding.${f.id}.warn`, params);
}

export function countProblems(findings: Finding[]): { critical: number; warn: number } {
  return {
    critical: findings.filter((f) => f.level === 'critical').length,
    warn: findings.filter((f) => f.level === 'warn').length,
  };
}
```

`src/features/kham-may/overview/StartupList.tsx`:

```tsx
import { useEffect, useRef, useState } from 'react';
import { Switch } from '@fluentui/react-components';
import type { KhamMayApi, Notify, StartupEntry } from '../api/types';
import { friendly } from '../fmt';
import { tk } from '../i18n';
import { Spin } from '../ui/Spin';

/** Danh sách app khởi động với công tắc bật/tắt (ghi StartupApproved như Task Manager, bật lại được). */
export function StartupList({ api, notify, onChanged }: { api: KhamMayApi; notify: Notify; onChanged?: () => void }) {
  const [entries, setEntries] = useState<StartupEntry[] | null>(null);
  const [pending, setPending] = useState<Record<string, boolean>>({});
  const alive = useRef(true);

  useEffect(() => {
    alive.current = true;
    api
      .startupList()
      .then((l) => {
        if (!alive.current) return;
        setEntries(l.entries);
        for (const e of l.errors) notify('warning', tk('km.startup.failed', { message: e }));
      })
      .catch((e) => {
        if (!alive.current) return;
        setEntries([]);
        notify('error', tk('km.startup.failed', { message: friendly(e) }));
      });
    return () => {
      alive.current = false;
    };
  }, [api, notify]);

  async function toggle(e: StartupEntry, enabled: boolean) {
    setPending((p) => ({ ...p, [e.id]: true }));
    try {
      const updated = await api.startupSet(e.id, enabled);
      if (alive.current) setEntries((list) => (list ?? []).map((x) => (x.id === e.id ? { ...x, enabled: updated.enabled } : x)));
      onChanged?.();
    } catch (err) {
      notify('error', tk('km.startup.setFailed', { name: e.name, message: friendly(err) }));
    } finally {
      if (alive.current) setPending((p) => ({ ...p, [e.id]: false }));
    }
  }

  if (entries === null) return <Spin label={tk('km.startup.loading')} />;
  const enabled = entries.filter((e) => e.enabled).length;
  return (
    <section className="km-root" aria-label={tk('km.startup.title')}>
      <h3>
        {tk('km.startup.title')} <span className="km-muted">({tk('km.startup.count', { enabled, total: entries.length })})</span>
      </h3>
      {entries.length === 0 ? (
        <p className="km-muted">{tk('km.startup.empty')}</p>
      ) : (
        <ul className="km-startup">
          {entries.map((e) => (
            <li key={e.id} className="km-startup-item">
              <Switch
                checked={e.enabled}
                disabled={!!pending[e.id]}
                aria-label={e.name}
                onChange={(_, d) => void toggle(e, d.checked)}
              />
              <span>
                <strong>{e.name}</strong> <span className="km-muted">· {tk(`km.startup.source.${e.source}`)}</span>{' '}
                {pending[e.id] && <Spin size="extra-tiny" />}
              </span>
              <span className="km-muted km-startup-cmd" title={e.command}>
                {e.command}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
```

`src/features/kham-may/overview/OverviewView.tsx`:

```tsx
import { useCallback, useEffect, useRef, useState } from 'react';
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle } from '@fluentui/react-components';
import type { Finding, FindingId, KhamMayApi, Notify } from '../api/types';
import { friendly } from '../fmt';
import { tk } from '../i18n';
import { Spin } from '../ui/Spin';
import { actionsFor, countProblems, FINDING_ORDER, findingSentence, levelLabel, type FindingAction } from './findingText';
import { StartupList } from './StartupList';

export type NavTarget = 'clean' | 'disk' | 'perf';
type Guide = 'backup' | 'ssd' | 'restart' | 'cooling';

export function OverviewView({ api, notify, onNavigate }: { api: KhamMayApi; notify: Notify; onNavigate: (t: NavTarget) => void }) {
  const [findings, setFindings] = useState<Partial<Record<FindingId, Finding>>>({});
  const [checking, setChecking] = useState(false);
  const [throttling, setThrottling] = useState(false);
  const [guide, setGuide] = useState<Guide | null>(null);
  const startupRef = useRef<HTMLDivElement>(null);
  const alive = useRef(true);

  const put = useCallback((f: Finding) => {
    if (alive.current) setFindings((m) => ({ ...m, [f.id]: f }));
  }, []);

  const run = useCallback(async () => {
    setFindings({});
    setChecking(true);
    setThrottling(true);
    const throttle = api
      .healthThrottle()
      .then(put)
      .catch((e) => put({ id: 'cpu_throttle', level: 'unknown', value: null, detail: friendly(e) }))
      .finally(() => alive.current && setThrottling(false));
    try {
      const all = await api.healthCheck(put);
      all.forEach(put);
    } catch (e) {
      notify('error', tk('km.overview.failed', { message: friendly(e) }));
    } finally {
      if (alive.current) setChecking(false);
    }
    await throttle;
  }, [api, notify, put]);

  // StrictMode chạy effect hai lần: chỉ khám một lần mỗi lần mở mục, nếu không lõi sẽ trả «busy».
  const started = useRef(false);
  useEffect(() => {
    alive.current = true;
    if (!started.current) {
      started.current = true;
      void run();
    }
    return () => {
      alive.current = false;
    };
  }, [run]);

  async function act(a: FindingAction) {
    switch (a) {
      case 'clean':
      case 'disk':
      case 'perf':
        onNavigate(a);
        return;
      case 'startup':
        startupRef.current?.scrollIntoView?.({ behavior: 'smooth', block: 'start' });
        return;
      case 'power':
        try {
          await api.openSettings('power');
        } catch (e) {
          notify('warning', tk('km.perf.settingsFailed', { message: friendly(e) }));
        }
        return;
      default:
        setGuide(a);
    }
  }

  const done = FINDING_ORDER.map((id) => findings[id]).filter((f): f is Finding => !!f);
  const { critical, warn } = countProblems(done);
  const finished = !checking && !throttling;

  return (
    <section className="km-root" aria-label={tk('km.tab.overview')}>
      <div className="km-toolbar">
        <h2 style={{ margin: 0 }}>{tk('km.overview.title')}</h2>
        <Button appearance="primary" disabled={!finished} onClick={() => void run()}>
          {tk('km.overview.recheck')}
        </Button>
        {!finished && <Spin label={tk('km.overview.checking')} />}
      </div>
      {finished && (
        <p aria-live="polite">{critical + warn === 0 ? tk('km.overview.allGood') : tk('km.overview.summary', { critical, warn })}</p>
      )}
      <ul className="km-findings">
        {FINDING_ORDER.map((id) => {
          const f = findings[id];
          return (
            <li key={id} className="km-finding" data-finding={id}>
              {f ? <strong>{levelLabel(f.level)}</strong> : <Spin />}
              <span>{f ? findingSentence(f) : id === 'cpu_throttle' ? tk('km.finding.cpu_throttle.pending') : tk('km.overview.checking')}</span>
              <span className="km-finding-actions">
                {f &&
                  actionsFor(f).map((a) => (
                    <Button key={a} size="small" onClick={() => void act(a)}>
                      {tk(`km.action.${a}`)}
                    </Button>
                  ))}
              </span>
            </li>
          );
        })}
      </ul>
      <div ref={startupRef}>
        <StartupList api={api} notify={notify} />
      </div>
      <Dialog open={guide !== null} onOpenChange={(_, d) => !d.open && setGuide(null)}>
        <DialogSurface>
          <DialogBody>
            <DialogTitle>{guide && tk(`km.guide.${guide}.title`)}</DialogTitle>
            <DialogContent>{guide && tk(`km.guide.${guide}.body`)}</DialogContent>
            <DialogActions>
              <Button onClick={() => setGuide(null)}>{tk('km.guide.close')}</Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </section>
  );
}
```

- [ ] **Step 4: Chạy test, thấy qua**

Run: `npx vitest run src/features/kham-may/overview` rồi `npm run typecheck`
Expected: `Tests  15 passed`; typecheck sạch.

- [ ] **Step 5: Commit**

```bash
rtk git add src/features/kham-may/overview/findingText.ts
rtk git add src/features/kham-may/overview/findingText.test.ts
rtk git add src/features/kham-may/overview/StartupList.tsx
rtk git add src/features/kham-may/overview/OverviewView.tsx
rtk git add src/features/kham-may/overview/OverviewView.test.tsx
rtk git commit -m "feat(kham-may): mục Tổng quan — khám nhanh hiện dần từng dòng, nút xử lý, công tắc khởi động

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Giao diện mục Bộ nhớ & Hiệu năng

**Files:**
- Create: `src/features/kham-may/perf/perfModel.ts`, `perf/perfModel.test.tsx`, `perf/LineChart.tsx`, `perf/AppTable.tsx`, `perf/StutterList.tsx`, `perf/PerfView.tsx`, `perf/PerfView.test.tsx`

**Interfaces:**
- Consumes (Task 2): `KhamMayApi`, `AppRow`, `Sample`, `PerfTick`, `StutterView`, `TopApp`, `Notify`; `tk`; `formatBytes`, `formatKBps`, `formatMBps`, `formatPct`, `formatRate`, `formatClock`, `friendly`; `Spin`; `fakeApi`, `app`, `sample`, `tick`, `deferred`, `GB`.
- Produces:
  - `perfModel.ts`: `WINDOW_MS = 300000`, `pushSample(list, s)`, `AppSortKey`, `netTotal(a) -> number | null`, `sortApps(apps, key, desc)`, `Pt { t, v }`, `segments(samples, pick) -> Pt[][]` (ngắt ở chỗ thiếu số/khoảng dừng), `niceMax(values)`.
  - `LineChart.tsx`: `LineChart({ title, valueText, lines, max, now, bands })`, `Band { start_ms, end_ms }` — SVG `role="img"` `aria-label="<title>: <valueText>"`, dải đỏ `[data-band]`.
  - `AppTable.tsx`: `AppTable({ apps, icons, busy, onKill, onReveal })`; `StutterList.tsx`: `StutterList({ stutters })`.
  - `PerfView.tsx`: `PerfView({ api, notify })` — gắn ⇒ `perfStart`, gỡ ⇒ `perfStop`.

- [ ] **Step 1: Viết test hỏng**

`src/features/kham-may/perf/perfModel.test.tsx`:

```tsx
import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/react';
import { app, GB, sample } from '../testing/fakeApi';
import { LineChart } from './LineChart';
import { netTotal, niceMax, pushSample, segments, sortApps, WINDOW_MS } from './perfModel';

describe('chuỗi mẫu', () => {
  it('giữ đúng 5 phút gần nhất, bỏ trùng giờ', () => {
    let l = [sample(1000)];
    l = pushSample(l, sample(1000));
    expect(l).toHaveLength(1);
    for (let i = 1; i <= 400; i++) l = pushSample(l, sample(1000 + i * 1000));
    expect(l[l.length - 1].t_ms - l[0].t_ms).toBe(WINDOW_MS);
  });

  it('đường bị ngắt ở chỗ thiếu số liệu và ở khoảng dừng lấy mẫu', () => {
    const s = [sample(1000), sample(2000), { ...sample(3000), disk_active: null }, sample(4000), sample(60_000)];
    const segs = segments(s, (x) => x.disk_active);
    expect(segs.map((g) => g.map((p) => p.t))).toEqual([[1000, 2000], [4000], [60_000]]);
  });

  it('trục mạng tự co theo lũy thừa 2, tối thiểu 64 KB/s', () => {
    expect(niceMax([])).toBe(64 * 1024);
    expect(niceMax([3 * 1024 ** 2])).toBe(4 * 1024 ** 2);
  });
});

describe('bảng ứng dụng', () => {
  const apps = [
    app('a', 'Zalo', GB, { cpu: 5, disk_bps: 10, net_up_bps: 1, net_down_bps: 1 }),
    app('b', 'Chrome', 3 * GB, { cpu: 1, disk_bps: 50, net_up_bps: 100, net_down_bps: 900 }),
    app('c', 'Ảnh', 2 * GB, { cpu: 9, disk_bps: 0, net_up_bps: null, net_down_bps: null }),
  ];
  it('sắp theo mọi cột, hai chiều', () => {
    expect(sortApps(apps, 'ram', true).map((a) => a.name)).toEqual(['Chrome', 'Ảnh', 'Zalo']);
    expect(sortApps(apps, 'cpu', true).map((a) => a.name)).toEqual(['Ảnh', 'Zalo', 'Chrome']);
    expect(sortApps(apps, 'disk', false).map((a) => a.name)).toEqual(['Ảnh', 'Zalo', 'Chrome']);
    expect(sortApps(apps, 'net', true).map((a) => a.name)).toEqual(['Chrome', 'Zalo', 'Ảnh']);
    expect(sortApps(apps, 'name', false).map((a) => a.name)).toEqual(['Ảnh', 'Chrome', 'Zalo']);
  });

  it('mạng không theo dõi được thì là null, không phải 0', () => {
    expect(netTotal(apps[2])).toBeNull();
    expect(netTotal(apps[1])).toBe(1000);
  });
});

describe('LineChart', () => {
  it('vẽ một polyline mỗi đoạn và dải đỏ cho cơn giật trong cửa sổ', () => {
    const now = 400_000;
    const { container } = render(
      <LineChart
        title="CPU"
        valueText="20%"
        max={100}
        now={now}
        lines={[{ segments: [[{ t: 100_000, v: 50 }, { t: 400_000, v: 100 }], [{ t: 200_000, v: 0 }]] }]}
        bands={[
          { start_ms: 250_000, end_ms: 280_000 },
          { start_ms: 10_000, end_ms: 20_000 },
        ]}
      />,
    );
    const lines = container.querySelectorAll('polyline');
    expect(lines).toHaveLength(2);
    expect(lines[0].getAttribute('points')).toBe('0.0,50.0 300.0,0.0');
    const bands = container.querySelectorAll('[data-band]');
    expect(bands).toHaveLength(1);
    expect(bands[0].getAttribute('x')).toBe('150');
    expect(bands[0].getAttribute('width')).toBe('30');
    expect(container.querySelector('svg')?.getAttribute('aria-label')).toBe('CPU: 20%');
  });
});
```

`src/features/kham-may/perf/PerfView.test.tsx`:

```tsx
import { describe, expect, it, vi } from 'vitest';
import { act, configure, fireEvent, render, screen, within } from '@testing-library/react';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import type { KhamMayApi, PerfTick } from '../api/types';
import { app, deferred, fakeApi, GB, tick } from '../testing/fakeApi';
import { PerfView } from './PerfView';

configure({ asyncUtilTimeout: 5000 });

function setup(over: Partial<KhamMayApi> = {}) {
  let push: (t: PerfTick) => void = () => {};
  let fail: (m: string) => void = () => {};
  const api = fakeApi({
    perfStart: vi.fn(async (onTick: (t: PerfTick) => void, onError: (m: string) => void) => {
      push = onTick;
      fail = onError;
      return [];
    }),
    ...over,
  });
  const notify = vi.fn();
  const view = render(
    <FluentProvider theme={webLightTheme}>
      <PerfView api={api} notify={notify} />
    </FluentProvider>,
  );
  const send = (t: PerfTick) => act(() => push(t));
  return { api, notify, view, send, fail: (m: string) => act(() => fail(m)) };
}

function appRow(name: string): HTMLElement {
  return screen.getByText(name).closest('tr') as HTMLElement;
}

const T0 = new Date(2026, 8, 25, 14, 3, 12).getTime();

describe('Bộ nhớ & Hiệu năng', () => {
  it('chưa có mẫu thì vòng quay; có mẫu thì bốn biểu đồ và bảng app sắp theo RAM', async () => {
    const { api, send } = setup();
    expect(screen.getByText('Đang bắt đầu đo…')).toBeTruthy();
    await vi.waitFor(() => expect(api.perfStart).toHaveBeenCalledTimes(1));
    send(tick(T0));
    expect(screen.getByRole('img', { name: 'CPU: 20%' })).toBeTruthy();
    expect(screen.getByRole('img', { name: /^RAM: 50% · 8 GB \/ 16 GB$/ })).toBeTruthy();
    expect(screen.getByRole('img', { name: 'Hoạt động đĩa: 10%' })).toBeTruthy();
    expect(screen.getByRole('img', { name: 'Mạng: ↑ 1 KB/s · ↓ 4 KB/s' })).toBeTruthy();
    const names = screen.getAllByText(/Google Chrome|Service Host/).map((e) => e.textContent);
    expect(names).toEqual(['Google Chrome', 'Service Host']);
  });

  it('rời mục thì dừng lấy mẫu', async () => {
    const { api, view } = setup();
    await vi.waitFor(() => expect(api.perfStart).toHaveBeenCalled());
    view.unmount();
    expect(api.perfStop).toHaveBeenCalledTimes(1);
  });

  it('sắp theo cột CPU khi bấm tiêu đề, bấm lần nữa thì đảo chiều', async () => {
    const { send } = setup();
    await vi.waitFor(() => expect(screen.queryByText('Đang bắt đầu đo…')).toBeTruthy());
    send(tick(T0, { apps: [app('a.exe', 'Nhẹ', 3 * GB, { cpu: 1 }), app('b.exe', 'Nặng', GB, { cpu: 80 })] }));
    fireEvent.click(screen.getByRole('button', { name: 'CPU' }));
    expect(screen.getAllByText(/^(Nhẹ|Nặng)$/).map((e) => e.textContent)).toEqual(['Nặng', 'Nhẹ']);
    fireEvent.click(screen.getByRole('button', { name: /^CPU/ }));
    expect(screen.getAllByText(/^(Nhẹ|Nặng)$/).map((e) => e.textContent)).toEqual(['Nhẹ', 'Nặng']);
  });

  it('tiến trình thiết yếu khóa nút Kết thúc; app thường kết thúc sau khi xác nhận', async () => {
    const { api, send } = setup();
    await vi.waitFor(() => expect(api.perfStart).toHaveBeenCalled());
    send(tick(T0));
    const svc = within(appRow('Service Host')).getByRole('button', { name: 'Kết thúc app', hidden: true }) as HTMLButtonElement;
    expect(svc.disabled).toBe(true);
    expect(svc.title).toBe('Tiến trình thiết yếu của Windows');
    fireEvent.click(within(appRow('Google Chrome')).getByRole('button', { name: 'Kết thúc app', hidden: true }));
    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByText('Kết thúc «Google Chrome»?')).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Kết thúc', hidden: true }));
    expect(await screen.findByText('Đã kết thúc «Google Chrome».')).toBeTruthy();
    expect(api.appKill).toHaveBeenCalledWith('chrome.exe');
  });

  it('đang kết thúc thì khóa nút khác; lõi từ chối thì băng đỏ câu dễ hiểu', async () => {
    const kill = deferred<never>();
    const { send, notify } = setup({ appKill: vi.fn(() => kill.promise) });
    await vi.waitFor(() => expect(screen.getByText('Đang bắt đầu đo…')).toBeTruthy());
    send(tick(T0, { apps: [app('a.exe', 'Một', 2 * GB), app('b.exe', 'Hai', GB)] }));
    fireEvent.click(within(appRow('Một')).getByRole('button', { name: 'Kết thúc app', hidden: true }));
    fireEvent.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Kết thúc', hidden: true }));
    expect(await screen.findByText('Đang kết thúc…')).toBeTruthy();
    expect((within(appRow('Hai')).getByRole('button', { name: 'Kết thúc app', hidden: true }) as HTMLButtonElement).disabled).toBe(true);
    await act(async () => kill.reject('essential'));
    expect(notify).toHaveBeenCalledWith('error', 'Không kết thúc được «Một»: Đây là tiến trình thiết yếu của Windows — không kết thúc được.');
  });

  it('mở ra từng tiến trình và mở vị trí file', async () => {
    const { api, send } = setup();
    await vi.waitFor(() => expect(api.perfStart).toHaveBeenCalled());
    send(tick(T0));
    fireEvent.click(screen.getByRole('button', { name: 'Xem các tiến trình của Google Chrome' }));
    expect(screen.getByText('chrome.exe · PID 100')).toBeTruthy();
    fireEvent.click(within(appRow('Google Chrome')).getByRole('button', { name: 'Mở vị trí file', hidden: true }));
    expect(api.revealPath).toHaveBeenCalledWith('C:\\Apps\\chrome.exe');
  });

  it('cơn giật: giờ, thời lượng, chỉ số, 3 app ngốn nhất, cờ hạ xung; dải đỏ trên cả bốn biểu đồ', async () => {
    const { send, view } = setup();
    await vi.waitFor(() => expect(screen.getByText('Đang bắt đầu đo…')).toBeTruthy());
    send(tick(T0 + 60_000));
    expect(screen.getByText('Chưa ghi nhận cơn giật nào trong 5 phút qua.')).toBeTruthy();
    send(
      tick(T0 + 61_000, {
        stutters: [
          {
            start_ms: T0,
            end_ms: T0 + 5000,
            cpu: true,
            disk: false,
            top_cpu: [
              { key: 'x', name: 'Game', avg: 71.4 },
              { key: 'y', name: 'Defender', avg: 20 },
            ],
            top_disk: [],
            throttled: true,
          },
        ],
      }),
    );
    expect(screen.getByText('14:03:12 · 5 giây · CPU')).toBeTruthy();
    expect(screen.getByText('Ngốn nhất: Game 71%, Defender 20%')).toBeTruthy();
    expect(screen.getByText('Trùng lúc hạ xung')).toBeTruthy();
    expect(view.container.querySelectorAll('[data-band]')).toHaveLength(4);
  });

  it('hạ xung, nhiệt độ và các nguồn không đo được', async () => {
    const { send, notify } = setup();
    await vi.waitFor(() => expect(screen.getByText('Đang bắt đầu đo…')).toBeTruthy());
    send(tick(T0, { throttled: true }));
    expect(screen.getByText(/CPU đang bị hạ xung/)).toBeTruthy();
    expect(screen.getByText('Nhiệt độ CPU: 55 °C')).toBeTruthy();
    const degraded = (t: number) =>
      tick(t, {
        throttled: null,
        perf_error: 'PDH \\Processor Information(_Total)\\% Processor Performance: 0xC0000BB8',
        temp: { state: 'unavailable', code: 'stuck', detail: '27.8' },
        net_app_error: 'ETW WinFreeUp-Net: Access is denied.',
        apps: [app('a.exe', 'Một', GB, { net_up_bps: null, net_down_bps: null })],
      });
    send(degraded(T0 + 1000));
    send(degraded(T0 + 2000));
    expect(screen.getByText('Máy này không cho đọc nhiệt độ')).toBeTruthy();
    expect(screen.getByText(/^Không đo được hạ xung: PDH/)).toBeTruthy();
    expect(screen.getByText('–')).toBeTruthy();
    expect(notify.mock.calls.filter((c) => c[1].startsWith('Không theo dõi được mạng'))).toHaveLength(1);
  });

  it('bắt đầu đo hỏng thì băng đỏ; lỗi từng mẫu thì hổ phách', async () => {
    const { notify } = setup({ perfStart: vi.fn(async () => Promise.reject('PDH open: 0xC0000BB8')) });
    await vi.waitFor(() => expect(notify).toHaveBeenCalledWith('error', 'Không bắt đầu đo được: PDH open: 0xC0000BB8'));
    const s = setup();
    await vi.waitFor(() => expect(s.api.perfStart).toHaveBeenCalled());
    s.fail('NtQuerySystemInformation failed: 0xC0000017');
    expect(s.notify).toHaveBeenCalledWith('warning', 'Lỗi khi lấy mẫu: NtQuerySystemInformation failed: 0xC0000017');
  });

  it('giãn chu kỳ thì báo cho người dùng biết', async () => {
    const { send } = setup();
    await vi.waitFor(() => expect(screen.getByText('Đang bắt đầu đo…')).toBeTruthy());
    send(tick(T0, { interval_ms: 2000 }));
    expect(screen.getByText('Đang lấy mẫu mỗi 2 giây để WinFreeUp không làm máy nặng thêm.')).toBeTruthy();
  });
});
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `npx vitest run src/features/kham-may/perf`
Expected: FAIL — `Failed to resolve import "./perfModel"`, `"./LineChart"`, `"./PerfView"`.

- [ ] **Step 3: Viết mã**

`src/features/kham-may/perf/perfModel.ts`:

```ts
import type { AppRow, Sample } from '../api/types';

/** Spec 5: giữ 5 phút gần nhất. */
export const WINDOW_MS = 5 * 60 * 1000;

/** Thêm mẫu mới, bỏ mẫu trùng giờ và mẫu cũ hơn 5 phút so với mẫu mới nhất. */
export function pushSample(list: Sample[], s: Sample): Sample[] {
  const next = list.filter((x) => x.t_ms !== s.t_ms && x.t_ms >= s.t_ms - WINDOW_MS);
  next.push(s);
  next.sort((a, b) => a.t_ms - b.t_ms);
  return next;
}

export type AppSortKey = 'name' | 'ram' | 'cpu' | 'disk' | 'net';

/** Tổng mạng lên + xuống; `null` khi không theo dõi được mạng theo app. */
export function netTotal(a: { net_up_bps: number | null; net_down_bps: number | null }): number | null {
  if (a.net_up_bps === null || a.net_down_bps === null) return null;
  return a.net_up_bps + a.net_down_bps;
}

function metric(a: AppRow, key: AppSortKey): number {
  switch (key) {
    case 'ram':
      return a.ram;
    case 'cpu':
      return a.cpu;
    case 'disk':
      return a.disk_bps;
    case 'net':
      return netTotal(a) ?? -1;
    default:
      return 0;
  }
}

/** Sắp theo mọi cột (spec 5.2). Mặc định RAM giảm dần. */
export function sortApps(apps: AppRow[], key: AppSortKey, desc: boolean): AppRow[] {
  const dir = desc ? -1 : 1;
  return [...apps].sort((a, b) => {
    const c = key === 'name' ? a.name.localeCompare(b.name, 'vi') : metric(a, key) - metric(b, key);
    return c !== 0 ? c * dir : a.key.localeCompare(b.key);
  });
}

export interface Pt {
  t: number;
  v: number;
}

/**
 * Chuỗi điểm cho một đường, tách thành nhiều đoạn ở chỗ thiếu số liệu hoặc có khoảng dừng lấy mẫu
 * (rời mục rồi quay lại) — không nối thẳng qua chỗ trống, kẻo người xem tưởng có số đo.
 */
export function segments(samples: Sample[], pick: (s: Sample) => number | null): Pt[][] {
  const out: Pt[][] = [];
  let cur: Pt[] = [];
  let prev: Sample | null = null;
  for (const s of samples) {
    const v = pick(s);
    const gap = prev !== null && s.t_ms - prev.t_ms > 3 * Math.max(s.dur_ms, 1000);
    if (v === null || gap) {
      if (cur.length) out.push(cur);
      cur = [];
    }
    if (v !== null) cur.push({ t: s.t_ms, v });
    prev = s;
  }
  if (cur.length) out.push(cur);
  return out;
}

/** Trục dọc tự co cho biểu đồ mạng: bội số đẹp ≥ giá trị lớn nhất, tối thiểu 64 KB/s. */
export function niceMax(values: number[]): number {
  const m = Math.max(64 * 1024, ...values);
  const p = 2 ** Math.ceil(Math.log2(m));
  return p;
}
```

`src/features/kham-may/perf/LineChart.tsx`:

```tsx
import type { Pt } from './perfModel';
import { WINDOW_MS } from './perfModel';

export interface Band {
  start_ms: number;
  end_ms: number;
}

/**
 * Biểu đồ đường SVG gọn (không thêm thư viện): trục ngang là 5 phút gần nhất tính tới `now`,
 * trục dọc 0..`max`. `bands` là các cơn giật — tô dải đỏ.
 */
export function LineChart({
  title,
  valueText,
  lines,
  max,
  now,
  bands,
}: {
  title: string;
  valueText: string;
  lines: { segments: Pt[][]; alt?: boolean; label?: string }[];
  max: number;
  now: number;
  bands: Band[];
}) {
  const W = 300;
  const H = 100;
  const start = now - WINDOW_MS;
  const x = (t: number) => Math.min(W, Math.max(0, ((t - start) / WINDOW_MS) * W));
  const y = (v: number) => H - Math.min(H, Math.max(0, (v / (max || 1)) * H));
  return (
    <figure className="km-chart" style={{ margin: 0 }}>
      <figcaption className="km-chart-head">
        <strong>{title}</strong>
        <span className="km-num">{valueText}</span>
      </figcaption>
      <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" role="img" aria-label={`${title}: ${valueText}`}>
        {bands
          .filter((b) => b.end_ms > start)
          .map((b) => (
            <rect
              key={`${b.start_ms}-${b.end_ms}`}
              className="km-chart-band"
              data-band=""
              x={x(b.start_ms)}
              y={0}
              width={Math.max(1, x(b.end_ms) - x(b.start_ms))}
              height={H}
            />
          ))}
        {lines.flatMap((l, li) =>
          l.segments.map((seg, si) => (
            <polyline
              key={`${li}-${si}`}
              className={l.alt ? 'km-chart-line km-alt' : 'km-chart-line'}
              points={seg.map((p) => `${x(p.t).toFixed(1)},${y(p.v).toFixed(1)}`).join(' ')}
            >
              {l.label && <title>{l.label}</title>}
            </polyline>
          )),
        )}
      </svg>
    </figure>
  );
}
```

`src/features/kham-may/perf/AppTable.tsx`:

```tsx
import { Fragment, useState } from 'react';
import { Button } from '@fluentui/react-components';
import type { AppRow } from '../api/types';
import { formatBytes, formatKBps, formatMBps, formatPct } from '../fmt';
import { tk } from '../i18n';
import { netTotal, sortApps, type AppSortKey } from './perfModel';

const COLUMNS: { key: AppSortKey; label: string; numeric: boolean }[] = [
  { key: 'name', label: 'km.perf.col.name', numeric: false },
  { key: 'ram', label: 'km.perf.col.ram', numeric: true },
  { key: 'cpu', label: 'km.perf.col.cpu', numeric: true },
  { key: 'disk', label: 'km.perf.col.disk', numeric: true },
  { key: 'net', label: 'km.perf.col.net', numeric: true },
];

function net(v: { net_up_bps: number | null; net_down_bps: number | null }): string {
  const n = netTotal(v);
  return n === null ? '–' : formatKBps(n);
}

export function AppTable({
  apps,
  icons,
  busy,
  onKill,
  onReveal,
}: {
  apps: AppRow[];
  icons: Record<string, string | null>;
  /** Đang kết thúc một app ⇒ khóa mọi nút Kết thúc khác. */
  busy: boolean;
  onKill: (a: AppRow) => void;
  onReveal: (path: string) => void;
}) {
  const [sort, setSort] = useState<{ key: AppSortKey; desc: boolean }>({ key: 'ram', desc: true });
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const rows = sortApps(apps, sort.key, sort.desc);

  function by(key: AppSortKey) {
    setSort((s) => (s.key === key ? { key, desc: !s.desc } : { key, desc: key !== 'name' }));
  }

  return (
    <table className="km-apps">
      <thead>
        <tr>
          {COLUMNS.map((c) => (
            <th
              key={c.key}
              className={c.numeric ? 'km-num' : undefined}
              aria-sort={sort.key === c.key ? (sort.desc ? 'descending' : 'ascending') : 'none'}
            >
              <Button appearance="transparent" size="small" onClick={() => by(c.key)}>
                {tk(c.label)}
                {sort.key === c.key ? (sort.desc ? ' ▼' : ' ▲') : ''}
              </Button>
            </th>
          ))}
          <th />
        </tr>
      </thead>
      <tbody>
        {rows.map((a) => (
          <Fragment key={a.key}>
            <tr>
              <td>
                <span className="km-tree-name">
                  <Button
                    size="small"
                    appearance="transparent"
                    aria-label={tk(open[a.key] ? 'km.perf.collapse' : 'km.perf.expand', { name: a.name })}
                    onClick={() => setOpen((o) => ({ ...o, [a.key]: !o[a.key] }))}
                  >
                    {open[a.key] ? '▾' : '▸'}
                  </Button>
                  {a.path && icons[a.path] ? <img className="km-app-icon" src={icons[a.path] ?? undefined} alt="" /> : <span className="km-app-icon" />}
                  <span className="km-tree-label" title={a.path ?? a.name}>
                    {a.name}
                  </span>
                  {a.procs.length > 1 && <span className="km-muted">({a.procs.length})</span>}
                </span>
              </td>
              <td className="km-num">{formatBytes(a.ram)}</td>
              <td className="km-num">{formatPct(a.cpu)}</td>
              <td className="km-num">{formatMBps(a.disk_bps)}</td>
              <td className="km-num">{net(a)}</td>
              <td>
                <span className="km-row-actions">
                  <Button
                    size="small"
                    appearance="subtle"
                    disabled={a.essential || busy}
                    title={a.essential ? tk('km.perf.essential') : undefined}
                    onClick={() => onKill(a)}
                  >
                    {tk('km.perf.kill')}
                  </Button>
                  {a.path && (
                    <Button size="small" appearance="subtle" onClick={() => onReveal(a.path as string)}>
                      {tk('km.perf.reveal')}
                    </Button>
                  )}
                </span>
              </td>
            </tr>
            {open[a.key] &&
              a.procs.map((p) => (
                <tr key={`${a.key}-${p.pid}-${p.create_time}`} className="km-proc">
                  <td className="km-muted">
                    {p.name} · PID {p.pid}
                    {p.essential ? ' 🔒' : ''}
                  </td>
                  <td className="km-num km-muted">{formatBytes(p.ram)}</td>
                  <td className="km-num km-muted">{formatPct(p.cpu)}</td>
                  <td className="km-num km-muted">{formatMBps(p.disk_bps)}</td>
                  <td className="km-num km-muted">{net(p)}</td>
                  <td />
                </tr>
              ))}
          </Fragment>
        ))}
      </tbody>
    </table>
  );
}
```

`src/features/kham-may/perf/StutterList.tsx`:

```tsx
import { Badge } from '@fluentui/react-components';
import type { StutterView, TopApp } from '../api/types';
import { formatClock, formatMBps, formatPct } from '../fmt';
import { tk } from '../i18n';

function top(apps: TopApp[], cpu: boolean): string {
  return apps.map((a) => `${a.name} ${cpu ? formatPct(a.avg) : `${formatMBps(a.avg)} MB/s`}`).join(', ');
}

/** Spec 5.3: mỗi cơn — giờ bắt đầu, thời lượng, chỉ số vượt, 3 app ngốn nhất, cờ trùng lúc hạ xung. Mới nhất lên đầu. */
export function StutterList({ stutters }: { stutters: StutterView[] }) {
  if (stutters.length === 0) return <p className="km-muted">{tk('km.perf.noStutters')}</p>;
  return (
    <ul className="km-stutters">
      {[...stutters].reverse().map((s) => {
        const metrics = [s.cpu && tk('km.perf.stutterCpu'), s.disk && tk('km.perf.stutterDisk')].filter(Boolean).join(' + ');
        return (
          <li key={s.start_ms}>
            <strong>
              {tk('km.perf.stutterLine', { time: formatClock(s.start_ms), seconds: Math.round((s.end_ms - s.start_ms) / 1000), metrics })}
            </strong>{' '}
            {s.throttled && (
              <Badge appearance="tint" color="warning">
                {tk('km.perf.stutterThrottled')}
              </Badge>
            )}
            {s.top_cpu.length > 0 && <div className="km-muted">{tk('km.perf.stutterTop', { apps: top(s.top_cpu, true) })}</div>}
            {s.top_disk.length > 0 && <div className="km-muted">{tk('km.perf.stutterTop', { apps: top(s.top_disk, false) })}</div>}
          </li>
        );
      })}
    </ul>
  );
}
```

`src/features/kham-may/perf/PerfView.tsx`:

```tsx
import { useEffect, useRef, useState } from 'react';
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle } from '@fluentui/react-components';
import type { AppRow, KhamMayApi, Notify, PerfTick, Sample } from '../api/types';
import { formatBytes, formatPct, formatRate, friendly } from '../fmt';
import { tk } from '../i18n';
import { Spin } from '../ui/Spin';
import { AppTable } from './AppTable';
import { LineChart } from './LineChart';
import { niceMax, pushSample, segments } from './perfModel';
import { StutterList } from './StutterList';

/** Mục Bộ nhớ & Hiệu năng. Chỉ lấy mẫu khi đang hiển thị: gắn vào ⇒ `perfStart`, gỡ ra ⇒ `perfStop` (spec 5). */
export function PerfView({ api, notify }: { api: KhamMayApi; notify: Notify }) {
  const [samples, setSamples] = useState<Sample[]>([]);
  const [last, setLast] = useState<PerfTick | null>(null);
  const [icons, setIcons] = useState<Record<string, string | null>>({});
  const [confirm, setConfirm] = useState<AppRow | null>(null);
  const [killing, setKilling] = useState(false);
  const [status, setStatus] = useState('');
  const warned = useRef<Set<string>>(new Set());
  const requested = useRef<Set<string>>(new Set());
  const alive = useRef(true);

  function warnOnce(key: string, message: string) {
    if (warned.current.has(key)) return;
    warned.current.add(key);
    notify('warning', message);
  }

  useEffect(() => {
    alive.current = true;
    const onTick = (t: PerfTick) => {
      if (!alive.current) return;
      setLast(t);
      setSamples((s) => pushSample(s, t.sample));
      if (t.net_app_error) warnOnce('net', tk('km.perf.netAppMissing', { message: t.net_app_error }));
      if (t.disk_error) warnOnce('disk', tk('km.perf.diskMissing', { message: t.disk_error }));
    };
    const onError = (m: string) => notify('warning', tk('km.perf.sampleFailed', { message: m }));
    api
      .perfStart(onTick, onError)
      .then((history) => alive.current && setSamples((s) => history.reduce(pushSample, s)))
      .catch((e) => notify('error', tk('km.perf.startFailed', { message: friendly(e) })));
    return () => {
      alive.current = false;
      api.perfStop().catch((e) => notify('warning', friendly(e)));
    };
  }, [api, notify]);

  // Biểu tượng app: xin một lần cho mỗi đường dẫn, lõi nhớ đệm.
  useEffect(() => {
    for (const a of last?.apps ?? []) {
      const p = a.path;
      if (!p || requested.current.has(p)) continue;
      requested.current.add(p);
      api
        .appIcon(p)
        .then((url) => alive.current && setIcons((m) => ({ ...m, [p]: url })))
        .catch(() => alive.current && setIcons((m) => ({ ...m, [p]: null })));
    }
  }, [api, last]);

  async function kill() {
    const a = confirm;
    if (!a) return;
    setKilling(true);
    try {
      const r = await api.appKill(a.key);
      if (r.errors.length > 0) notify('warning', tk('km.perf.killPartial', { name: a.name, count: r.errors.length, first: r.errors[0] }));
      else setStatus(tk('km.perf.killed', { name: a.name }));
      if (r.log_error) notify('warning', tk('km.err.logWrite', { message: r.log_error }));
    } catch (e) {
      notify('error', tk('km.perf.killFailed', { name: a.name, message: friendly(e) }));
    } finally {
      setKilling(false);
      setConfirm(null);
    }
  }

  async function reveal(path: string) {
    try {
      await api.revealPath(path);
    } catch (e) {
      notify('warning', tk('km.perf.revealFailed', { message: friendly(e) }));
    }
  }

  if (!last) return <Spin label={tk('km.perf.starting')} />;

  const s = last.sample;
  const now = s.t_ms;
  const bands = last.stutters;
  const netMax = niceMax(samples.flatMap((x) => [x.net_up_bps ?? 0, x.net_down_bps ?? 0]));

  return (
    <section className="km-root" aria-label={tk('km.tab.perf')}>
      <div className="km-charts">
        <LineChart title={tk('km.perf.chart.cpu')} valueText={formatPct(s.cpu)} max={100} now={now} bands={bands} lines={[{ segments: segments(samples, (x) => x.cpu) }]} />
        <LineChart
          title={tk('km.perf.chart.ram')}
          valueText={tk('km.perf.ramValue', { pct: Math.round(s.ram_pct), used: formatBytes(s.ram_used), total: formatBytes(s.ram_total) })}
          max={100}
          now={now}
          bands={bands}
          lines={[{ segments: segments(samples, (x) => x.ram_pct) }]}
        />
        <LineChart
          title={tk('km.perf.chart.disk')}
          valueText={s.disk_active === null ? tk('km.perf.noValue') : formatPct(s.disk_active)}
          max={100}
          now={now}
          bands={bands}
          lines={[{ segments: segments(samples, (x) => x.disk_active) }]}
        />
        <LineChart
          title={tk('km.perf.chart.net')}
          valueText={
            s.net_up_bps === null || s.net_down_bps === null
              ? tk('km.perf.noValue')
              : `↑ ${formatRate(s.net_up_bps)} · ↓ ${formatRate(s.net_down_bps)}`
          }
          max={netMax}
          now={now}
          bands={bands}
          lines={[
            { segments: segments(samples, (x) => x.net_down_bps), label: tk('km.perf.netDown') },
            { segments: segments(samples, (x) => x.net_up_bps), alt: true, label: tk('km.perf.netUp') },
          ]}
        />
      </div>

      <div className="km-toolbar">
        {last.throttled === true && <strong>🟠 {tk('km.perf.throttled')}</strong>}
        {last.throttled === false && <span className="km-muted">{tk('km.perf.notThrottled')}</span>}
        {last.throttled === null && last.perf_error && <span className="km-muted">{tk('km.perf.throttleMissing', { message: last.perf_error })}</span>}
        <span className={last.temp.state === 'ok' ? undefined : 'km-muted'} title={last.temp.state === 'ok' ? undefined : last.temp.detail}>
          {last.temp.state === 'ok'
            ? tk('km.perf.temp', { celsius: Math.round(last.temp.celsius) })
            : tk('km.perf.tempMissing')}
        </span>
        {last.interval_ms > 1000 && <span className="km-muted">{tk('km.perf.slow')}</span>}
      </div>

      <h3>{tk('km.perf.stutters')}</h3>
      <StutterList stutters={last.stutters} />

      <h3>{tk('km.perf.apps')}</h3>
      <p className="km-muted" aria-live="polite">
        {status}
      </p>
      <AppTable apps={last.apps} icons={icons} busy={killing} onKill={setConfirm} onReveal={(p) => void reveal(p)} />

      <Dialog open={confirm !== null} onOpenChange={(_, d) => !d.open && !killing && setConfirm(null)}>
        <DialogSurface>
          <DialogBody>
            <DialogTitle>{confirm && tk('km.perf.killTitle', { name: confirm.name })}</DialogTitle>
            <DialogContent>{tk('km.perf.killBody')}</DialogContent>
            <DialogActions>
              {killing ? (
                <Spin label={tk('km.perf.killing')} />
              ) : (
                <>
                  <Button appearance="primary" onClick={() => void kill()}>
                    {tk('km.perf.killConfirm')}
                  </Button>
                  <Button onClick={() => setConfirm(null)}>{tk('km.perf.killCancel')}</Button>
                </>
              )}
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </section>
  );
}
```

- [ ] **Step 4: Chạy test, thấy qua**

Run: `npx vitest run src/features/kham-may/perf` rồi `npm run typecheck`
Expected: `Tests  16 passed`; typecheck sạch.

- [ ] **Step 5: Commit**

```bash
rtk git add src/features/kham-may/perf/perfModel.ts
rtk git add src/features/kham-may/perf/perfModel.test.tsx
rtk git add src/features/kham-may/perf/LineChart.tsx
rtk git add src/features/kham-may/perf/AppTable.tsx
rtk git add src/features/kham-may/perf/StutterList.tsx
rtk git add src/features/kham-may/perf/PerfView.tsx
rtk git add src/features/kham-may/perf/PerfView.test.tsx
rtk git commit -m "feat(kham-may): mục Bộ nhớ & Hiệu năng — 4 biểu đồ SVG có dải cơn giật, bảng app, kết thúc app

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 13: `WalkScanner` — duyệt thư mục song song

**Files:**
- Modify (thay dòng khung): `crates/winfreeup-diagnose/src/disk/scan.rs`, `disk/walk.rs`
- Test: hai file trên

**Interfaces:**
- Consumes: Task 3 (`TreeBuilder`, `DiskTree`, `NodeId`, `FLAG_DIR`, `FLAG_LINK`, `FLAG_UNREADABLE`, `NO_PARENT`, `MAX_CHILDREN`, `DiskTree::{child_named, children, view, bytes, files, root}`); Task 1 (`util::{wide, filetime_to_unix}`); `winfreeup_core::{CancelToken, CoreError, Result, error::io_err}`.
- Produces:
  - `scan::ScanStatus { files: u64, bytes: u64, current: String, percent: Option<f32> }` (Serialize — payload `disk-scan-progress`), trait `ScanProgress` (mọi `Fn(&ScanStatus) + Sync` tự cài), trait `DiskScanner { fn scan(&self, root: &Path, cancel: &CancelToken, progress: &dyn ScanProgress) -> Result<DiskTree> }`, `Throttle::new(every)`, `ready(&self) -> bool`.
  - `walk::WalkScanner { threads }` (+ `Default`: gấp đôi số luồng CPU, 8–32), `read_dir_entries(dir: &Path) -> io::Result<Vec<DirEntry>>`, `DirEntry { name, is_dir, is_link, alloc, modified, file_id }`, `is_link_tag(attributes: u32, reparse_tag: u32) -> bool` (bit name-surrogate).

- [ ] **Step 1: Viết test hỏng** — thay dòng khung của hai file bằng phần test:

`scan.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throttle_lets_the_first_call_through_then_waits() {
        let t = Throttle::new(Duration::from_secs(60));
        assert!(t.ready());
        assert!(!t.ready());
        let t0 = Throttle::new(Duration::ZERO);
        assert!(t0.ready() && t0.ready());
    }
}
```

`walk.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::tree::MAX_CHILDREN;
    use std::fs;
    use std::os::windows::io::AsRawHandle;
    use std::process::Command;

    fn write(path: &Path, len: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![b'x'; len]).unwrap();
    }

    fn scan(root: &Path) -> DiskTree {
        WalkScanner { threads: 4 }.scan(root, &CancelToken::new(), &|_: &ScanStatus| {}).unwrap()
    }

    #[test]
    fn allocated_size_is_counted_not_logical_size() {
        let t = tempfile::tempdir().unwrap();
        write(&t.path().join("a").join("one.bin"), 1);
        write(&t.path().join("a").join("big.bin"), 100_000);
        let tree = scan(t.path());
        let a = tree.child_named(tree.root(), "a").unwrap();
        let one = tree.child_named(a, "one.bin").unwrap();
        // 1 byte thường nằm ngay trong MFT (0 byte cấp phát) hoặc chiếm trọn một cụm — không bao giờ đúng 1.
        assert_ne!(tree.bytes(one), 1);
        let big = tree.child_named(a, "big.bin").unwrap();
        assert!(tree.bytes(big) >= 100_000 && tree.bytes(big).is_multiple_of(512), "{}", tree.bytes(big));
        assert_eq!(tree.files(tree.root()), 2);
    }

    #[test]
    fn hard_link_is_counted_once() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("x").join("data.bin");
        write(&f, 200_000);
        fs::create_dir_all(t.path().join("y")).unwrap();
        fs::hard_link(&f, t.path().join("y").join("same.bin")).unwrap();
        let tree = scan(t.path());
        let one = tree.bytes(tree.child_named(tree.root(), "x").unwrap()) + tree.bytes(tree.child_named(tree.root(), "y").unwrap());
        assert!((200_000..400_000).contains(&one), "{one}");
        assert_eq!(tree.files(tree.root()), 1);
    }

    #[test]
    fn junction_pointing_outside_is_not_followed() {
        let t = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        write(&outside.path().join("huge.bin"), 500_000);
        fs::create_dir_all(t.path().join("in")).unwrap();
        junction::create(outside.path(), t.path().join("in").join("link")).unwrap();
        let tree = scan(t.path());
        assert_eq!(tree.bytes(tree.root()), 0);
        let inn = tree.child_named(tree.root(), "in").unwrap();
        let link = tree.children(inn, MAX_CHILDREN).unwrap().items[0].clone();
        assert!(link.is_link && !link.has_children);
    }

    #[test]
    fn unreadable_directory_stays_a_node_and_is_flagged() {
        let t = tempfile::tempdir().unwrap();
        let locked = t.path().join("khoa");
        write(&locked.join("secret.bin"), 50_000);
        let deny = Command::new("icacls").arg(&locked).args(["/deny", "*S-1-1-0:(RD)"]).output().unwrap();
        assert!(deny.status.success(), "{}", String::from_utf8_lossy(&deny.stdout));
        let tree = scan(t.path());
        let _ = Command::new("icacls").arg(&locked).args(["/remove:d", "*S-1-1-0"]).output();
        let node = tree.view(tree.child_named(tree.root(), "khoa").unwrap());
        assert!(node.unreadable);
        assert_eq!(node.bytes, 0);
    }

    #[test]
    fn sparse_file_counts_only_allocated_clusters() {
        use windows_sys::Win32::System::Ioctl::FSCTL_SET_SPARSE;
        use windows_sys::Win32::System::IO::DeviceIoControl;
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("sparse.bin");
        {
            let f = fs::File::create(&path).unwrap();
            let mut ret = 0u32;
            let ok = unsafe {
                DeviceIoControl(f.as_raw_handle(), FSCTL_SET_SPARSE, std::ptr::null(), 0, std::ptr::null_mut(), 0, &mut ret, std::ptr::null_mut())
            };
            assert_ne!(ok, 0, "{}", std::io::Error::last_os_error());
            f.set_len(64 * 1024 * 1024).unwrap();
        }
        let tree = scan(t.path());
        assert_eq!(fs::metadata(&path).unwrap().len(), 64 * 1024 * 1024);
        assert!(tree.bytes(tree.root()) < 1024 * 1024, "{}", tree.bytes(tree.root()));
    }

    #[test]
    fn long_and_vietnamese_names_are_read() {
        let t = tempfile::tempdir().unwrap();
        let mut deep = t.path().to_path_buf();
        for i in 0..12 {
            deep.push(format!("thư-mục-khá-dài-số-{i:02}-để-vượt-260-ký-tự"));
        }
        let long = long_path(&deep.join("tệp.bin"));
        fs::create_dir_all(long_path(&deep)).unwrap();
        fs::write(&long, vec![0u8; 10_000]).unwrap();
        assert!(deep.to_string_lossy().len() > 260);
        let tree = scan(t.path());
        assert_eq!(tree.files(tree.root()), 1);
    }

    #[test]
    fn cancel_stops_the_scan() {
        let t = tempfile::tempdir().unwrap();
        write(&t.path().join("a.bin"), 10);
        let c = CancelToken::new();
        c.cancel();
        let r = WalkScanner { threads: 2 }.scan(t.path(), &c, &|_: &ScanStatus| {});
        assert!(matches!(r, Err(CoreError::Cancelled)));
    }

    #[test]
    fn progress_is_reported_at_least_once_with_totals() {
        let t = tempfile::tempdir().unwrap();
        write(&t.path().join("a.bin"), 10_000);
        let seen = Mutex::new(Vec::new());
        WalkScanner { threads: 2 }.scan(t.path(), &CancelToken::new(), &|s: &ScanStatus| seen.lock().unwrap().push(s.clone())).unwrap();
        let last = seen.into_inner().unwrap().pop().unwrap();
        assert_eq!(last.files, 1);
        assert!(last.bytes >= 10_000);
    }

    #[test]
    fn file_id_zero_is_never_treated_as_a_hard_link() {
        // FAT32/exFAT/ổ mạng có thể trả FileId = 0 cho mọi file: gộp theo mã thì cả ổ chỉ còn một file.
        let seen = SeenIds::new();
        assert!(seen.first(0) && seen.first(0));
        assert!(seen.first(42));
        assert!(!seen.first(42));
    }

    #[test]
    fn link_tags_are_recognised() {
        assert!(is_link_tag(FILE_ATTRIBUTE_REPARSE_POINT, 0xA000_0003), "junction");
        assert!(is_link_tag(FILE_ATTRIBUTE_REPARSE_POINT, 0xA000_000C), "symlink");
        assert!(!is_link_tag(FILE_ATTRIBUTE_REPARSE_POINT, 0x9000_601A), "thư mục OneDrive");
        assert!(!is_link_tag(0, 0xA000_0003));
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test disk::`
Expected: FAIL biên dịch — `cannot find type Throttle`, `cannot find type WalkScanner`.

- [ ] **Step 3: Viết mã (trên khối test)**

`scan.rs`:

```rust
//! Khuôn chung của bộ quét ổ đĩa và tiến độ.
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use winfreeup_core::{CancelToken, Result};

use super::tree::DiskTree;

/// Payload sự kiện `disk-scan-progress`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScanStatus {
    pub files: u64,
    pub bytes: u64,
    pub current: String,
    /// MFT biết trước tổng số bản ghi nên có %; duyệt thư mục thì không (`None`).
    pub percent: Option<f32>,
}

pub trait ScanProgress: Sync {
    fn report(&self, status: &ScanStatus);
}

impl<F: Fn(&ScanStatus) + Sync> ScanProgress for F {
    fn report(&self, status: &ScanStatus) {
        self(status)
    }
}

/// Spec 3.1 `DiskScanner`. `root` là gốc ổ dạng `C:\`.
pub trait DiskScanner: Sync {
    fn scan(&self, root: &Path, cancel: &CancelToken, progress: &dyn ScanProgress) -> Result<DiskTree>;
}

/// Giãn nhịp báo tiến độ: tối đa một lần mỗi `every`, để giao diện không bị ngập sự kiện.
pub struct Throttle {
    every: Duration,
    last: Mutex<Option<Instant>>,
}

impl Throttle {
    pub fn new(every: Duration) -> Self {
        Throttle { every, last: Mutex::new(None) }
    }

    pub fn ready(&self) -> bool {
        let Ok(mut last) = self.last.lock() else { return false };
        let now = Instant::now();
        match *last {
            Some(t) if now.duration_since(t) < self.every => false,
            _ => {
                *last = Some(now);
                true
            }
        }
    }
}
```

`walk.rs`:

```rust
//! WalkScanner: duyệt song song mọi ổ (FAT32, exFAT, USB, mạng) — đường chậm nhưng chạy ở đâu cũng được.
//! Mỗi thư mục đọc bằng `GetFileInformationByHandleEx(FileIdBothDirectoryInfo)`: một lời gọi trả cả
//! dung lượng thực chiếm (AllocationSize), mã file (FileId — để hard link chỉ đếm một lần) và reparse tag.
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FileIdBothDirectoryInfo, GetFileInformationByHandleEx, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_ID_BOTH_DIR_INFO, FILE_LIST_DIRECTORY,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use winfreeup_core::{CancelToken, CoreError, Result};

use super::scan::{DiskScanner, ScanProgress, ScanStatus, Throttle};
use super::tree::{DiskTree, NodeId, TreeBuilder, FLAG_DIR, FLAG_LINK, FLAG_UNREADABLE, NO_PARENT};
use crate::util::{filetime_to_unix, wide};

/// Bit «name surrogate» của reparse tag: junction, symlink… trỏ sang chỗ khác ⇒ không đi theo.
/// Thư mục OneDrive (tag cloud) không có bit này ⇒ vẫn là thư mục thật, vẫn duyệt vào.
const NAME_SURROGATE: u32 = 0x2000_0000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    /// Junction/symlink (reparse point có bit name surrogate).
    pub is_link: bool,
    pub alloc: u64,
    pub modified: u32,
    pub file_id: u64,
}

pub fn is_link_tag(attributes: u32, reparse_tag: u32) -> bool {
    attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 && reparse_tag & NAME_SURROGATE != 0
}

/// `C:\a` ⇒ `\\?\C:\a` để vượt giới hạn 260 ký tự.
fn long_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with(r"\\?\") {
        path.to_path_buf()
    } else if let Some(unc) = s.strip_prefix(r"\\") {
        PathBuf::from(format!(r"\\?\UNC\{unc}"))
    } else {
        PathBuf::from(format!(r"\\?\{s}"))
    }
}

/// Đọc toàn bộ mục trong một thư mục (bỏ `.` và `..`).
pub fn read_dir_entries(dir: &Path) -> std::io::Result<Vec<DirEntry>> {
    let w = wide(long_path(dir));
    let h = unsafe {
        CreateFileW(
            w.as_ptr(),
            FILE_LIST_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    if h == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
    }
    let mut out = Vec::new();
    // u64 để bộ đệm căn 8 byte như cấu trúc yêu cầu.
    let mut buf = vec![0u64; 64 * 1024 / 8];
    let result = loop {
        let ok = unsafe {
            GetFileInformationByHandleEx(h, FileIdBothDirectoryInfo, buf.as_mut_ptr().cast(), (buf.len() * 8) as u32)
        };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            break if err == ERROR_NO_MORE_FILES { Ok(()) } else { Err(std::io::Error::from_raw_os_error(err as i32)) };
        }
        let base = buf.as_ptr() as *const u8;
        let mut off = 0usize;
        loop {
            // SAFETY: hệ điều hành ghi các bản ghi FILE_ID_BOTH_DIR_INFO nối nhau, căn 8 byte, trong `buf`.
            let info = unsafe { &*(base.add(off) as *const FILE_ID_BOTH_DIR_INFO) };
            let name_len = info.FileNameLength as usize / 2;
            let name_ptr = unsafe { base.add(off + std::mem::offset_of!(FILE_ID_BOTH_DIR_INFO, FileName)) } as *const u16;
            let name = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(name_ptr, name_len) });
            if name != "." && name != ".." {
                let attrs = info.FileAttributes;
                out.push(DirEntry {
                    name,
                    is_dir: attrs & FILE_ATTRIBUTE_DIRECTORY != 0,
                    // Khi có FILE_ATTRIBUTE_REPARSE_POINT, trường EaSize chứa reparse tag.
                    is_link: is_link_tag(attrs, info.EaSize),
                    alloc: info.AllocationSize.max(0) as u64,
                    modified: filetime_to_unix(info.LastWriteTime),
                    file_id: info.FileId as u64,
                });
            }
            if info.NextEntryOffset == 0 {
                break;
            }
            off += info.NextEntryOffset as usize;
        }
    };
    unsafe { CloseHandle(h) };
    result.map(|_| out)
}

/// Mã file đã gặp, chia 16 ngăn để các luồng ít tranh khóa.
struct SeenIds([Mutex<HashSet<u64>>; 16]);

impl SeenIds {
    fn new() -> Self {
        SeenIds(std::array::from_fn(|_| Mutex::new(HashSet::new())))
    }
    /// true nếu đây là lần đầu gặp mã này. Mã 0 (hệ tệp không có mã file) luôn coi là lần đầu.
    fn first(&self, id: u64) -> bool {
        if id == 0 {
            return true;
        }
        self.0[(id % 16) as usize].lock().map(|mut s| s.insert(id)).unwrap_or(true)
    }
}

struct Shared<'a> {
    builder: Mutex<TreeBuilder>,
    queue: Mutex<Vec<(NodeId, PathBuf)>>,
    wake: Condvar,
    /// Số thư mục đang chờ hoặc đang đọc; về 0 là xong.
    pending: AtomicUsize,
    seen: SeenIds,
    files: AtomicU64,
    bytes: AtomicU64,
    throttle: Throttle,
    cancel: &'a CancelToken,
    progress: &'a dyn ScanProgress,
}

impl Shared<'_> {
    fn next_job(&self) -> Option<(NodeId, PathBuf)> {
        let mut q = self.queue.lock().ok()?;
        loop {
            if self.cancel.is_cancelled() {
                return None;
            }
            if let Some(job) = q.pop() {
                return Some(job);
            }
            if self.pending.load(Ordering::SeqCst) == 0 {
                return None;
            }
            q = self.wake.wait_timeout(q, Duration::from_millis(50)).ok()?.0;
        }
    }

    fn visit(&self, node: NodeId, dir: &Path) {
        match read_dir_entries(dir) {
            Err(_) => {
                if let Ok(mut b) = self.builder.lock() {
                    b.add_flags(node, FLAG_UNREADABLE);
                }
            }
            Ok(entries) => {
                let mut subdirs = Vec::new();
                if let Ok(mut b) = self.builder.lock() {
                    for e in entries {
                        if e.is_link {
                            let flags = FLAG_LINK | if e.is_dir { FLAG_DIR } else { 0 };
                            b.add(node, &e.name, flags, 0, e.modified);
                        } else if e.is_dir {
                            let id = b.add(node, &e.name, FLAG_DIR, 0, e.modified);
                            subdirs.push((id, dir.join(&e.name)));
                        } else if self.seen.first(e.file_id) {
                            b.add(node, &e.name, 0, e.alloc, e.modified);
                            self.files.fetch_add(1, Ordering::Relaxed);
                            self.bytes.fetch_add(e.alloc, Ordering::Relaxed);
                        } else {
                            // Tên thứ hai của cùng một file (hard link): hiện ra nhưng không đếm lại.
                            b.add(node, &e.name, FLAG_LINK, 0, e.modified);
                        }
                    }
                }
                if !subdirs.is_empty() {
                    self.pending.fetch_add(subdirs.len(), Ordering::SeqCst);
                    if let Ok(mut q) = self.queue.lock() {
                        q.extend(subdirs);
                    }
                    self.wake.notify_all();
                }
            }
        }
        if self.throttle.ready() {
            self.progress.report(&ScanStatus {
                files: self.files.load(Ordering::Relaxed),
                bytes: self.bytes.load(Ordering::Relaxed),
                current: dir.display().to_string(),
                percent: None,
            });
        }
    }
}

pub struct WalkScanner {
    pub threads: usize,
}

impl Default for WalkScanner {
    /// Thời gian chủ yếu là chờ mở thư mục, không phải CPU: đo trên NVMe (16 luồng CPU), ổ C: 3,8 triệu mục
    /// mất 437 giây với 8 luồng, 85 giây với 32 luồng ⇒ gấp đôi số luồng CPU, trong khoảng 8–32.
    fn default() -> Self {
        let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        WalkScanner { threads: (n * 2).clamp(8, 32) }
    }
}

impl DiskScanner for WalkScanner {
    fn scan(&self, root: &Path, cancel: &CancelToken, progress: &dyn ScanProgress) -> Result<DiskTree> {
        let meta = std::fs::metadata(root).map_err(|e| winfreeup_core::error::io_err(root, e))?;
        if !meta.is_dir() {
            return Err(CoreError::System(format!("not a directory: {}", root.display())));
        }
        let mut builder = TreeBuilder::with_capacity(1 << 16);
        let root_id = builder.add(NO_PARENT, &root.display().to_string(), FLAG_DIR, 0, 0);
        let shared = Shared {
            builder: Mutex::new(builder),
            queue: Mutex::new(vec![(root_id, root.to_path_buf())]),
            wake: Condvar::new(),
            pending: AtomicUsize::new(1),
            seen: SeenIds::new(),
            files: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            throttle: Throttle::new(Duration::from_millis(100)),
            cancel,
            progress,
        };
        std::thread::scope(|s| {
            for _ in 0..self.threads.max(1) {
                s.spawn(|| {
                    while let Some((node, dir)) = shared.next_job() {
                        shared.visit(node, &dir);
                        if shared.pending.fetch_sub(1, Ordering::SeqCst) == 1 {
                            shared.wake.notify_all();
                        }
                    }
                });
            }
        });
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        progress.report(&ScanStatus {
            files: shared.files.load(Ordering::Relaxed),
            bytes: shared.bytes.load(Ordering::Relaxed),
            current: root.display().to_string(),
            percent: None,
        });
        let builder = shared.builder.into_inner().map_err(|e| CoreError::System(e.to_string()))?;
        Ok(builder.finish(root_id))
    }
}
```

- [ ] **Step 4: Chạy test và clippy**

Run: `cargo test disk::scan`, `cargo test disk::walk`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
Expected: scan `1`, walk `10 passed`; toàn bộ `0 failed`. Test `unreadable_directory…` dùng `icacls /deny *S-1-1-0:(RD)` trên thư mục tạm của chính test và gỡ lại ngay.

- [ ] **Step 5: Đo tốc độ (không bắt buộc, ghi số vào commit message)**

Đã đo khi viết kế hoạch (NVMe, 16 luồng CPU, không Admin): `C:\Windows\WinSxS` 755 252 mục — 332 giây với 1 luồng, 96 giây với 8, 35 giây với 32; cả ổ `C:` 3,8 triệu mục — 437 giây với 8 luồng, 85 giây với 32. Thời gian chủ yếu là chờ mở thư mục, nên mặc định lấy gấp đôi số luồng CPU (8–32). Đây là đường chậm; ổ NTFS dưới Admin dùng MFT (Task 17).

- [ ] **Step 6: Commit**

```bash
rtk git add crates/winfreeup-diagnose/src/disk/scan.rs
rtk git add crates/winfreeup-diagnose/src/disk/walk.rs
rtk git commit -m "feat(diagnose): WalkScanner — duyệt song song, dung lượng cấp phát, hard link một lần, không theo junction

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 14: Vòng lấy mẫu hiệu năng và dịch vụ hiệu năng

**Files:**
- Modify (thay dòng khung): `crates/winfreeup-diagnose/src/perf/monitor.rs`, `perf/service.rs`
- Test: hai file trên

**Interfaces:**
- Consumes: Task 6 (`find_stutters`, `find_throttle`, `throttled_now`, `top_apps`, `next_interval_ms`, `AppLoad`, `Point`, `TempState`, `TempTracker`, `TopApp`); Task 7 (`aggregate`, `AppRow`, `EssentialRules`, `PrevTotals`, `ProcSample`, `snapshot`, `own_cpu_100ns`, `PathCache`, `kill`, `file_description`, `icon_data_url`); Task 8 (`CpuDiskCounters`, `memory`, `net_totals`, `NetTrace`, `stop_stale_session`, `ThermalReader`); Task 1 (`ActionLog`, `util::reveal_in_explorer`).
- Produces:
  - `monitor::HISTORY_MS = 300000`, `BASE_INTERVAL_MS = 1000`, `TEMP_EVERY_MS = 5000`; `Sample { t_ms, dur_ms, cpu, ram_pct, ram_used, ram_total, disk_active, net_up_bps, net_down_bps, cpu_perf }`; `StutterView { start_ms, end_ms, cpu, disk, top_cpu, top_disk, throttled }`; `PerfTick { sample, apps, stutters, throttled, perf_error, disk_error, temp, net_app_error, interval_ms }` (payload `perf-tick`); `History` (`push`, `samples`, `stutters`, `throttled_now`); `Sampler::open(rules)`, `tick(interval_ms)`; `TickSink = Arc<dyn Fn(&PerfTick) + Send + Sync>`, `ErrorSink = Arc<dyn Fn(&str) + Send + Sync>`; `PerfMonitor::start(&self, rules, on_tick, on_error) -> Vec<Sample>`, `stop()`, `is_running()`, `last_apps()`.
  - `service::Killer` + `RealKiller`, `KillResult { killed, errors, log_error }`, `PerfService::new(rules: Arc<EssentialRules>, killer: Box<dyn Killer>, log: Arc<ActionLog>)`, `start(on_tick, on_error) -> Vec<Sample>`, `stop()`, `shutdown()`, `kill_app(key) -> Result<KillResult, String>` (`essential`, `unknown_app`), `reveal(path) -> Result<(), String>`, `icon(path) -> Option<String>`.

- [ ] **Step 1: Viết test hỏng** — thay dòng khung của hai file bằng phần test:

`monitor.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn sample(t_ms: u64, cpu: f32, disk: f32) -> Sample {
        Sample {
            t_ms,
            dur_ms: 1000,
            cpu,
            ram_pct: 50.0,
            ram_used: 1,
            ram_total: 2,
            disk_active: Some(disk),
            net_up_bps: None,
            net_down_bps: None,
            cpu_perf: Some(100.0),
        }
    }

    fn load(key: &str, cpu: f32, disk: f64) -> AppLoad {
        AppLoad { key: key.into(), name: key.into(), cpu, disk_bps: disk }
    }

    #[test]
    fn history_keeps_only_the_last_five_minutes() {
        let mut h = History::default();
        for i in 0..400u64 {
            h.push(sample(1_000_000 + i * 1000, 1.0, 1.0), Vec::new());
        }
        let s = h.samples();
        assert_eq!(s.last().unwrap().t_ms - s.first().unwrap().t_ms, HISTORY_MS);
        assert_eq!(h.len(), 301);
    }

    #[test]
    fn stutter_lists_top_apps_for_the_metric_that_spiked_only() {
        let mut h = History::default();
        let t0 = 1_000_000;
        h.push(sample(t0, 10.0, 5.0), vec![load("idle", 1.0, 0.0)]);
        for i in 1..=3 {
            h.push(sample(t0 + i * 1000, 97.0, 5.0), vec![load("game", 80.0, 1e6), load("av", 15.0, 9e6)]);
        }
        h.push(sample(t0 + 4000, 10.0, 5.0), vec![]);
        let s = h.stutters();
        assert_eq!(s.len(), 1);
        assert!(s[0].cpu && !s[0].disk);
        assert_eq!(s[0].top_cpu[0].key, "game");
        assert!(s[0].top_disk.is_empty());
        assert!(!s[0].throttled);
    }

    #[test]
    fn stutter_during_throttling_is_flagged() {
        let mut h = History::default();
        for i in 0..12u64 {
            let mut s = sample(2_000_000 + i * 1000, 95.0, 5.0);
            s.cpu_perf = Some(50.0);
            h.push(s, vec![load("x", 90.0, 0.0)]);
        }
        assert!(h.stutters()[0].throttled);
        assert_eq!(h.throttled_now(), Some(true));
    }

    #[test]
    fn real_monitor_emits_ticks_with_apps_and_stops() {
        let m = PerfMonitor::default();
        let ticks = Arc::new(Mutex::new(Vec::new()));
        let sink = ticks.clone();
        let rules = Arc::new(EssentialRules::new(Path::new(r"C:\Windows\System32"), std::process::id()));
        let before = m.start(rules, Arc::new(move |t: &PerfTick| sink.lock().unwrap().push(t.clone())), Arc::new(|_: &str| {}));
        assert!(before.is_empty());
        // Mở nguồn (PDH, WMI, thử ETW) + mẫu mồi mất vài giây, lâu hơn khi máy đang bận: chờ tối đa 30 giây.
        let deadline = Instant::now() + Duration::from_secs(30);
        while ticks.lock().unwrap().len() < 2 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
        m.stop();
        assert!(!m.is_running());
        let got = ticks.lock().unwrap();
        assert!(!got.is_empty(), "ít nhất một mẫu");
        let t = &got[0];
        assert!((0.0..=100.0).contains(&t.sample.cpu));
        assert!(t.sample.ram_total > 0);
        assert!(t.apps.len() > 5);
        assert!(t.apps.iter().any(|a| a.essential), "svchost/System luôn có");
        assert!(!m.last_apps().is_empty());
        let n = got.len();
        drop(got);
        std::thread::sleep(Duration::from_millis(1300));
        assert_eq!(ticks.lock().unwrap().len(), n, "đã dừng thì không còn mẫu");
    }
}
```

`service.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::perf::apps::ProcRow;

    #[derive(Default)]
    struct FakeKiller {
        fail_pid: Option<u32>,
        calls: Mutex<Vec<u32>>,
    }

    impl Killer for FakeKiller {
        fn kill(&self, pid: u32, _: i64) -> Result<(), String> {
            self.calls.lock().unwrap().push(pid);
            if Some(pid) == self.fail_pid {
                Err("Access is denied. (os error 5)".into())
            } else {
                Ok(())
            }
        }
    }

    fn proc_row(pid: u32, essential: bool) -> ProcRow {
        ProcRow { pid, create_time: 1, name: "x.exe".into(), ram: 1, cpu: 0.0, disk_bps: 0.0, net_up_bps: None, net_down_bps: None, essential }
    }

    fn app(key: &str, procs: Vec<ProcRow>) -> AppRow {
        AppRow {
            key: key.into(),
            name: key.into(),
            path: Some(format!(r"C:\Apps\{key}")),
            ram: 1,
            cpu: 0.0,
            disk_bps: 0.0,
            net_up_bps: None,
            net_down_bps: None,
            essential: procs.iter().any(|p| p.essential),
            procs,
        }
    }

    fn service(killer: FakeKiller) -> (tempfile::TempDir, PerfService) {
        let t = tempfile::tempdir().unwrap();
        let rules = Arc::new(EssentialRules::new(Path::new(r"C:\Windows\System32"), 1));
        let s = PerfService::new(rules, Box::new(killer), Arc::new(ActionLog::new(t.path().join("logs"))));
        (t, s)
    }

    #[test]
    fn kill_app_ends_every_process_and_logs() {
        let (t, s) = service(FakeKiller::default());
        let apps = vec![app("game.exe", vec![proc_row(10, false), proc_row(11, false)])];
        let r = s.kill_in(&apps, "game.exe").unwrap();
        assert_eq!(r, KillResult { killed: 2, errors: vec![], log_error: None });
        let log = std::fs::read_dir(t.path().join("logs")).unwrap().next().unwrap().unwrap().path();
        let text = std::fs::read_to_string(log).unwrap();
        assert!(text.contains(r"APP_KILL pid=10 C:\Apps\game.exe"));
    }

    #[test]
    fn essential_apps_are_refused_without_touching_any_process() {
        let (_t, s) = service(FakeKiller::default());
        let apps = vec![app("svchost", vec![proc_row(20, false), proc_row(21, true)])];
        assert_eq!(s.kill_in(&apps, "svchost").unwrap_err(), "essential");
        assert_eq!(s.kill_in(&apps, "khong-co").unwrap_err(), "unknown_app");
    }

    #[test]
    fn partial_failure_reports_the_verbatim_error() {
        let (_t, s) = service(FakeKiller { fail_pid: Some(31), ..Default::default() });
        let apps = vec![app("a", vec![proc_row(30, false), proc_row(31, false)])];
        let r = s.kill_in(&apps, "a").unwrap();
        assert_eq!(r.killed, 1);
        assert_eq!(r.errors, vec!["Access is denied. (os error 5)".to_string()]);
    }

    #[test]
    fn icons_are_cached_including_misses() {
        let (_t, s) = service(FakeKiller::default());
        assert!(s.icon(r"C:\khong-co\x.exe").is_none());
        assert!(s.icons.lock().unwrap().contains_key(r"C:\khong-co\x.exe"));
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test perf::`
Expected: FAIL biên dịch — `cannot find type History`, `cannot find type PerfService`.

- [ ] **Step 3: Viết mã (trên khối test)**

`monitor.rs`:

```rust
//! Vòng lấy mẫu của mục Bộ nhớ & Hiệu năng (spec 5): chỉ chạy khi mục đang hiển thị, mỗi 1 giây,
//! giữ 5 phút gần nhất; chi phí của chính mình > 2% CPU ⇒ giãn còn 2 giây.
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::apps::{aggregate, AppRow, EssentialRules, PrevTotals, ProcSample};
use super::counters::{memory, net_totals, CpuDiskCounters};
use super::detect::{find_stutters, find_throttle, next_interval_ms, throttled_now, top_apps, AppLoad, Point, TempState, TempTracker, TopApp};
use super::icon::file_description;
use super::netetw::NetTrace;
use super::procs::{own_cpu_100ns, snapshot, PathCache};
use super::thermal::ThermalReader;

pub const HISTORY_MS: u64 = 5 * 60 * 1000;
pub const BASE_INTERVAL_MS: u32 = 1000;
/// Đọc nhiệt độ mỗi 5 giây — truy vấn WMI tốn hơn các bộ đếm khác.
pub const TEMP_EVERY_MS: u64 = 5000;

/// Số liệu toàn máy của một mẫu — điểm trên bốn biểu đồ.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sample {
    /// Giờ Unix (ms) lúc lấy mẫu.
    pub t_ms: u64,
    pub dur_ms: u64,
    pub cpu: f32,
    pub ram_pct: f32,
    pub ram_used: u64,
    pub ram_total: u64,
    pub disk_active: Option<f32>,
    pub net_up_bps: Option<f64>,
    pub net_down_bps: Option<f64>,
    pub cpu_perf: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StutterView {
    pub start_ms: u64,
    pub end_ms: u64,
    pub cpu: bool,
    pub disk: bool,
    pub top_cpu: Vec<TopApp>,
    pub top_disk: Vec<TopApp>,
    /// Trùng lúc CPU đang bị hạ xung.
    pub throttled: bool,
}

/// Payload sự kiện `perf-tick`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PerfTick {
    pub sample: Sample,
    pub apps: Vec<AppRow>,
    pub stutters: Vec<StutterView>,
    /// `None` ⇒ bộ đếm hiệu năng CPU không có: ẩn chỉ báo hạ xung.
    pub throttled: Option<bool>,
    pub perf_error: Option<String>,
    pub disk_error: Option<String>,
    pub temp: TempState,
    /// `None` ⇒ ETW đang chạy (có cột Mạng theo app); có chữ ⇒ lý do nguyên văn, cột Mạng hiện «–».
    pub net_app_error: Option<String>,
    pub interval_ms: u32,
}

struct Frame {
    sample: Sample,
    loads: Vec<AppLoad>,
}

/// 5 phút mẫu gần nhất cùng tải từng app — đủ để tìm cơn giật và 3 app ngốn nhất trong cơn.
#[derive(Default)]
pub struct History {
    frames: VecDeque<Frame>,
}

impl History {
    pub fn push(&mut self, sample: Sample, loads: Vec<AppLoad>) {
        let cutoff = sample.t_ms.saturating_sub(HISTORY_MS);
        self.frames.push_back(Frame { sample, loads });
        while self.frames.front().is_some_and(|f| f.sample.t_ms < cutoff) {
            self.frames.pop_front();
        }
    }

    pub fn samples(&self) -> Vec<Sample> {
        self.frames.iter().map(|f| f.sample.clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    fn points(&self) -> Vec<Point> {
        self.frames
            .iter()
            .map(|f| Point {
                t_ms: f.sample.t_ms,
                dur_ms: f.sample.dur_ms,
                cpu: f.sample.cpu,
                disk: f.sample.disk_active.unwrap_or(0.0),
                perf: f.sample.cpu_perf,
            })
            .collect()
    }

    pub fn throttled_now(&self) -> Option<bool> {
        throttled_now(&self.points())
    }

    pub fn stutters(&self) -> Vec<StutterView> {
        let points = self.points();
        let throttle = find_throttle(&points);
        find_stutters(&points)
            .into_iter()
            .map(|s| {
                let frames: Vec<&[AppLoad]> = self.frames.range(s.first..=s.last).map(|f| f.loads.as_slice()).collect();
                StutterView {
                    start_ms: s.start_ms,
                    end_ms: s.end_ms,
                    cpu: s.cpu,
                    disk: s.disk,
                    top_cpu: if s.cpu { top_apps(&frames, true, 3) } else { Vec::new() },
                    top_disk: if s.disk { top_apps(&frames, false, 3) } else { Vec::new() },
                    throttled: throttle.iter().any(|t| t.start_ms < s.end_ms && s.start_ms < t.end_ms),
                }
            })
            .collect()
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Mọi nguồn số liệu thật. Nguồn nào không mở được thì ghi lý do, phần còn lại vẫn chạy (spec 6).
pub struct Sampler {
    counters: Result<CpuDiskCounters, String>,
    net: Option<NetTrace>,
    net_error: Option<String>,
    thermal: ThermalReader,
    temp: TempTracker,
    last_temp: TempState,
    last_temp_ms: u64,
    paths: PathCache,
    descriptions: HashMap<String, String>,
    prev: PrevTotals,
    prev_net: Option<(u64, u64)>,
    last: Instant,
    cpus: u32,
    rules: Arc<EssentialRules>,
}

impl Sampler {
    pub fn open(rules: Arc<EssentialRules>) -> Sampler {
        let (net, net_error) = match NetTrace::start() {
            Ok(t) => (Some(t), None),
            Err(e) => (None, Some(e)),
        };
        Sampler {
            counters: CpuDiskCounters::open().map_err(|e| e.to_string()),
            net,
            net_error,
            thermal: ThermalReader::new(),
            temp: TempTracker::default(),
            last_temp: TempState::Unavailable { code: "no_sensor".into(), detail: String::new() },
            last_temp_ms: 0,
            paths: PathCache::default(),
            descriptions: HashMap::new(),
            prev: PrevTotals::new(),
            prev_net: net_totals().ok(),
            last: Instant::now(),
            cpus: std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1),
            rules,
        }
    }

    fn description(&mut self, path: &str) -> String {
        self.descriptions.entry(path.to_string()).or_insert_with(|| file_description(Path::new(path))).clone()
    }

    /// Lấy một mẫu. Lỗi chỉ khi không đọc được CPU lẫn danh sách tiến trình.
    pub fn tick(&mut self, interval_ms: u32) -> Result<(PerfTick, Vec<AppLoad>), String> {
        let t_ms = now_ms();
        let dt_ms = self.last.elapsed().as_millis().max(1) as u64;
        self.last = Instant::now();
        let reading = self.counters.as_ref().map_err(|e| e.clone()).and_then(|c| c.read().map_err(|e| e.to_string()));
        let (cpu, disk, perf) = match &reading {
            Ok(r) => (r.cpu.unwrap_or(0.0), r.disk_active, r.perf),
            Err(_) => (0.0, None, None),
        };
        let mem = memory().ok();
        let net = net_totals().ok();
        let (net_up, net_down) = match (self.prev_net, net) {
            (Some((rx0, tx0)), Some((rx, tx))) => {
                let s = dt_ms as f64 / 1000.0;
                (Some(tx.saturating_sub(tx0) as f64 / s), Some(rx.saturating_sub(rx0) as f64 / s))
            }
            _ => (None, None),
        };
        self.prev_net = net;
        let procs = snapshot().map_err(|e| e.to_string());
        if reading.is_err() && procs.is_err() {
            return Err(reading.err().unwrap_or_default());
        }
        let per_pid_net = self.net.as_ref().map(|n| n.take());
        let mut samples = Vec::new();
        let mut alive = HashSet::new();
        for p in procs.unwrap_or_default() {
            alive.insert((p.pid, p.create_time));
            let path = self.paths.get(p.pid, p.create_time);
            let description = path.as_deref().map(|x| self.description(x)).unwrap_or_default();
            let net = per_pid_net.as_ref().map(|m| m.get(&p.pid).copied().unwrap_or((0, 0)));
            samples.push(ProcSample {
                pid: p.pid,
                parent_pid: p.parent_pid,
                name: p.name,
                path,
                description,
                create_time: p.create_time,
                private_ws: p.private_ws,
                cpu_100ns: p.cpu_100ns,
                io_bytes: p.io_bytes,
                net,
            });
        }
        self.paths.retain(&alive);
        let (apps, next) = aggregate(&self.prev, &samples, dt_ms, self.cpus, &self.rules);
        self.prev = next;
        if t_ms.saturating_sub(self.last_temp_ms) >= TEMP_EVERY_MS {
            self.last_temp = self.temp.push(t_ms, self.thermal.read());
            self.last_temp_ms = t_ms;
        }
        let loads = apps
            .iter()
            .map(|a| AppLoad { key: a.key.as_str().into(), name: a.name.as_str().into(), cpu: a.cpu, disk_bps: a.disk_bps })
            .collect();
        let sample = Sample {
            t_ms,
            dur_ms: dt_ms,
            cpu,
            ram_pct: mem.map(|m| m.load_pct as f32).unwrap_or(0.0),
            ram_used: mem.map(|m| m.total.saturating_sub(m.available)).unwrap_or(0),
            ram_total: mem.map(|m| m.total).unwrap_or(0),
            disk_active: disk,
            net_up_bps: net_up,
            net_down_bps: net_down,
            cpu_perf: perf,
        };
        let (perf_error, disk_error) = match &self.counters {
            Ok(c) => (c.perf_error.clone(), c.disk_error.clone()),
            Err(e) => (Some(e.clone()), Some(e.clone())),
        };
        let tick = PerfTick {
            sample,
            apps,
            stutters: Vec::new(),
            throttled: None,
            perf_error,
            disk_error,
            temp: self.last_temp.clone(),
            net_app_error: self.net_error.clone(),
            interval_ms,
        };
        Ok((tick, loads))
    }

    pub fn close(&mut self) {
        if let Some(n) = self.net.take() {
            n.stop();
        }
    }
}

pub type TickSink = Arc<dyn Fn(&PerfTick) + Send + Sync>;
pub type ErrorSink = Arc<dyn Fn(&str) + Send + Sync>;

/// Luồng lấy mẫu chạy nền. `start` hai lần liên tiếp chỉ đổi nơi nhận; `stop` chờ luồng dừng hẳn.
#[derive(Default)]
pub struct PerfMonitor {
    history: Arc<Mutex<History>>,
    last_apps: Arc<Mutex<Vec<AppRow>>>,
    running: Arc<AtomicBool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl PerfMonitor {
    /// Bắt đầu lấy mẫu; trả lịch sử sẵn có (mẫu cũ của lần xem trước, nếu còn trong 5 phút).
    pub fn start(&self, rules: Arc<EssentialRules>, on_tick: TickSink, on_error: ErrorSink) -> Vec<Sample> {
        let mut handle = self.handle.lock().unwrap_or_else(|e| e.into_inner());
        let existing = self.history.lock().map(|h| h.samples()).unwrap_or_default();
        if handle.as_ref().is_some_and(|h| !h.is_finished()) {
            return existing;
        }
        self.running.store(true, Ordering::SeqCst);
        let (history, last_apps, running) = (self.history.clone(), self.last_apps.clone(), self.running.clone());
        *handle = Some(std::thread::spawn(move || {
            let mut sampler = Sampler::open(rules);
            let mut interval = BASE_INTERVAL_MS;
            let mut own_prev = own_cpu_100ns();
            let mut wall_prev = Instant::now();
            // Mẫu đầu chỉ để mồi bộ đếm tốc độ.
            let _ = sampler.tick(interval);
            while running.load(Ordering::SeqCst) {
                let wake = Instant::now() + Duration::from_millis(u64::from(interval));
                while running.load(Ordering::SeqCst) && Instant::now() < wake {
                    std::thread::sleep(Duration::from_millis(50));
                }
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                match sampler.tick(interval) {
                    Ok((mut tick, loads)) => {
                        if let Ok(mut h) = history.lock() {
                            h.push(tick.sample.clone(), loads);
                            tick.stutters = h.stutters();
                            tick.throttled = h.throttled_now();
                        }
                        if let Ok(mut a) = last_apps.lock() {
                            a.clone_from(&tick.apps);
                        }
                        on_tick(&tick);
                    }
                    Err(e) => on_error(&e),
                }
                let own = own_cpu_100ns();
                let wall = wall_prev.elapsed().as_millis().max(1) as f64;
                let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
                let pct = (own.saturating_sub(own_prev) as f64 / 10_000.0) / (wall * cpus) * 100.0;
                interval = next_interval_ms(interval, pct as f32);
                own_prev = own;
                wall_prev = Instant::now();
            }
            sampler.close();
        }));
        existing
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        let h = self.handle.lock().ok().and_then(|mut h| h.take());
        if let Some(h) = h {
            let _ = h.join();
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Bảng ứng dụng của mẫu gần nhất (để tìm tiến trình khi người dùng bấm Kết thúc app).
    pub fn last_apps(&self) -> Vec<AppRow> {
        self.last_apps.lock().map(|a| a.clone()).unwrap_or_default()
    }
}

impl Drop for PerfMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}
```

`service.rs`:

```rust
//! Tầng dịch vụ của mục Bộ nhớ & Hiệu năng: bật/tắt lấy mẫu, kết thúc app có chặn tiến trình thiết yếu,
//! mở vị trí file, biểu tượng app (nhớ đệm). Lỗi dạng chuỗi: `essential`, `unknown_app`, hoặc nguyên văn.
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::Serialize;

use super::apps::{AppRow, EssentialRules};
use super::icon::icon_data_url;
use super::monitor::{ErrorSink, PerfMonitor, Sample, TickSink};
use super::netetw::stop_stale_session;
use crate::actlog::ActionLog;

/// Kết thúc một tiến trình cụ thể — bản thật gọi `procs::kill`, test dùng bản giả.
pub trait Killer: Send + Sync {
    fn kill(&self, pid: u32, create_time: i64) -> Result<(), String>;
}

pub struct RealKiller;

impl Killer for RealKiller {
    fn kill(&self, pid: u32, create_time: i64) -> Result<(), String> {
        super::procs::kill(pid, create_time).map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct KillResult {
    pub killed: u32,
    /// Thông điệp nguyên văn cho từng tiến trình không kết thúc được.
    pub errors: Vec<String>,
    pub log_error: Option<String>,
}

pub struct PerfService {
    monitor: PerfMonitor,
    rules: Arc<EssentialRules>,
    killer: Box<dyn Killer>,
    log: Arc<ActionLog>,
    icons: Mutex<HashMap<String, Option<String>>>,
}

impl PerfService {
    pub fn new(rules: Arc<EssentialRules>, killer: Box<dyn Killer>, log: Arc<ActionLog>) -> Self {
        PerfService { monitor: PerfMonitor::default(), rules, killer, log, icons: Mutex::new(HashMap::new()) }
    }

    /// Mục Bộ nhớ & Hiệu năng vừa hiện ra. Trả lịch sử còn giữ để vẽ ngay.
    pub fn start(&self, on_tick: TickSink, on_error: ErrorSink) -> Vec<Sample> {
        self.monitor.start(self.rules.clone(), on_tick, on_error)
    }

    /// Rời mục ⇒ dừng lấy mẫu (và dừng phiên ETW).
    pub fn stop(&self) {
        self.monitor.stop();
    }

    /// Đóng ứng dụng: dừng lấy mẫu và phiên ETW còn sót.
    pub fn shutdown(&self) {
        self.monitor.stop();
        stop_stale_session();
    }

    /// Kết thúc mọi tiến trình của một app (theo khóa của mẫu gần nhất). Luật chặn kiểm lại ở đây.
    pub fn kill_app(&self, key: &str) -> Result<KillResult, String> {
        let apps = self.monitor.last_apps();
        self.kill_in(&apps, key)
    }

    fn kill_in(&self, apps: &[AppRow], key: &str) -> Result<KillResult, String> {
        let app = apps.iter().find(|a| a.key == key).ok_or("unknown_app")?;
        if app.essential || app.procs.iter().any(|p| p.essential) {
            return Err("essential".into());
        }
        let mut result = KillResult { killed: 0, errors: Vec::new(), log_error: None };
        let path = app.path.as_deref().unwrap_or(&app.key);
        for p in &app.procs {
            match self.killer.kill(p.pid, p.create_time) {
                Ok(()) => {
                    result.killed += 1;
                    if let Err(e) = self.log.line(&format!("APP_KILL pid={} {path}", p.pid)) {
                        result.log_error = Some(e);
                    }
                }
                Err(e) => {
                    let _ = self.log.line(&format!("APP_KILL_FAILED pid={} {path} -- {e}", p.pid));
                    result.errors.push(e);
                }
            }
        }
        Ok(result)
    }

    pub fn reveal(&self, path: &str) -> Result<(), String> {
        crate::util::reveal_in_explorer(Path::new(path))
    }

    pub fn icon(&self, path: &str) -> Option<String> {
        if let Some(v) = self.icons.lock().ok().and_then(|m| m.get(path).cloned()) {
            return v;
        }
        let v = icon_data_url(Path::new(path));
        if let Ok(mut m) = self.icons.lock() {
            m.insert(path.to_string(), v.clone());
        }
        v
    }
}
```

- [ ] **Step 4: Chạy test và clippy**

Run: `cargo test perf::monitor`, `cargo test perf::service`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
Expected: monitor `4`, service `4 passed`; toàn bộ `0 failed`. Đã đo khi viết kế hoạch (bản release, máy có 995 tiến trình): mở nguồn 1,8 giây, mẫu đầu 0,6 giây, mỗi mẫu sau 35–60 ms CPU.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-diagnose/src/perf/monitor.rs
rtk git add crates/winfreeup-diagnose/src/perf/service.rs
rtk git commit -m "feat(diagnose): vòng lấy mẫu 1 giây giữ 5 phút, tự giãn 2 giây, kết thúc app có chặn thiết yếu

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 15: Luật khám nhanh (`evaluate`)

**Files:**
- Modify (thay dòng khung): `crates/winfreeup-diagnose/src/health/rules.rs`
- Test: `rules.rs`

**Interfaces:**
- Consumes: Task 8 (`perf::counters::MemoryStatus`); Task 6 (`perf::detect::{find_throttle, Point, TEMP_MAX_C, TEMP_MIN_C}`).
- Produces: `Level { Critical, Warn, Ok, Unknown }` (serde chữ thường), `Finding { id: &'static str, level, value: Option<f64>, detail: Option<String> }`, `Measured<T> = Result<T, String>`, `DiskHealth { name, health_status: u16, predict_failure: Option<bool> }`, `MediaType { Hdd, Ssd, Scm, Unspecified }` + `from_code(u16)`, `PowerState { on_ac, saver }`, `HealthMetrics { system_free, disks, system_media, memory, startup_enabled, uptime_secs, power, cpu_temp_c }`, từng luật `disk_full`, `disk_health`, `system_hdd`, `ram_pressure`, `startup_apps`, `uptime`, `power_saver`, `cpu_hot`, `cpu_throttle(&Measured<Vec<Point>>)`, và `evaluate(&HealthMetrics) -> Vec<Finding>` (8 dòng đúng thứ tự bảng spec; `cpu_throttle` đến riêng).

- [ ] **Step 1: Viết test hỏng** — thay dòng khung bằng:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1 << 30;

    fn mem(load: u32, used: u64, limit: u64) -> Measured<MemoryStatus> {
        Ok(MemoryStatus { load_pct: load, total: 16 * GB, available: 4 * GB, commit_used: used, commit_limit: limit })
    }

    #[test]
    fn disk_full_thresholds_and_exact_boundaries() {
        assert_eq!(disk_full(&Ok((15, 100))).level, Level::Ok, "đúng 15% chưa cảnh báo");
        assert_eq!(disk_full(&Ok((149, 1000))).level, Level::Warn);
        assert_eq!(disk_full(&Ok((5, 100))).level, Level::Warn, "đúng 5% chưa nghiêm trọng");
        assert_eq!(disk_full(&Ok((49, 1000))).level, Level::Critical);
        assert_eq!(disk_full(&Ok((49, 1000))).value, Some(4.9));
        assert_eq!(disk_full(&Ok((0, 0))).level, Level::Unknown);
    }

    #[test]
    fn disk_health_from_status_and_smart() {
        let d = |s: u16, p: Option<bool>| DiskHealth { name: "Samsung".into(), health_status: s, predict_failure: p };
        assert_eq!(disk_health(&Ok(vec![d(0, Some(false))])).level, Level::Ok);
        assert_eq!(disk_health(&Ok(vec![d(0, None)])).level, Level::Ok, "NVMe không có SMART cũ vẫn ổn nếu HealthStatus tốt");
        let bad = disk_health(&Ok(vec![d(0, None), d(1, None)]));
        assert_eq!((bad.level, bad.detail.as_deref()), (Level::Critical, Some("Samsung")));
        assert_eq!(disk_health(&Ok(vec![d(2, None)])).level, Level::Critical);
        assert_eq!(disk_health(&Ok(vec![d(0, Some(true))])).level, Level::Critical, "SMART báo sắp hỏng");
        assert_eq!(disk_health(&Ok(vec![d(5, None)])).level, Level::Unknown, "HealthStatus Unknown không phải 🔴");
        assert_eq!(disk_health(&Ok(vec![])).level, Level::Unknown);
    }

    #[test]
    fn system_drive_media_type() {
        assert_eq!(system_hdd(&Ok(MediaType::Hdd)).level, Level::Warn);
        assert_eq!(system_hdd(&Ok(MediaType::Ssd)).level, Level::Ok);
        assert_eq!(system_hdd(&Ok(MediaType::Unspecified)).level, Level::Unknown, "máy ảo thường không rõ loại ổ");
        assert_eq!(MediaType::from_code(3), MediaType::Hdd);
        assert_eq!(MediaType::from_code(0), MediaType::Unspecified);
    }

    #[test]
    fn ram_thresholds_and_boundaries() {
        assert_eq!(ram_pressure(&mem(85, 10, 100)).level, Level::Ok, "đúng 85% chưa cảnh báo");
        assert_eq!(ram_pressure(&mem(86, 10, 100)).level, Level::Warn);
        assert_eq!(ram_pressure(&mem(50, 90, 100)).level, Level::Ok, "commit đúng 90% chưa nghiêm trọng");
        let crit = ram_pressure(&mem(50, 91, 100));
        assert_eq!((crit.level, crit.value), (Level::Critical, Some(91.0)));
    }

    #[test]
    fn startup_uptime_power_temperature() {
        assert_eq!(startup_apps(&Ok(8)).level, Level::Ok);
        assert_eq!(startup_apps(&Ok(9)).level, Level::Warn);
        assert_eq!(uptime(&Ok(UPTIME_WARN_SECS)).level, Level::Ok, "đúng 7 ngày chưa cảnh báo");
        let up = uptime(&Ok(UPTIME_WARN_SECS + 86_400));
        assert_eq!((up.level, up.value), (Level::Warn, Some(8.0)));
        assert_eq!(power_saver(&Ok(PowerState { on_ac: true, saver: true })).level, Level::Warn);
        assert_eq!(power_saver(&Ok(PowerState { on_ac: false, saver: true })).level, Level::Ok, "đang chạy pin thì tiết kiệm là đúng");
        assert_eq!(cpu_hot(&Ok(90.0)).level, Level::Ok);
        assert_eq!(cpu_hot(&Ok(90.5)).level, Level::Warn);
        assert_eq!(cpu_hot(&Ok(150.0)).level, Level::Unknown, "số ngoài 20–110 °C là cảm biến hỏng");
    }

    #[test]
    fn every_missing_source_is_unknown_with_its_reason_never_ok() {
        fn e<T>() -> Measured<T> {
            Err("Access denied".to_string())
        }
        let m = HealthMetrics {
            system_free: e(),
            disks: e(),
            system_media: e(),
            memory: e(),
            startup_enabled: e(),
            uptime_secs: e(),
            power: e(),
            cpu_temp_c: e(),
        };
        let all = evaluate(&m);
        assert_eq!(
            all.iter().map(|f| f.id).collect::<Vec<_>>(),
            vec!["disk_full", "disk_health", "system_hdd", "ram_pressure", "startup_apps", "uptime", "power_saver", "cpu_hot"]
        );
        for f in all {
            assert_eq!(f.level, Level::Unknown, "{}", f.id);
            assert_eq!(f.detail.as_deref(), Some("Access denied"));
        }
        assert_eq!(cpu_throttle(&Err("PDH open: 0xC0000BB8".into())).level, Level::Unknown);
    }

    #[test]
    fn throttle_finding_from_ten_second_sampling() {
        let pts = |perf: Option<f32>| -> Measured<Vec<Point>> {
            Ok((0..10).map(|i| Point { t_ms: (i + 1) * 1000, dur_ms: 1000, cpu: 95.0, disk: 0.0, perf }).collect())
        };
        assert_eq!(cpu_throttle(&pts(Some(50.0))).level, Level::Warn);
        assert_eq!(cpu_throttle(&pts(Some(99.0))).level, Level::Ok);
        assert_eq!(cpu_throttle(&pts(None)).level, Level::Unknown);
    }

    #[test]
    fn finding_serializes_for_the_ui() {
        let v = serde_json::to_value(disk_full(&Ok((10, 100)))).unwrap();
        assert_eq!(v, serde_json::json!({"id": "disk_full", "level": "warn", "value": 10.0, "detail": null}));
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test health::rules`
Expected: FAIL biên dịch — `cannot find function disk_full`.

- [ ] **Step 3: Viết mã (trên khối test)**

```rust
//! Luật khám nhanh (spec 4) — hàm thuần, không gọi hệ thống, test được từng ngưỡng.
//! Nguồn số liệu nào không đọc được ⇒ mức `unknown` kèm lý do nguyên văn, KHÔNG BAO GIỜ là `ok`.
use serde::Serialize;

use crate::perf::counters::MemoryStatus;
use crate::perf::detect::{find_throttle, Point, TEMP_MAX_C, TEMP_MIN_C};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Critical,
    Warn,
    Ok,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Finding {
    pub id: &'static str,
    pub level: Level,
    /// Con số cho câu giải thích: % trống, % RAM, số app, số ngày, °C.
    pub value: Option<f64>,
    /// Lý do «Không đo được» (nguyên văn), hoặc tên ổ đĩa có vấn đề.
    pub detail: Option<String>,
}

pub type Measured<T> = Result<T, String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskHealth {
    pub name: String,
    /// `MSFT_PhysicalDisk.HealthStatus`: 0 Healthy, 1 Warning, 2 Unhealthy, 5 Unknown.
    pub health_status: u16,
    /// SMART `PredictFailure`; `None` khi ổ không hỗ trợ / không đọc được.
    pub predict_failure: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Hdd,
    Ssd,
    Scm,
    Unspecified,
}

impl MediaType {
    /// `MSFT_PhysicalDisk.MediaType`: 3 HDD, 4 SSD, 5 SCM, còn lại chưa rõ.
    pub fn from_code(code: u16) -> MediaType {
        match code {
            3 => MediaType::Hdd,
            4 => MediaType::Ssd,
            5 => MediaType::Scm,
            _ => MediaType::Unspecified,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerState {
    pub on_ac: bool,
    /// Tiết kiệm pin đang bật hoặc gói nguồn đang là «Power saver».
    pub saver: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HealthMetrics {
    /// (byte trống, tổng) của ổ hệ thống.
    pub system_free: Measured<(u64, u64)>,
    pub disks: Measured<Vec<DiskHealth>>,
    pub system_media: Measured<MediaType>,
    pub memory: Measured<MemoryStatus>,
    pub startup_enabled: Measured<u32>,
    pub uptime_secs: Measured<u64>,
    pub power: Measured<PowerState>,
    pub cpu_temp_c: Measured<f32>,
}

pub const DISK_WARN_FREE_PCT: f64 = 15.0;
pub const DISK_CRIT_FREE_PCT: f64 = 5.0;
pub const RAM_WARN_PCT: u32 = 85;
pub const COMMIT_CRIT_RATIO: f64 = 0.90;
pub const STARTUP_WARN_COUNT: u32 = 8;
pub const UPTIME_WARN_SECS: u64 = 7 * 24 * 3600;
pub const CPU_HOT_C: f32 = 90.0;

fn finding(id: &'static str, level: Level, value: Option<f64>) -> Finding {
    Finding { id, level, value, detail: None }
}

fn unknown(id: &'static str, reason: &str) -> Finding {
    Finding { id, level: Level::Unknown, value: None, detail: Some(reason.to_string()) }
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

pub fn disk_full(m: &Measured<(u64, u64)>) -> Finding {
    match m {
        Err(e) => unknown("disk_full", e),
        Ok((_, 0)) => unknown("disk_full", "total size is 0"),
        Ok((free, total)) => {
            let pct = *free as f64 * 100.0 / *total as f64;
            let level = if pct < DISK_CRIT_FREE_PCT {
                Level::Critical
            } else if pct < DISK_WARN_FREE_PCT {
                Level::Warn
            } else {
                Level::Ok
            };
            finding("disk_full", level, Some(round1(pct)))
        }
    }
}

pub fn disk_health(m: &Measured<Vec<DiskHealth>>) -> Finding {
    let disks = match m {
        Err(e) => return unknown("disk_health", e),
        Ok(d) if d.is_empty() => return unknown("disk_health", "no physical disks reported"),
        Ok(d) => d,
    };
    if let Some(bad) = disks.iter().find(|d| matches!(d.health_status, 1 | 2) || d.predict_failure == Some(true)) {
        return Finding { id: "disk_health", level: Level::Critical, value: None, detail: Some(bad.name.clone()) };
    }
    if disks.iter().all(|d| d.health_status == 5 && d.predict_failure.is_none()) {
        return unknown("disk_health", "HealthStatus Unknown");
    }
    finding("disk_health", Level::Ok, None)
}

pub fn system_hdd(m: &Measured<MediaType>) -> Finding {
    match m {
        Err(e) => unknown("system_hdd", e),
        Ok(MediaType::Hdd) => finding("system_hdd", Level::Warn, None),
        Ok(MediaType::Ssd | MediaType::Scm) => finding("system_hdd", Level::Ok, None),
        Ok(MediaType::Unspecified) => unknown("system_hdd", "MediaType Unspecified"),
    }
}

pub fn ram_pressure(m: &Measured<MemoryStatus>) -> Finding {
    match m {
        Err(e) => unknown("ram_pressure", e),
        Ok(mem) => {
            let commit = if mem.commit_limit > 0 { mem.commit_used as f64 / mem.commit_limit as f64 } else { 0.0 };
            if commit > COMMIT_CRIT_RATIO {
                finding("ram_pressure", Level::Critical, Some(round1(commit * 100.0)))
            } else if mem.load_pct > RAM_WARN_PCT {
                finding("ram_pressure", Level::Warn, Some(f64::from(mem.load_pct)))
            } else {
                finding("ram_pressure", Level::Ok, Some(f64::from(mem.load_pct)))
            }
        }
    }
}

pub fn startup_apps(m: &Measured<u32>) -> Finding {
    match m {
        Err(e) => unknown("startup_apps", e),
        Ok(n) => finding("startup_apps", if *n > STARTUP_WARN_COUNT { Level::Warn } else { Level::Ok }, Some(f64::from(*n))),
    }
}

pub fn uptime(m: &Measured<u64>) -> Finding {
    match m {
        Err(e) => unknown("uptime", e),
        Ok(s) => finding("uptime", if *s > UPTIME_WARN_SECS { Level::Warn } else { Level::Ok }, Some(round1(*s as f64 / 86_400.0))),
    }
}

pub fn power_saver(m: &Measured<PowerState>) -> Finding {
    match m {
        Err(e) => unknown("power_saver", e),
        Ok(p) => finding("power_saver", if p.on_ac && p.saver { Level::Warn } else { Level::Ok }, None),
    }
}

pub fn cpu_hot(m: &Measured<f32>) -> Finding {
    match m {
        Err(e) => unknown("cpu_hot", e),
        Ok(c) if !(TEMP_MIN_C..=TEMP_MAX_C).contains(c) => unknown("cpu_hot", &format!("out of range: {c:.1} °C")),
        Ok(c) => finding("cpu_hot", if *c > CPU_HOT_C { Level::Warn } else { Level::Ok }, Some(round1(f64::from(*c)))),
    }
}

/// Spec 4 `cpu_throttle`: lấy mẫu 10 giây lúc khám, áp luật mục 5.4.
pub fn cpu_throttle(points: &Measured<Vec<Point>>) -> Finding {
    match points {
        Err(e) => unknown("cpu_throttle", e),
        Ok(p) if p.is_empty() || p.iter().any(|x| x.perf.is_none()) => unknown("cpu_throttle", "% Processor Performance counter is missing"),
        Ok(p) => finding("cpu_throttle", if find_throttle(p).is_empty() { Level::Ok } else { Level::Warn }, None),
    }
}

/// Tám dòng khám nhanh, đúng thứ tự bảng spec mục 4 (dòng `cpu_throttle` đến riêng sau 10 giây).
pub fn evaluate(m: &HealthMetrics) -> Vec<Finding> {
    vec![
        disk_full(&m.system_free),
        disk_health(&m.disks),
        system_hdd(&m.system_media),
        ram_pressure(&m.memory),
        startup_apps(&m.startup_enabled),
        uptime(&m.uptime_secs),
        power_saver(&m.power),
        cpu_hot(&m.cpu_temp_c),
    ]
}
```

- [ ] **Step 4: Chạy test và clippy**

Run: `cargo test health::rules`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
Expected: `8 passed`; toàn bộ `0 failed`.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-diagnose/src/health/rules.rs
rtk git commit -m "feat(diagnose): luật khám nhanh thuần — từng ngưỡng, biên, nguồn thiếu là Không đo được

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 16: Tab Khám máy — ba mục con

**Files:**
- Create: `src/features/kham-may/KhamMayTab.tsx`, `KhamMayTab.test.tsx`

**Interfaces:**
- Consumes: Task 2 (`KhamMayApi`, `Notify`, `khamMayTauriApi`, `tk`, `km.css`, `fakeApi`); Task 10 (`DiskView`); Task 11 (`OverviewView`, `NavTarget`); Task 12 (`PerfView`).
- Produces: `KhamMayTab({ api?: KhamMayApi /* mặc định khamMayTauriApi */, notify: Notify, onGoClean?: () => void, active?: boolean /* mặc định true */ })` — Task 20 gắn vào `App`.

- [ ] **Step 1: Viết test hỏng**

`src/features/kham-may/KhamMayTab.test.tsx`:

```tsx
import { describe, expect, it, vi } from 'vitest';
import { configure, fireEvent, render, screen } from '@testing-library/react';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import type { KhamMayApi } from './api/types';
import { fakeApi } from './testing/fakeApi';
import { KhamMayTab } from './KhamMayTab';

configure({ asyncUtilTimeout: 5000 });

function setup(over: Partial<KhamMayApi> = {}) {
  const api = fakeApi(over);
  const notify = vi.fn();
  const onGoClean = vi.fn();
  const ui = (active: boolean) => (
    <FluentProvider theme={webLightTheme}>
      <KhamMayTab api={api} notify={notify} onGoClean={onGoClean} active={active} />
    </FluentProvider>
  );
  const view = render(ui(true));
  return { api, notify, onGoClean, setActive: (a: boolean) => view.rerender(ui(a)) };
}

describe('Tab Khám máy', () => {
  it('mặc định mở Tổng quan và khám ngay', async () => {
    const { api } = setup();
    expect((screen.getByRole('tab', { name: 'Tổng quan' }) as HTMLElement).getAttribute('aria-selected')).toBe('true');
    await vi.waitFor(() => expect(api.healthCheck).toHaveBeenCalledTimes(1));
    expect(api.perfStart).not.toHaveBeenCalled();
  });

  it('Bộ nhớ & Hiệu năng chỉ lấy mẫu khi đang xem; quay lại Tổng quan không khám lại', async () => {
    const { api } = setup();
    await vi.waitFor(() => expect(api.healthCheck).toHaveBeenCalled());
    fireEvent.click(screen.getByRole('tab', { name: 'Bộ nhớ & Hiệu năng' }));
    await vi.waitFor(() => expect(api.perfStart).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole('tab', { name: 'Tổng quan' }));
    await vi.waitFor(() => expect(api.perfStop).toHaveBeenCalledTimes(1));
    expect(api.healthCheck).toHaveBeenCalledTimes(1);
  });

  it('App chuyển sang tab khác (active = false) thì cũng dừng lấy mẫu, quay lại thì chạy tiếp', async () => {
    const { api, setActive } = setup();
    fireEvent.click(screen.getByRole('tab', { name: 'Bộ nhớ & Hiệu năng' }));
    await vi.waitFor(() => expect(api.perfStart).toHaveBeenCalledTimes(1));
    setActive(false);
    await vi.waitFor(() => expect(api.perfStop).toHaveBeenCalledTimes(1));
    setActive(true);
    await vi.waitFor(() => expect(api.perfStart).toHaveBeenCalledTimes(2));
  });

  it('nút trên dòng khám dẫn sang mục Ổ đĩa hoặc sang tab Dọn dẹp', async () => {
    const { api, onGoClean } = setup({
      healthCheck: vi.fn(async () => [{ id: 'disk_full' as const, level: 'critical' as const, value: 3, detail: null }]),
    });
    fireEvent.click(await screen.findByRole('button', { name: 'Dọn dẹp' }));
    expect(onGoClean).toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Xem Ổ đĩa' }));
    expect((screen.getByRole('tab', { name: 'Ổ đĩa' }) as HTMLElement).getAttribute('aria-selected')).toBe('true');
    await vi.waitFor(() => expect(api.diskVolumes).toHaveBeenCalled());
  });
});
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `npx vitest run src/features/kham-may/KhamMayTab.test.tsx`
Expected: FAIL — `Failed to resolve import "./KhamMayTab"`.

- [ ] **Step 3: Viết mã**

`src/features/kham-may/KhamMayTab.tsx`:

```tsx
import { useState } from 'react';
import { Tab, TabList } from '@fluentui/react-components';
import type { KhamMayApi, Notify } from './api/types';
import { khamMayTauriApi } from './api/tauri';
import { DiskView } from './disk/DiskView';
import { tk } from './i18n';
import { OverviewView, type NavTarget } from './overview/OverviewView';
import { PerfView } from './perf/PerfView';
import './km.css';

type Sub = 'overview' | 'disk' | 'perf';

/**
 * Tab «Khám máy» (spec mục 2): ba mục con, mặc định Tổng quan.
 * Tổng quan và Ổ đĩa giữ nguyên khi chuyển mục (không khám/quét lại); Bộ nhớ & Hiệu năng chỉ gắn khi
 * đang xem, để việc lấy mẫu dừng ngay khi rời mục (spec 5). `active = false` (App đang hiện tab khác)
 * cũng coi là rời mục.
 */
export function KhamMayTab({
  api = khamMayTauriApi,
  notify,
  onGoClean,
  active = true,
}: {
  api?: KhamMayApi;
  notify: Notify;
  onGoClean?: () => void;
  active?: boolean;
}) {
  const [sub, setSub] = useState<Sub>('overview');
  const [diskVisited, setDiskVisited] = useState(false);

  function go(s: Sub) {
    if (s === 'disk') setDiskVisited(true);
    setSub(s);
  }

  function navigate(t: NavTarget) {
    if (t === 'clean') onGoClean?.();
    else go(t);
  }

  return (
    <div className="km-root">
      <TabList selectedValue={sub} onTabSelect={(_, d) => go(d.value as Sub)}>
        <Tab value="overview">{tk('km.tab.overview')}</Tab>
        <Tab value="disk">{tk('km.tab.disk')}</Tab>
        <Tab value="perf">{tk('km.tab.perf')}</Tab>
      </TabList>
      <div hidden={sub !== 'overview'}>
        <OverviewView api={api} notify={notify} onNavigate={navigate} />
      </div>
      {(diskVisited || sub === 'disk') && (
        <div hidden={sub !== 'disk'}>
          <DiskView api={api} notify={notify} />
        </div>
      )}
      {active && sub === 'perf' && <PerfView api={api} notify={notify} />}
    </div>
  );
}
```

- [ ] **Step 4: Chạy test, thấy qua**

Run: `npx vitest run src/features/kham-may` rồi `npm run typecheck` rồi `npm test`
Expected: tab `4 passed`; cả thư mục `src/features/kham-may` `65 passed`; typecheck sạch; `npm test` toàn bộ `0 failed`.

- [ ] **Step 5: Commit**

```bash
rtk git add src/features/kham-may/KhamMayTab.tsx
rtk git add src/features/kham-may/KhamMayTab.test.tsx
rtk git commit -m "feat(kham-may): tab Khám máy — Tổng quan mặc định, giữ trạng thái Ổ đĩa, chỉ lấy mẫu khi đang xem

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 17: `MftScanner` — đọc thẳng MFT

**Files:**
- Modify (thay dòng khung): `crates/winfreeup-diagnose/src/disk/mft.rs`
- Create: `crates/winfreeup-diagnose/testdata/ntfs-testfs1.img`, `testdata/create-testfs1.sh`, `testdata/README.md`
- Test: `mft.rs`

**Interfaces:**
- Consumes: Task 13 (`DiskScanner`, `ScanProgress`, `ScanStatus`, `Throttle`, `walk::is_link_tag`, `WalkScanner` trong test `#[ignore]`); Task 3 (`TreeBuilder`, `DiskTree`, `NodeId`, `FLAG_DIR`, `FLAG_LINK`, `NO_PARENT`, `MAX_CHILDREN`); Task 1 (`util::{filetime_to_unix, wide}`).
- Produces: `BootInfo { cluster_size, record_size, mft_lcn }`, `parse_boot(&[u8]) -> Result<BootInfo>`, `apply_fixups(&mut [u8]) -> bool`, `parse_runlist(&[u8]) -> Vec<(Option<u64>, u64)>`, `RecordInfo`, `parse_record(&[u8]) -> RecordInfo`, `scan_ntfs<R: Read + Seek>(dev, root_name, cancel, progress) -> Result<DiskTree>` (báo `percent`), `MftScanner` (`impl DiskScanner`; không Admin ⇒ lỗi `\\.\C:: Access is denied`).

- [ ] **Step 1: Chép ảnh NTFS thử nghiệm**

Ảnh lấy nguyên từ crate `ntfs` 0.4.0 (MIT/Apache-2.0). Chạy trong PowerShell ở gốc worktree:

```powershell
New-Item -ItemType Directory -Force $env:TEMP\wfu-ntfs | Out-Null
Set-Content $env:TEMP\wfu-ntfs\Cargo.toml "[package]`nname = `"wfu-ntfs-fetch`"`nversion = `"0.0.0`"`nedition = `"2021`"`n[dependencies]`nntfs = `"=0.4.0`"`n[lib]`npath = `"lib.rs`""
Set-Content $env:TEMP\wfu-ntfs\lib.rs ""
cargo fetch --manifest-path $env:TEMP\wfu-ntfs\Cargo.toml
$src = Get-Item "$env:USERPROFILE\.cargo\registry\src\*\ntfs-0.4.0\testdata" | Select-Object -First 1
New-Item -ItemType Directory -Force crates\winfreeup-diagnose\testdata | Out-Null
Copy-Item "$($src.FullName)\testfs1" crates\winfreeup-diagnose\testdata\ntfs-testfs1.img
Copy-Item "$($src.FullName)\create-testfs1.sh" crates\winfreeup-diagnose\testdata\create-testfs1.sh
(Get-Item crates\winfreeup-diagnose\testdata\ntfs-testfs1.img).Length
```

Expected: file 2 097 152 byte. `crates/winfreeup-diagnose/testdata/README.md`:

```markdown
# testdata

`ntfs-testfs1.img` — ảnh ổ NTFS 2 MiB lấy nguyên từ crate [`ntfs` 0.4.0](https://crates.io/crates/ntfs)
(file `testdata/testfs1`, © Colin Finck, giấy phép MIT OR Apache-2.0). `create-testfs1.sh` là script gốc đã
tạo ra ảnh đó (mkntfs, cụm 512 byte): 512 thư mục con, file 1000 byte, file 5 byte nằm trong MFT, file thưa.

Dùng để test `MftScanner` mà không cần quyền Admin (đọc ảnh như đọc một ổ thô).
```

- [ ] **Step 2: Viết test hỏng** — thay dòng khung của `mft.rs` bằng:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::tree::MAX_CHILDREN;

    /// Ảnh NTFS 2 MiB của crate `ntfs` (MIT/Apache-2.0) — nội dung xem `testdata/create-testfs1.sh`.
    fn image() -> File {
        File::open(Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata").join("ntfs-testfs1.img")).unwrap()
    }

    fn scan_image() -> DiskTree {
        scan_ntfs(&mut image(), "X:\\", &CancelToken::new(), &|_: &ScanStatus| {}).unwrap()
    }

    #[test]
    fn boot_sector_geometry_of_the_test_image() {
        let mut b = vec![0u8; 512];
        image().read_exact(&mut b).unwrap();
        assert_eq!(parse_boot(&b).unwrap(), BootInfo { cluster_size: 512, record_size: 1024, mft_lcn: 32 });
        assert!(parse_boot(&[0u8; 512]).is_err());
    }

    #[test]
    fn runlist_decodes_relative_offsets_and_sparse_runs() {
        // 0x21: dài 1 byte, lệch 2 byte. Chạy 1: 0x18 cụm tại LCN 0x5634; chạy 2: 0x10 cụm tại +(-0x10) = 0x5624; chạy 3: thưa 4 cụm.
        let runs = parse_runlist(&[0x21, 0x18, 0x34, 0x56, 0x21, 0x10, 0xF0, 0xFF, 0x01, 0x04, 0x00]);
        assert_eq!(runs, vec![(Some(0x5634), 0x18), (Some(0x5624), 0x10), (None, 4)]);
    }

    #[test]
    fn fixups_restore_sector_tails_and_reject_torn_records() {
        let mut rec = vec![0u8; 1024];
        rec[0..4].copy_from_slice(b"FILE");
        rec[4..6].copy_from_slice(&0x30u16.to_le_bytes());
        rec[6..8].copy_from_slice(&3u16.to_le_bytes());
        rec[0x30..0x36].copy_from_slice(&[0xAB, 0xCD, 0x11, 0x22, 0x33, 0x44]);
        rec[510..512].copy_from_slice(&[0xAB, 0xCD]);
        rec[1022..1024].copy_from_slice(&[0xAB, 0xCD]);
        let mut good = rec.clone();
        assert!(apply_fixups(&mut good));
        assert_eq!(&good[510..512], &[0x11, 0x22]);
        assert_eq!(&good[1022..1024], &[0x33, 0x44]);
        rec[1023] = 0;
        assert!(!apply_fixups(&mut rec));
    }

    #[test]
    fn image_tree_matches_the_script_that_built_it() {
        let t = scan_image();
        let root = t.root();
        let many = t.child_named(root, "many_subdirs").unwrap();
        assert_eq!(t.children(many, 1000).unwrap().items.len(), 512);
        assert!(t.children(many, 1000).unwrap().items.iter().all(|v| v.is_dir));
        let f1000 = t.child_named(root, "1000-bytes-file").unwrap();
        assert_eq!(t.bytes(f1000), 1024, "1000 byte ⇒ 2 cụm 512");
        assert_eq!(t.bytes(t.child_named(root, "file-with-12345").unwrap()), 0, "dữ liệu nằm ngay trong MFT");
        assert_eq!(t.bytes(t.child_named(root, "empty-file").unwrap()), 0);
        // File thưa dài ~500 KB nhưng chỉ có dữ liệu ở đầu và cuối ⇒ chỉ 2 cụm thật sự chiếm.
        assert_eq!(t.bytes(t.child_named(root, "sparse-file").unwrap()), 1024);
        assert_eq!(t.path(f1000), "X:\\1000-bytes-file");
    }

    #[test]
    fn metafiles_show_under_the_root_and_totals_roll_up() {
        let t = scan_image();
        let root = t.root();
        let mft = t.child_named(root, "$MFT").unwrap();
        assert!(t.bytes(mft) > 0);
        let sum: u64 = t.children(root, MAX_CHILDREN).unwrap().items.iter().map(|v| v.bytes).sum::<u64>()
            + t.children(root, MAX_CHILDREN).unwrap().rest.map(|r| r.bytes).unwrap_or(0);
        assert_eq!(t.bytes(root), sum);
    }

    #[test]
    fn progress_reaches_one_hundred_percent() {
        let seen = std::sync::Mutex::new(Vec::new());
        scan_ntfs(&mut image(), "X:\\", &CancelToken::new(), &|s: &ScanStatus| seen.lock().unwrap().push(s.percent)).unwrap();
        assert_eq!(seen.into_inner().unwrap().last().copied().flatten(), Some(100.0));
    }

    #[test]
    fn cancel_and_garbage_are_errors_not_panics() {
        let c = CancelToken::new();
        c.cancel();
        assert!(matches!(scan_ntfs(&mut image(), "X:\\", &c, &|_: &ScanStatus| {}), Err(CoreError::Cancelled)));
        let mut junk = std::io::Cursor::new(vec![0x5Au8; 64 * 1024]);
        assert!(scan_ntfs(&mut junk, "X:\\", &CancelToken::new(), &|_: &ScanStatus| {}).is_err());
    }

    /// Spec mục 7: đối chiếu MftScanner với WalkScanner trên một VHD nhỏ tạo ngay trong test.
    /// Cần Admin (diskpart, mở ổ thô). Chạy: `cargo test -- --ignored mft_matches_walk` trong terminal Admin.
    #[test]
    #[ignore]
    fn mft_matches_walk_on_a_fresh_vhd() {
        use crate::disk::walk::WalkScanner;
        use std::process::Command;
        let t = tempfile::tempdir().unwrap();
        let vhd = t.path().join("wfu-test.vhdx");
        let used = unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() };
        let letter = (b'P'..=b'Z').find(|c| used & (1 << (c - b'A')) == 0).expect("còn ký tự ổ trống") as char;
        let run = |script: String| {
            let f = t.path().join("dp.txt");
            std::fs::write(&f, script).unwrap();
            let out = Command::new("diskpart").arg("/s").arg(&f).output().unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
        };
        let v = vhd.display();
        run(format!(
            "create vdisk file=\"{v}\" maximum=64 type=expandable\nselect vdisk file=\"{v}\"\nattach vdisk\n\
             create partition primary\nformat fs=ntfs quick label=WFUTEST\nassign letter={letter}\n"
        ));
        let root = format!("{letter}:\\");
        let data = Path::new(&root).join("data");
        std::fs::create_dir_all(data.join("sub").join("sâu")).unwrap();
        std::fs::write(data.join("a.bin"), vec![1u8; 300_000]).unwrap();
        std::fs::write(data.join("nho.txt"), b"12345").unwrap();
        std::fs::write(data.join("sub").join("sâu").join("b.bin"), vec![2u8; 70_000]).unwrap();
        std::fs::hard_link(data.join("a.bin"), data.join("sub").join("a-link.bin")).unwrap();
        let cancel = CancelToken::new();
        let mft = MftScanner.scan(Path::new(&root), &cancel, &|_: &ScanStatus| {});
        let walk = WalkScanner::default().scan(Path::new(&root), &cancel, &|_: &ScanStatus| {});
        run(format!("select vdisk file=\"{v}\"\ndetach vdisk\n"));
        let (mft, walk) = (mft.unwrap(), walk.unwrap());
        let (dm, dw) = (mft.child_named(mft.root(), "data").unwrap(), walk.child_named(walk.root(), "data").unwrap());
        assert_eq!(mft.bytes(dm), walk.bytes(dw), "tổng byte");
        assert_eq!(mft.files(dm), walk.files(dw), "số file");
        assert_eq!(mft.files(dm), 3, "hard link chỉ đếm một lần");
    }

    #[test]
    fn without_admin_rights_opening_the_volume_fails_cleanly() {
        // Chạy dưới Admin thì mở được và quét thật; không Admin thì phải là lỗi, không panic.
        let sys_drive = format!("{}\\", std::env::var("SystemDrive").unwrap());
        match open_volume(Path::new(&sys_drive)) {
            Ok(_) => {}
            Err(e) => assert!(e.to_string().contains(r"\\.\"), "{e}"),
        }
        assert!(open_volume(Path::new(r"C:\Windows")).is_err());
    }
}
```

- [ ] **Step 3: Chạy test, thấy hỏng**

Run: `cargo test disk::mft`
Expected: FAIL biên dịch — `cannot find function scan_ntfs`.

- [ ] **Step 4: Viết mã (trên khối test)**

```rust
//! MftScanner: đọc thẳng bảng MFT của ổ NTFS cục bộ (cần Admin) — nhanh hơn duyệt thư mục nhiều lần.
//! Đọc tuần tự từng khúc lớn của `$MFT`, mỗi bản ghi lấy: tên + thư mục cha ($FILE_NAME, bỏ tên DOS 8.3),
//! dung lượng cấp phát của luồng dữ liệu chính ($DATA không tên), ngày sửa ($STANDARD_INFORMATION),
//! reparse tag. Một bản ghi nhiều $FILE_NAME = hard link ⇒ dung lượng chỉ tính cho tên đầu tiên.
//! Chỉ đếm luồng dữ liệu chính, giống WalkScanner, để hai bộ quét cho cùng một con số.
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::io::FromRawHandle;
use std::path::Path;

use windows_sys::Win32::Foundation::{GENERIC_READ, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
use winfreeup_core::{CancelToken, CoreError, Result};

use super::scan::{DiskScanner, ScanProgress, ScanStatus, Throttle};
use super::tree::{DiskTree, NodeId, TreeBuilder, FLAG_DIR, FLAG_LINK, NO_PARENT};
use super::walk::is_link_tag;
use crate::util::{filetime_to_unix, wide};

const ROOT_RECORD: usize = 5;
const ATTR_STANDARD_INFORMATION: u32 = 0x10;
const ATTR_ATTRIBUTE_LIST: u32 = 0x20;
const ATTR_FILE_NAME: u32 = 0x30;
const ATTR_DATA: u32 = 0x80;
const ATTR_REPARSE_POINT: u32 = 0xC0;
const ATTR_END: u32 = 0xFFFF_FFFF;
const NAMESPACE_DOS: u8 = 2;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
/// Mỗi lần đọc 4 MiB của $MFT.
const CHUNK: u64 = 4 << 20;

fn sys(msg: impl Into<String>) -> CoreError {
    CoreError::System(msg.into())
}

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}
fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap()))
}
fn u64_at(b: &[u8], o: usize) -> Option<u64> {
    b.get(o..o + 8).map(|s| u64::from_le_bytes(s.try_into().unwrap()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootInfo {
    pub cluster_size: u64,
    pub record_size: u64,
    pub mft_lcn: u64,
}

/// Đọc boot sector NTFS (512 byte đầu của ổ).
pub fn parse_boot(b: &[u8]) -> Result<BootInfo> {
    if b.get(3..11) != Some(b"NTFS    ") {
        return Err(sys("not an NTFS boot sector"));
    }
    let bytes_per_sector = u64::from(u16_at(b, 0x0B).unwrap_or(0));
    let spc = b[0x0D];
    // Cụm rất lớn (> 64 KiB) được ghi dạng 2^(256 - giá trị).
    let sectors_per_cluster = if spc > 0x80 { 1u64 << (256 - u32::from(spc)) } else { u64::from(spc) };
    let cluster_size = bytes_per_sector * sectors_per_cluster;
    let cpr = b[0x40] as i8;
    let record_size = if cpr < 0 { 1u64 << (-i32::from(cpr)) } else { cpr as u64 * cluster_size };
    let mft_lcn = u64_at(b, 0x30).unwrap_or(0);
    if cluster_size == 0 || !(256..=65536).contains(&record_size) || mft_lcn == 0 {
        return Err(sys(format!("bad NTFS geometry: cluster={cluster_size} record={record_size} mft_lcn={mft_lcn}")));
    }
    Ok(BootInfo { cluster_size, record_size, mft_lcn })
}

/// Khôi phục 2 byte cuối mỗi đoạn 512 byte từ mảng update sequence. Sai chữ ký/khớp ⇒ `false` (bỏ bản ghi).
pub fn apply_fixups(rec: &mut [u8]) -> bool {
    if rec.get(0..4) != Some(b"FILE") {
        return false;
    }
    let (Some(usa_off), Some(usa_count)) = (u16_at(rec, 4), u16_at(rec, 6)) else { return false };
    let (usa_off, usa_count) = (usa_off as usize, usa_count as usize);
    if usa_count < 2 || usa_off + usa_count * 2 > rec.len() {
        return false;
    }
    let stride = rec.len() / (usa_count - 1);
    let check = [rec[usa_off], rec[usa_off + 1]];
    for i in 1..usa_count {
        let end = i * stride;
        if end > rec.len() || rec[end - 2..end] != check {
            return false;
        }
        rec[end - 2] = rec[usa_off + i * 2];
        rec[end - 1] = rec[usa_off + i * 2 + 1];
    }
    true
}

/// Giải mã runlist: danh sách (LCN, số cụm). LCN `None` là đoạn thưa (không chiếm đĩa).
pub fn parse_runlist(b: &[u8]) -> Vec<(Option<u64>, u64)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut lcn: i64 = 0;
    while let Some(&head) = b.get(i) {
        if head == 0 {
            break;
        }
        let (len_size, off_size) = ((head & 0x0F) as usize, (head >> 4) as usize);
        if len_size == 0 || len_size > 8 || off_size > 8 || i + 1 + len_size + off_size > b.len() {
            break;
        }
        let mut len: u64 = 0;
        for k in 0..len_size {
            len |= u64::from(b[i + 1 + k]) << (8 * k);
        }
        if off_size == 0 {
            out.push((None, len));
        } else {
            let mut delta: i64 = 0;
            for k in 0..off_size {
                delta |= i64::from(b[i + 1 + len_size + k]) << (8 * k);
            }
            // Mở rộng dấu.
            let shift = 64 - 8 * off_size as u32;
            delta = (delta << shift) >> shift;
            lcn += delta;
            out.push((Some(lcn as u64), len));
        }
        i += 1 + len_size + off_size;
    }
    out
}

/// Những gì một bản ghi MFT (gốc hoặc mở rộng) đóng góp.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RecordInfo {
    pub in_use: bool,
    pub is_dir: bool,
    /// Số bản ghi gốc nếu đây là bản ghi mở rộng; 0 nếu chính nó là gốc.
    pub base: u64,
    pub seq: u16,
    /// (tham chiếu cha 64 bit, tên) — đã bỏ tên thuần DOS.
    pub names: Vec<(u64, String)>,
    /// Byte cấp phát của $DATA không tên (chỉ ở phần mở đầu VCN 0).
    pub alloc: Option<u64>,
    pub modified: Option<u32>,
    pub is_link: bool,
    pub has_attribute_list: bool,
    /// Runlist của $DATA không tên (dùng cho chính bản ghi $MFT).
    pub data_runs: Vec<(Option<u64>, u64)>,
    pub data_size: u64,
}

/// Đọc một bản ghi đã áp fixup.
pub fn parse_record(rec: &[u8]) -> RecordInfo {
    let mut info = RecordInfo::default();
    let flags = u16_at(rec, 0x16).unwrap_or(0);
    info.in_use = flags & 1 != 0;
    info.is_dir = flags & 2 != 0;
    info.seq = u16_at(rec, 0x10).unwrap_or(0);
    info.base = u64_at(rec, 0x20).unwrap_or(0) & 0x0000_FFFF_FFFF_FFFF;
    let mut off = u16_at(rec, 0x14).unwrap_or(0) as usize;
    let mut si_attrs = 0u32;
    let mut reparse_tag = 0u32;
    while let (Some(ty), Some(len)) = (u32_at(rec, off), u32_at(rec, off + 4)) {
        if ty == ATTR_END || len < 16 || off + len as usize > rec.len() {
            break;
        }
        let a = &rec[off..off + len as usize];
        let non_resident = a[8] != 0;
        let name_len = a[9];
        let resident_value = || -> Option<&[u8]> {
            let vlen = u32_at(a, 0x10)? as usize;
            let voff = u16_at(a, 0x14)? as usize;
            a.get(voff..voff + vlen)
        };
        match ty {
            ATTR_STANDARD_INFORMATION if !non_resident => {
                if let Some(v) = resident_value() {
                    info.modified = u64_at(v, 0x08).map(|t| filetime_to_unix(t as i64));
                    si_attrs = u32_at(v, 0x20).unwrap_or(0);
                }
            }
            ATTR_ATTRIBUTE_LIST => info.has_attribute_list = true,
            ATTR_FILE_NAME if !non_resident => {
                if let Some(v) = resident_value() {
                    let parent = u64_at(v, 0).unwrap_or(0);
                    let n = v.get(0x40).copied().unwrap_or(0) as usize;
                    let ns = v.get(0x41).copied().unwrap_or(0);
                    if ns != NAMESPACE_DOS {
                        if let Some(raw) = v.get(0x42..0x42 + n * 2) {
                            let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                            info.names.push((parent, String::from_utf16_lossy(&units)));
                        }
                    }
                }
            }
            ATTR_DATA if name_len == 0 => {
                if !non_resident {
                    info.alloc = Some(0);
                } else if u64_at(a, 0x10) == Some(0) {
                    let attr_flags = u16_at(a, 0x0C).unwrap_or(0);
                    // Nén (0x0001) hoặc thưa (0x8000): trường «compressed size» mới là số cụm thật sự chiếm.
                    let alloc = if attr_flags & 0x8001 != 0 { u64_at(a, 0x40) } else { u64_at(a, 0x28) };
                    info.alloc = alloc;
                    info.data_size = u64_at(a, 0x30).unwrap_or(0);
                    let run_off = u16_at(a, 0x20).unwrap_or(0) as usize;
                    info.data_runs = a.get(run_off..).map(parse_runlist).unwrap_or_default();
                } else {
                    // Phần nối tiếp (VCN > 0) của runlist $DATA nằm ở bản ghi mở rộng.
                    let run_off = u16_at(a, 0x20).unwrap_or(0) as usize;
                    info.data_runs = a.get(run_off..).map(parse_runlist).unwrap_or_default();
                }
            }
            ATTR_REPARSE_POINT if !non_resident => {
                reparse_tag = resident_value().and_then(|v| u32_at(v, 0)).unwrap_or(0);
            }
            _ => {}
        }
        off += len as usize;
    }
    info.is_link = is_link_tag(si_attrs | if reparse_tag != 0 { FILE_ATTRIBUTE_REPARSE_POINT } else { 0 }, reparse_tag);
    info
}

/// Gom kết quả mọi bản ghi, theo số bản ghi gốc.
struct Collected {
    seq: Vec<u16>,
    /// 0 = không dùng, 1 = file, 2 = thư mục.
    kind: Vec<u8>,
    alloc: Vec<u64>,
    modified: Vec<u32>,
    link: Vec<bool>,
    names: Vec<Vec<(u64, Box<str>)>>,
}

impl Collected {
    fn new(n: usize) -> Self {
        Collected { seq: vec![0; n], kind: vec![0; n], alloc: vec![0; n], modified: vec![0; n], link: vec![false; n], names: vec![Vec::new(); n] }
    }

    fn add(&mut self, recno: usize, r: RecordInfo) {
        if !r.in_use {
            return;
        }
        let base = if r.base == 0 { recno } else { r.base as usize };
        if base >= self.kind.len() {
            return;
        }
        if r.base == 0 {
            self.seq[base] = r.seq;
            self.kind[base] = if r.is_dir { 2 } else { 1 };
            self.link[base] = r.is_link;
            if let Some(m) = r.modified {
                self.modified[base] = m;
            }
        }
        if let Some(a) = r.alloc {
            self.alloc[base] = a;
        }
        self.names[base].extend(r.names.into_iter().map(|(p, n)| (p, n.into_boxed_str())));
    }

    fn into_tree(self, root_name: &str) -> Result<DiskTree> {
        let n = self.kind.len();
        if n <= ROOT_RECORD || self.kind[ROOT_RECORD] != 2 {
            return Err(sys("MFT has no root directory record"));
        }
        let mut b = TreeBuilder::with_capacity(n);
        let mut node_of = vec![NO_PARENT; n];
        let root = b.add(NO_PARENT, root_name, FLAG_DIR, 0, self.modified[ROOT_RECORD]);
        node_of[ROOT_RECORD] = root;
        let mut extra: Vec<(NodeId, u64)> = Vec::new();
        let mut primary: Vec<(NodeId, u64)> = Vec::new();
        for (rec, names) in self.names.iter().enumerate() {
            if rec == ROOT_RECORD || self.kind[rec] == 0 || names.is_empty() {
                continue;
            }
            let is_dir = self.kind[rec] == 2;
            let mut flags = if is_dir { FLAG_DIR } else { 0 };
            if self.link[rec] {
                flags |= FLAG_LINK;
            }
            let bytes = if is_dir || self.link[rec] { 0 } else { self.alloc[rec] };
            let (parent, name) = &names[0];
            let id = b.add(NO_PARENT, name, flags, bytes, self.modified[rec]);
            node_of[rec] = id;
            primary.push((id, *parent));
            for (p, other) in names.iter().skip(1) {
                let link = b.add(NO_PARENT, other, flags | FLAG_LINK, 0, self.modified[rec]);
                extra.push((link, *p));
            }
        }
        for (id, pref) in primary.into_iter().chain(extra) {
            let prec = (pref & 0x0000_FFFF_FFFF_FFFF) as usize;
            let pseq = (pref >> 48) as u16;
            if prec < n && self.kind[prec] == 2 && (pseq == 0 || pseq == self.seq[prec]) && node_of[prec] != NO_PARENT {
                b.set_parent(id, node_of[prec]);
            }
        }
        Ok(b.finish(root))
    }
}

/// Quét toàn bộ MFT từ một nguồn đọc được (ổ thật `\\.\C:` hoặc file ảnh trong test).
pub fn scan_ntfs<R: Read + Seek>(dev: &mut R, root_name: &str, cancel: &CancelToken, progress: &dyn ScanProgress) -> Result<DiskTree> {
    let io = |e: std::io::Error| sys(format!("{root_name}: {e}"));
    let mut boot = vec![0u8; 4096];
    dev.seek(SeekFrom::Start(0)).map_err(io)?;
    dev.read_exact(&mut boot[..512]).map_err(io)?;
    let geo = parse_boot(&boot)?;
    let rs = geo.record_size as usize;
    // Bản ghi 0 là chính $MFT: runlist của nó cho biết MFT nằm ở đâu.
    let mut rec0 = vec![0u8; geo.cluster_size.max(geo.record_size) as usize];
    dev.seek(SeekFrom::Start(geo.mft_lcn * geo.cluster_size)).map_err(io)?;
    dev.read_exact(&mut rec0).map_err(io)?;
    let mut rec0 = rec0[..rs].to_vec();
    if !apply_fixups(&mut rec0) {
        return Err(sys("MFT record 0 is damaged"));
    }
    let mft = parse_record(&rec0);
    let run_bytes: u64 = mft.data_runs.iter().map(|(_, len)| len * geo.cluster_size).sum();
    if mft.data_runs.is_empty() || (mft.has_attribute_list && run_bytes < mft.data_size) {
        return Err(sys("$MFT is split across extension records (attribute list) — not supported"));
    }
    let total_records = (mft.data_size / geo.record_size) as usize;
    let mut col = Collected::new(total_records);
    let throttle = Throttle::new(std::time::Duration::from_millis(100));
    let (mut files, mut bytes) = (0u64, 0u64);
    let mut recno = 0usize;
    // Bản ghi có thể nằm vắt qua hai đoạn (cụm nhỏ hơn bản ghi): phần dư của khúc trước ghép vào khúc sau.
    let mut buf: Vec<u8> = Vec::new();
    for (lcn, len) in &mft.data_runs {
        let mut pos = lcn.map(|l| l * geo.cluster_size);
        let mut left = len * geo.cluster_size;
        while left > 0 && recno < total_records {
            if cancel.is_cancelled() {
                return Err(CoreError::Cancelled);
            }
            let want = CHUNK.min(left) as usize;
            let start = buf.len();
            buf.resize(start + want, 0);
            if let Some(p) = pos {
                dev.seek(SeekFrom::Start(p)).map_err(io)?;
                dev.read_exact(&mut buf[start..]).map_err(io)?;
                pos = Some(p + want as u64);
            }
            left -= want as u64;
            let whole = buf.len() / rs * rs;
            for rec in buf[..whole].chunks_exact_mut(rs) {
                if recno >= total_records {
                    break;
                }
                if apply_fixups(rec) {
                    let info = parse_record(rec);
                    if info.in_use && info.base == 0 && !info.is_dir {
                        files += 1;
                        bytes += info.alloc.unwrap_or(0);
                    }
                    col.add(recno, info);
                }
                recno += 1;
            }
            buf.drain(..whole);
            if throttle.ready() {
                progress.report(&ScanStatus {
                    files,
                    bytes,
                    current: root_name.to_string(),
                    percent: Some(recno as f32 * 100.0 / total_records.max(1) as f32),
                });
            }
        }
    }
    progress.report(&ScanStatus { files, bytes, current: root_name.to_string(), percent: Some(100.0) });
    col.into_tree(root_name)
}

/// Mở ổ thô `\\.\C:` (cần Admin; không có quyền ⇒ lỗi «Access is denied», bộ chọn lùi về WalkScanner).
fn open_volume(root: &Path) -> Result<File> {
    let s = root.to_string_lossy();
    let letter = s.trim_end_matches('\\');
    if letter.len() != 2 || !letter.ends_with(':') {
        return Err(sys(format!("not a drive root: {s}")));
    }
    let w = wide(format!(r"\\.\{letter}"));
    let h = unsafe {
        CreateFileW(w.as_ptr(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut())
    };
    if h == INVALID_HANDLE_VALUE {
        return Err(sys(format!(r"\\.\{letter}: {}", std::io::Error::last_os_error())));
    }
    Ok(unsafe { File::from_raw_handle(h) })
}

pub struct MftScanner;

impl DiskScanner for MftScanner {
    fn scan(&self, root: &Path, cancel: &CancelToken, progress: &dyn ScanProgress) -> Result<DiskTree> {
        let mut dev = open_volume(root)?;
        scan_ntfs(&mut dev, &root.display().to_string(), cancel, progress)
    }
}
```

- [ ] **Step 5: Chạy test và clippy**

Run: `cargo test disk::mft`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
Expected: `8 passed; 1 ignored`; toàn bộ `0 failed`. Nếu test ảnh chỉ thấy 500/512 thư mục thì bản ghi vắt qua hai đoạn `$MFT` đang bị bỏ (đoạn đầu của ảnh dài 511 cụm 512 byte) — xem Review Focus 2.

- [ ] **Step 6: Đối chiếu với WalkScanner trên VHD (cần Admin)**

Trong terminal **Run as administrator**, ở `crates/winfreeup-diagnose`: `cargo test -- --ignored mft_matches_walk`
Expected: `1 passed` — tổng byte và số file của thư mục `data` khớp giữa hai bộ quét, hard link đếm một lần. Test tự tạo/gỡ VHD 64 MB bằng `diskpart` trong thư mục tạm. Không có quyền Admin thì ghi «chưa chạy — cần Admin» vào báo cáo task; Task 21 cho CI chạy test này (runner GitHub có quyền Admin).

- [ ] **Step 7: Commit**

```bash
rtk git add crates/winfreeup-diagnose/src/disk/mft.rs
rtk git add crates/winfreeup-diagnose/testdata/ntfs-testfs1.img
rtk git add crates/winfreeup-diagnose/testdata/create-testfs1.sh
rtk git add crates/winfreeup-diagnose/testdata/README.md
rtk git commit -m "feat(diagnose): MftScanner — đọc MFT tuần tự, fixup, runlist, hard link, file thưa; test trên ảnh NTFS

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 18: Thu số liệu khám nhanh và dịch vụ Tổng quan

**Files:**
- Modify (thay dòng khung): `crates/winfreeup-diagnose/src/health/collect.rs`, `health/service.rs`
- Test: hai file trên

**Interfaces:**
- Consumes: Task 15 (mọi luật, `evaluate`, `HealthMetrics`, `Finding`, `DiskHealth`, `MediaType`, `PowerState`, `Measured`); Task 9 (`Startup`, `StartupEntry`, `StartupList`, `Startup::{from_env, enabled_count, list, set_enabled}`); Task 8 (`memory`, `CpuDiskCounters`, `ThermalReader`, `wmiq::{query, NS_STORAGE, NS_WMI}`); Task 6 (`Point`); Task 1 (`util::wide`, `ActionLog` trong test).
- Produces:
  - `collect::system_drive()`, `system_free()`, `disk_health()` (Storage WMI + SMART bổ sung), `system_media()` (IOCTL seek-penalty — không qua WMI, vài phần mười ms), `uptime_secs()`, `power()`, `cpu_temp()`, `throttle_points(seconds) -> Measured<Vec<Point>>`, `collect_streaming(startup: &Startup, on_finding: &(dyn Fn(Finding) + Sync)) -> HealthMetrics`, `POWER_SAVER_SCHEME`.
  - `service::settings_uri(page) -> Option<&'static str>` (danh sách trắng `power`, `battery`, `startup`, `storage`), `HealthService::new(startup)`, `check(&self, on_finding) -> Result<Vec<Finding>, String>` (`busy` khi đang khám), `throttle() -> Finding`, `startup_list()`, `set_startup(id, enabled)`, `open_settings(page)`.

- [ ] **Step 1: Viết test hỏng** — thay dòng khung của hai file bằng phần test:

`collect.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::actlog::ActionLog;
    use std::sync::Arc;

    #[test]
    fn power_saver_guid_constant_matches_windows() {
        let g = windows_sys::core::GUID::from_u128(0xa1841308_3541_4fab_bc81_f71556f20b4a);
        assert_eq!(guid_u128(&g), POWER_SAVER_SCHEME);
    }

    #[test]
    fn cheap_sources_read_on_this_machine() {
        let (free, total) = system_free().unwrap();
        assert!(total > 0 && free <= total);
        assert!(uptime_secs().unwrap() > 0);
        power().unwrap();
    }

    #[test]
    fn system_media_comes_from_the_seek_penalty_ioctl() {
        // Máy dev là NVMe ⇒ Ssd; máy ảo có thể trả Ssd/Hdd/Unspecified — không được là lỗi.
        system_media().unwrap();
    }

    #[test]
    fn disk_health_reports_something_or_a_message() {
        match disk_health() {
            Ok(v) => assert!(!v.is_empty()),
            Err(e) => assert!(!e.is_empty()),
        }
    }

    #[test]
    fn streaming_reports_each_row_once_and_returns_every_source() {
        let log = Arc::new(ActionLog::new(std::env::temp_dir().join("wfu-khong-ghi")));
        let st = Startup::from_env(log).unwrap();
        let seen = std::sync::Mutex::new(Vec::new());
        let m = collect_streaming(&st, &|f: Finding| seen.lock().unwrap().push(f.id));
        let mut ids = seen.into_inner().unwrap();
        ids.sort();
        assert_eq!(ids, vec!["cpu_hot", "disk_full", "disk_health", "power_saver", "ram_pressure", "startup_apps", "system_hdd", "uptime"]);
        assert!(m.system_free.is_ok() && m.memory.is_ok() && m.startup_enabled.is_ok());
        assert_eq!(rules::evaluate(&m).len(), 8);
    }

    #[test]
    fn throttle_sampling_takes_one_point_per_second() {
        let p = throttle_points(2).unwrap();
        assert_eq!(p.len(), 2);
        assert!(p[1].dur_ms >= 900);
    }
}
```

`service.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::actlog::ActionLog;
    use std::sync::Arc;

    #[test]
    fn only_whitelisted_settings_pages_open() {
        assert_eq!(settings_uri("power"), Some("ms-settings:powersleep"));
        assert_eq!(settings_uri("startup"), Some("ms-settings:startupapps"));
        assert_eq!(settings_uri("ms-settings:windowsupdate"), None);
        assert_eq!(settings_uri("cmd.exe"), None);
    }

    #[test]
    fn a_second_check_while_one_runs_is_refused() {
        let log = Arc::new(ActionLog::new(std::env::temp_dir().join("wfu-khong-ghi")));
        let svc = HealthService::new(Startup::from_env(log).unwrap());
        svc.busy.store(true, Ordering::SeqCst);
        assert_eq!(svc.check(&|_| {}).unwrap_err(), "busy");
        svc.busy.store(false, Ordering::SeqCst);
        assert_eq!(svc.check(&|_| {}).unwrap().len(), 8);
        assert!(!svc.busy.load(Ordering::SeqCst));
        assert!(svc.open_settings("khong-co").is_err());
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test health::`
Expected: FAIL biên dịch — `cannot find function system_free`, `cannot find type HealthService`.

- [ ] **Step 3: Viết mã (trên khối test)**

`collect.rs`:

```rust
//! Thu số liệu thật cho khám nhanh. Mỗi nguồn độc lập: hỏng nguồn nào thì chỉ dòng đó «Không đo được».
use std::os::windows::io::{AsRawHandle, FromRawHandle};

use serde::Deserialize;
use windows_sys::Win32::Foundation::{LocalFree, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, GetDiskFreeSpaceExW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
use windows_sys::Win32::System::Ioctl::{
    PropertyStandardQuery, StorageDeviceSeekPenaltyProperty, DEVICE_SEEK_PENALTY_DESCRIPTOR, IOCTL_STORAGE_QUERY_PROPERTY,
    STORAGE_PROPERTY_QUERY, VOLUME_DISK_EXTENTS,
};
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, PowerGetActiveScheme, SYSTEM_POWER_STATUS};
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::System::IO::DeviceIoControl;

use super::rules::{self, DiskHealth, Finding, HealthMetrics, Measured, MediaType, PowerState};
use super::startup::Startup;
use crate::perf::counters::{memory, CpuDiskCounters};
use crate::perf::detect::Point;
use crate::perf::thermal::ThermalReader;
use crate::util::wide;
use crate::wmiq;

/// Gói nguồn «Power saver» (GUID_MAX_POWER_SAVINGS = {a1841308-3541-4fab-bc81-f71556f20b4a}).
pub const POWER_SAVER_SCHEME: u128 = 0xa1841308_3541_4fab_bc81_f71556f20b4a;
/// CTL_CODE(IOCTL_VOLUME_BASE 0x56, 0, METHOD_BUFFERED, FILE_ANY_ACCESS) — windows-sys chưa có hằng này.
const IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS: u32 = 0x0056_0000;

pub fn system_drive() -> Result<String, String> {
    std::env::var("SystemDrive").map(|d| format!(r"{d}\")).map_err(|_| "environment variable SystemDrive is not set".to_string())
}

pub fn system_free() -> Measured<(u64, u64)> {
    let root = system_drive()?;
    let w = wide(&root);
    let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
    if unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, &mut total, &mut free) } == 0 {
        return Err(format!("{root}: {}", std::io::Error::last_os_error()));
    }
    Ok((free, total))
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PhysicalDisk {
    friendly_name: String,
    health_status: u16,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct FailurePredict {
    instance_name: String,
    predict_failure: bool,
}

/// Sức khỏe mọi ổ vật lý (`MSFT_PhysicalDisk.HealthStatus`). SMART (cần Admin, nhiều ổ NVMe không có) chỉ
/// bổ sung: đọc không được thì bỏ qua, không làm cả dòng thành «Không đo được».
/// Storage WMI có thể rất chậm (đo trên máy dev đang tải nặng: 22–56 giây) ⇒ dòng này thường về sau cùng.
pub fn disk_health() -> Measured<Vec<DiskHealth>> {
    let physical: Vec<PhysicalDisk> = wmiq::query(wmiq::NS_STORAGE, "SELECT FriendlyName, HealthStatus FROM MSFT_PhysicalDisk")?;
    let mut health: Vec<DiskHealth> =
        physical.into_iter().map(|d| DiskHealth { name: d.friendly_name, health_status: d.health_status, predict_failure: None }).collect();
    if let Ok(smart) = wmiq::query::<FailurePredict>(wmiq::NS_WMI, "SELECT InstanceName, PredictFailure FROM MSStorageDriver_FailurePredictStatus") {
        for s in smart.into_iter().filter(|s| s.predict_failure) {
            health.push(DiskHealth { name: s.instance_name, health_status: 0, predict_failure: Some(true) });
        }
    }
    Ok(health)
}

fn open_device(path: &str) -> Result<std::fs::File, String> {
    let w = wide(path);
    // Quyền 0: chỉ hỏi thông tin thiết bị, không cần Admin.
    let h = unsafe { CreateFileW(w.as_ptr(), 0, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut()) };
    if h == INVALID_HANDLE_VALUE {
        return Err(format!("{path}: {}", std::io::Error::last_os_error()));
    }
    Ok(unsafe { std::fs::File::from_raw_handle(h) })
}

fn ioctl<I, O: Default>(dev: &std::fs::File, code: u32, input: Option<&I>) -> Result<O, String> {
    let mut out = O::default();
    let mut ret = 0u32;
    let (ip, il) = match input {
        Some(i) => ((i as *const I).cast(), std::mem::size_of::<I>() as u32),
        None => (std::ptr::null(), 0),
    };
    let ok = unsafe {
        DeviceIoControl(dev.as_raw_handle(), code, ip, il, (&mut out as *mut O).cast(), std::mem::size_of::<O>() as u32, &mut ret, std::ptr::null_mut())
    };
    if ok == 0 {
        return Err(format!("DeviceIoControl 0x{code:08X}: {}", std::io::Error::last_os_error()));
    }
    Ok(out)
}

/// Loại ổ chứa Windows: hỏi thẳng ổ đĩa có «mất thời gian dời đầu đọc» (seek penalty) không — cách
/// Windows tự phân biệt HDD/SSD khi tối ưu ổ. Nhanh (vài ms), không cần Admin, không qua WMI.
pub fn system_media() -> Measured<MediaType> {
    let root = system_drive()?;
    let vol = open_device(&format!(r"\\.\{}", root.trim_end_matches('\\')))?;
    let ext: VOLUME_DISK_EXTENTS = ioctl::<(), _>(&vol, IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS, None)?;
    if ext.NumberOfDiskExtents == 0 {
        return Err("system volume has no disk extents".into());
    }
    let disk = open_device(&format!(r"\\.\PhysicalDrive{}", ext.Extents[0].DiskNumber))?;
    let q = STORAGE_PROPERTY_QUERY { PropertyId: StorageDeviceSeekPenaltyProperty, QueryType: PropertyStandardQuery, AdditionalParameters: [0] };
    let d: DEVICE_SEEK_PENALTY_DESCRIPTOR = ioctl(&disk, IOCTL_STORAGE_QUERY_PROPERTY, Some(&q))?;
    if d.Size == 0 {
        return Ok(MediaType::Unspecified);
    }
    Ok(if d.IncursSeekPenalty { MediaType::Hdd } else { MediaType::Ssd })
}

/// Giây từ lần khởi động thật gần nhất. Fast Startup: «Shut down» chỉ ngủ đông nhân Windows nên
/// đồng hồ này KHÔNG về 0 — đúng điều spec muốn giải thích cho người dùng.
pub fn uptime_secs() -> Measured<u64> {
    Ok(unsafe { GetTickCount64() } / 1000)
}

fn guid_u128(g: &windows_sys::core::GUID) -> u128 {
    (u128::from(g.data1) << 96) | (u128::from(g.data2) << 80) | (u128::from(g.data3) << 64) | u128::from(u64::from_be_bytes(g.data4))
}

pub fn power() -> Measured<PowerState> {
    let mut s = SYSTEM_POWER_STATUS { ACLineStatus: 0, BatteryFlag: 0, BatteryLifePercent: 0, SystemStatusFlag: 0, BatteryLifeTime: 0, BatteryFullLifeTime: 0 };
    if unsafe { GetSystemPowerStatus(&mut s) } == 0 {
        return Err(format!("GetSystemPowerStatus: {}", std::io::Error::last_os_error()));
    }
    let mut guid: *mut windows_sys::core::GUID = std::ptr::null_mut();
    let rc = unsafe { PowerGetActiveScheme(std::ptr::null_mut(), &mut guid) };
    let saver_scheme = if rc == 0 && !guid.is_null() {
        let v = guid_u128(unsafe { &*guid });
        unsafe { LocalFree(guid.cast()) };
        v == POWER_SAVER_SCHEME
    } else {
        false
    };
    // ACLineStatus: 0 pin, 1 cắm điện, 255 không rõ (máy bàn) — coi như cắm điện.
    Ok(PowerState { on_ac: s.ACLineStatus != 0, saver: s.SystemStatusFlag == 1 || saver_scheme })
}

pub fn cpu_temp() -> Measured<f32> {
    ThermalReader::new().read()
}

/// Spec 4 `cpu_throttle`: lấy mẫu `seconds` giây (1 mẫu/giây) tải CPU và % hiệu năng.
pub fn throttle_points(seconds: u32) -> Measured<Vec<Point>> {
    let c = CpuDiskCounters::open().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let mut last = std::time::Instant::now();
    for i in 0..seconds {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let r = c.read().map_err(|e| e.to_string())?;
        let dur = last.elapsed().as_millis() as u64;
        last = std::time::Instant::now();
        out.push(Point { t_ms: u64::from(i + 1) * 1000, dur_ms: dur, cpu: r.cpu.unwrap_or(0.0), disk: 0.0, perf: r.perf });
    }
    Ok(out)
}

/// Thu mọi số liệu; mỗi nguồn xong là báo ngay dòng kết luận của nó (`on_finding`) để giao diện hiện dần
/// thay vì chờ nguồn chậm nhất (Storage WMI, WMI nhiệt độ chạy trên luồng riêng). Trả đủ số liệu cho `evaluate`.
pub fn collect_streaming(startup: &Startup, on_finding: &(dyn Fn(Finding) + Sync)) -> HealthMetrics {
    fn join<T>(h: std::thread::ScopedJoinHandle<'_, Measured<T>>) -> Measured<T> {
        h.join().unwrap_or_else(|e| Err(format!("panic: {:?}", e.downcast_ref::<&str>())))
    }
    std::thread::scope(|s| {
        let disks = s.spawn(|| {
            let m = disk_health();
            on_finding(rules::disk_health(&m));
            m
        });
        let temp = s.spawn(|| {
            let m = cpu_temp();
            on_finding(rules::cpu_hot(&m));
            m
        });
        let system_free = system_free();
        on_finding(rules::disk_full(&system_free));
        let system_media = system_media();
        on_finding(rules::system_hdd(&system_media));
        let memory = memory().map_err(|e| e.to_string());
        on_finding(rules::ram_pressure(&memory));
        let startup_enabled = startup.enabled_count();
        on_finding(rules::startup_apps(&startup_enabled));
        let uptime_secs = uptime_secs();
        on_finding(rules::uptime(&uptime_secs));
        let power = power();
        on_finding(rules::power_saver(&power));
        HealthMetrics { system_free, disks: join(disks), system_media, memory, startup_enabled, uptime_secs, power, cpu_temp_c: join(temp) }
    })
}
```

`service.rs`:

```rust
//! Tầng dịch vụ của mục Tổng quan: khám nhanh (dòng nào xong báo dòng đó), đo hạ xung 10 giây,
//! danh sách khởi động, mở trang Cài đặt Windows theo danh sách trắng.
use std::sync::atomic::{AtomicBool, Ordering};

use super::collect::{collect_streaming, throttle_points};
use super::rules::{self, evaluate, Finding};
use super::startup::{Startup, StartupEntry, StartupList};

/// Trang Cài đặt được phép mở — mã giao diện gửi ⇒ URI `ms-settings:`. Mã lạ bị từ chối.
pub fn settings_uri(page: &str) -> Option<&'static str> {
    match page {
        "power" => Some("ms-settings:powersleep"),
        "battery" => Some("ms-settings:batterysaver"),
        "startup" => Some("ms-settings:startupapps"),
        "storage" => Some("ms-settings:storagesense"),
        _ => None,
    }
}

pub struct HealthService {
    startup: Startup,
    busy: AtomicBool,
}

impl HealthService {
    pub fn new(startup: Startup) -> Self {
        HealthService { startup, busy: AtomicBool::new(false) }
    }

    /// Khám nhanh. `on_finding` nhận từng dòng ngay khi nguồn của nó xong; kết quả cuối là đủ 8 dòng
    /// theo thứ tự bảng spec. Đang khám mà gọi lại ⇒ `Err("busy")`.
    pub fn check(&self, on_finding: &(dyn Fn(Finding) + Sync)) -> Result<Vec<Finding>, String> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err("busy".into());
        }
        let m = collect_streaming(&self.startup, on_finding);
        self.busy.store(false, Ordering::SeqCst);
        Ok(evaluate(&m))
    }

    /// Dòng `cpu_throttle`: lấy mẫu 10 giây.
    pub fn throttle(&self) -> Finding {
        rules::cpu_throttle(&throttle_points(10))
    }

    pub fn startup_list(&self) -> StartupList {
        self.startup.list()
    }

    pub fn set_startup(&self, id: &str, enabled: bool) -> Result<StartupEntry, String> {
        self.startup.set_enabled(id, enabled)
    }

    pub fn open_settings(&self, page: &str) -> Result<(), String> {
        let uri = settings_uri(page).ok_or("unknown_page")?;
        std::process::Command::new("explorer.exe").arg(uri).spawn().map(|_| ()).map_err(|e| e.to_string())
    }
}
```

- [ ] **Step 4: Chạy test và clippy**

Run: `cargo test health::collect`, `cargo test health::service`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
Expected: collect `6`, service `2 passed`; toàn bộ `0 failed`. Hai test này mất 30–60 giây vì Storage WMI chậm — đã đo trên máy dev đang tải nặng: `MSFT_PhysicalDisk` 22–56 giây, `MSFT_Partition` 9 giây; vì thế loại ổ đĩa lấy bằng IOCTL (0,3 ms) và dòng `disk_health` được báo riêng khi xong.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-diagnose/src/health/collect.rs
rtk git add crates/winfreeup-diagnose/src/health/service.rs
rtk git commit -m "feat(diagnose): thu số liệu khám nhanh báo dần từng dòng, dịch vụ Tổng quan, mở Cài đặt theo danh sách trắng

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 19: Dịch vụ Ổ đĩa và gom ba dịch vụ

**Files:**
- Modify (thay dòng khung): `crates/winfreeup-diagnose/src/disk/service.rs`, `src/app.rs`
- Test: hai file trên

**Interfaces:**
- Consumes: Task 3 (`DiskTree`, `ChildrenPage`, `NodeView`, `Removed`, `MAX_CHILDREN`, `TreeBuilder`…), Task 4 (`ProtectRules`), Task 5 (`recycle::recycle`, `volumes::{list_volumes, plan_for, ScanPlan, VolumeInfo, WalkReason, DriveKind}`), Task 13 (`DiskScanner`, `ScanProgress`, `ScanStatus`, `WalkScanner`), Task 17 (`MftScanner`), Task 14 (`PerfService`, `RealKiller`, `EssentialRules`), Task 18 (`HealthService`), Task 9 (`Startup::from_env`), Task 1 (`ActionLog`, `log_dir`, `util::reveal_in_explorer`).
- Produces:
  - `disk::service::DiskOps` (trait: `volumes`, `scan(root, mft: bool, cancel, progress)`, `recycle`, `reveal`) + `RealDiskOps`; `ScanSummary { root: NodeView, plan: ScanPlan, elapsed_ms }`; `DeleteResult { removed, parent: Option<NodeView>, log_error }`; `DiskService::new(ops, rules, log)`, `volumes()`, `scan(root: &str, progress) -> Result<ScanSummary, String>` (MFT hỏng ⇒ tự lùi về duyệt thư mục, `plan` ghi lý do), `cancel()`, `children(id)` (điền `protected`), `reveal(id)`, `delete(id) -> Result<DeleteResult, String>`. Mã lỗi: `busy`, `no_tree`, `unknown_node`, `unknown_volume`, `protected`, `cancelled`.
  - `app::KhamMay { disk, health, perf, log }`, `KhamMay::from_system() -> Result<KhamMay, String>`, `shutdown(&self)`.

- [ ] **Step 1: Viết test hỏng** — thay dòng khung của hai file bằng phần test:

`disk/service.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::scan::ScanStatus;
    use crate::disk::tree::{TreeBuilder, FLAG_DIR, NO_PARENT};
    use crate::disk::volumes::DriveKind;
    use std::path::PathBuf;

    #[derive(Default)]
    struct FakeOps {
        mft_error: Option<String>,
        recycle_error: Option<String>,
        calls: Mutex<Vec<String>>,
    }

    impl DiskOps for FakeOps {
        fn volumes(&self) -> Vec<VolumeInfo> {
            vec![
                VolumeInfo { root: "C:\\".into(), label: "".into(), fs: "NTFS".into(), total: 100, free: 10, kind: DriveKind::Fixed },
                VolumeInfo { root: "E:\\".into(), label: "USB".into(), fs: "FAT32".into(), total: 100, free: 10, kind: DriveKind::Removable },
            ]
        }
        fn scan(&self, root: &Path, mft: bool, cancel: &CancelToken, progress: &dyn ScanProgress) -> Result<DiskTree> {
            self.calls.lock().unwrap().push(format!("scan:{}:{}", root.display(), if mft { "mft" } else { "walk" }));
            if cancel.is_cancelled() {
                return Err(CoreError::Cancelled);
            }
            if mft {
                if let Some(m) = &self.mft_error {
                    return Err(CoreError::System(m.clone()));
                }
            }
            progress.report(&ScanStatus { files: 2, bytes: 30, current: root.display().to_string(), percent: None });
            // X:\ ─ Users ─ an ─ Downloads ─ film.mkv 20 ; Windows ─ x.dll 10
            let mut b = TreeBuilder::new();
            let r = b.add(NO_PARENT, &root.display().to_string(), FLAG_DIR, 0, 0);
            let users = b.add(r, "Users", FLAG_DIR, 0, 0);
            let an = b.add(users, "an", FLAG_DIR, 0, 0);
            let dl = b.add(an, "Downloads", FLAG_DIR, 0, 0);
            b.add(dl, "film.mkv", 0, 20, 0);
            let win = b.add(r, "Windows", FLAG_DIR, 0, 0);
            b.add(win, "x.dll", 0, 10, 0);
            Ok(b.finish(r))
        }
        fn recycle(&self, path: &Path) -> Result<()> {
            self.calls.lock().unwrap().push(format!("recycle:{}", path.display()));
            match &self.recycle_error {
                Some(m) => Err(CoreError::System(m.clone())),
                None => Ok(()),
            }
        }
        fn reveal(&self, path: &Path) -> Result<()> {
            self.calls.lock().unwrap().push(format!("reveal:{}", path.display()));
            Ok(())
        }
    }

    fn service(ops: FakeOps) -> (tempfile::TempDir, DiskService) {
        let t = tempfile::tempdir().unwrap();
        let rules = ProtectRules::new(&[PathBuf::from(r"C:\Windows"), PathBuf::from(r"C:\Users")], Path::new(r"C:\Users\an"));
        let log = Arc::new(ActionLog::new(t.path().join("logs")));
        (t, DiskService::new(Box::new(ops), rules, log))
    }

    fn find(s: &DiskService, parent: NodeId, name: &str) -> NodeView {
        s.children(parent).unwrap().items.into_iter().find(|v| v.name == name).unwrap()
    }

    #[test]
    fn ntfs_drive_scans_with_mft_and_reports_the_plan() {
        let (_t, s) = service(FakeOps::default());
        let sum = s.scan("c:\\", &|_: &ScanStatus| {}).unwrap();
        assert_eq!(sum.plan, ScanPlan::Mft);
        assert_eq!(sum.root.bytes, 30);
        assert!(sum.root.protected);
    }

    #[test]
    fn mft_failure_falls_back_to_walking_with_the_verbatim_reason() {
        let (_t, s) = service(FakeOps { mft_error: Some("Access is denied. (os error 5)".into()), ..Default::default() });
        let sum = s.scan("C:\\", &|_: &ScanStatus| {}).unwrap();
        assert_eq!(sum.plan, ScanPlan::Walk { reason: WalkReason::MftFailed { message: "Access is denied. (os error 5)".into() } });
        assert_eq!(sum.root.bytes, 30);
    }

    #[test]
    fn fat32_usb_walks_directly() {
        let (_t, s) = service(FakeOps::default());
        let sum = s.scan("E:\\", &|_: &ScanStatus| {}).unwrap();
        assert_eq!(sum.plan, ScanPlan::Walk { reason: WalkReason::NotNtfs { fs: "FAT32".into() } });
        assert_eq!(s.scan("Z:\\", &|_: &ScanStatus| {}).unwrap_err(), "unknown_volume");
    }

    #[test]
    fn children_mark_protected_paths() {
        let (_t, s) = service(FakeOps::default());
        let root = s.scan("C:\\", &|_: &ScanStatus| {}).unwrap().root.id;
        assert!(find(&s, root, "Windows").protected);
        let users = find(&s, root, "Users");
        assert!(users.protected);
        let an = find(&s, users.id, "an");
        assert!(an.protected, "chính thư mục hồ sơ bị chặn");
        let dl = find(&s, an.id, "Downloads");
        assert!(!dl.protected, "con bên trong hồ sơ xóa được");
    }

    #[test]
    fn delete_recycles_subtracts_and_logs() {
        let (t, s) = service(FakeOps::default());
        let root = s.scan("C:\\", &|_: &ScanStatus| {}).unwrap().root.id;
        let an = find(&s, find(&s, root, "Users").id, "an");
        let dl = find(&s, an.id, "Downloads");
        let r = s.delete(dl.id).unwrap();
        assert_eq!(r.removed, Removed { bytes: 20, files: 1 });
        assert_eq!(r.parent.unwrap().bytes, 0);
        assert!(r.log_error.is_none());
        assert_eq!(s.children(root).unwrap().parent.bytes, 10);
        let log = std::fs::read_dir(t.path().join("logs")).unwrap().next().unwrap().unwrap().path();
        assert!(std::fs::read_to_string(log).unwrap().contains(r"DISK_RECYCLE bytes=20 files=1 C:\Users\an\Downloads"));
    }

    #[test]
    fn protected_nodes_are_refused_even_if_the_ui_asks() {
        let ops = FakeOps::default();
        let (_t, s) = service(ops);
        let root = s.scan("C:\\", &|_: &ScanStatus| {}).unwrap().root.id;
        let win = find(&s, root, "Windows");
        assert_eq!(s.delete(win.id).unwrap_err(), "protected");
        assert_eq!(s.delete(root).unwrap_err(), "protected");
        assert_eq!(s.children(root).unwrap().parent.bytes, 30, "không trừ gì");
    }

    #[test]
    fn recycle_failure_keeps_the_tree_and_returns_the_message() {
        let (_t, s) = service(FakeOps { recycle_error: Some("The process cannot access the file".into()), ..Default::default() });
        let root = s.scan("C:\\", &|_: &ScanStatus| {}).unwrap().root.id;
        let an = find(&s, find(&s, root, "Users").id, "an");
        let dl = find(&s, an.id, "Downloads");
        assert_eq!(s.delete(dl.id).unwrap_err(), "The process cannot access the file");
        assert_eq!(s.children(root).unwrap().parent.bytes, 30);
    }

    #[test]
    fn calls_before_any_scan_or_with_stale_ids_are_clean_errors() {
        let (_t, s) = service(FakeOps::default());
        assert_eq!(s.children(0).unwrap_err(), "no_tree");
        s.scan("C:\\", &|_: &ScanStatus| {}).unwrap();
        assert_eq!(s.children(9999).unwrap_err(), "unknown_node");
        assert_eq!(s.delete(9999).unwrap_err(), "unknown_node");
    }

    #[test]
    fn reveal_opens_the_node_path() {
        let (_t, s) = service(FakeOps::default());
        let root = s.scan("C:\\", &|_: &ScanStatus| {}).unwrap().root.id;
        let win = find(&s, root, "Windows");
        s.reveal(win.id).unwrap();
    }
}
```

`app.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_from_this_machine_and_lists_drives() {
        let k = KhamMay::from_system().unwrap();
        assert!(!k.disk.volumes().is_empty());
        assert!(k.log.path().is_none(), "chưa làm gì thì chưa có file nhật ký");
        k.shutdown();
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test disk::service` rồi `cargo test app::`
Expected: FAIL biên dịch — `cannot find trait DiskOps`, `cannot find type KhamMay`.

- [ ] **Step 3: Viết mã (trên khối test)**

`disk/service.rs`:

```rust
//! Tầng dịch vụ của mục Ổ đĩa: giữ cây của lần quét gần nhất, trả từng tầng, xóa có kiểm bảo vệ và ghi nhật ký.
//! Mọi lỗi trả về dạng chuỗi: mã ngắn (`busy`, `no_tree`, `unknown_node`, `unknown_volume`, `protected`,
//! `cancelled`) để giao diện dịch, còn lại là thông điệp hệ thống nguyên văn.
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use winfreeup_core::{CancelToken, CoreError, Result};

use super::protect::ProtectRules;
use super::recycle;
use super::scan::{DiskScanner, ScanProgress};
use super::tree::{ChildrenPage, DiskTree, NodeId, NodeView, Removed, MAX_CHILDREN};
use super::volumes::{self, plan_for, ScanPlan, VolumeInfo, WalkReason};
use super::walk::WalkScanner;
use crate::actlog::ActionLog;

/// Mọi thao tác chạm hệ thống của mục Ổ đĩa. Bản thật `RealDiskOps`, test dùng bản giả.
pub trait DiskOps: Send + Sync {
    fn volumes(&self) -> Vec<VolumeInfo>;
    fn scan(&self, root: &Path, mft: bool, cancel: &CancelToken, progress: &dyn ScanProgress) -> Result<DiskTree>;
    fn recycle(&self, path: &Path) -> Result<()>;
    fn reveal(&self, path: &Path) -> Result<()>;
}

pub struct RealDiskOps;

impl DiskOps for RealDiskOps {
    fn volumes(&self) -> Vec<VolumeInfo> {
        volumes::list_volumes()
    }
    fn scan(&self, root: &Path, mft: bool, cancel: &CancelToken, progress: &dyn ScanProgress) -> Result<DiskTree> {
        if mft {
            super::mft::MftScanner.scan(root, cancel, progress)
        } else {
            WalkScanner::default().scan(root, cancel, progress)
        }
    }
    fn recycle(&self, path: &Path) -> Result<()> {
        recycle::recycle(path)
    }
    fn reveal(&self, path: &Path) -> Result<()> {
        crate::util::reveal_in_explorer(path).map_err(CoreError::System)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanSummary {
    pub root: NodeView,
    /// Bộ quét thật sự đã dùng (sau khi lùi về duyệt thư mục nếu MFT hỏng).
    pub plan: ScanPlan,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteResult {
    pub removed: Removed,
    /// Cha sau khi đã trừ dung lượng — giao diện cập nhật dòng cha mà không quét lại.
    pub parent: Option<NodeView>,
    /// Không ghi được nhật ký (đã xóa xong) ⇒ giao diện hiện băng hổ phách.
    pub log_error: Option<String>,
}

pub struct DiskService {
    ops: Box<dyn DiskOps>,
    rules: ProtectRules,
    log: Arc<ActionLog>,
    tree: Mutex<Option<DiskTree>>,
    cancel: Mutex<CancelToken>,
    busy: AtomicBool,
}

struct Busy<'a>(&'a AtomicBool);

impl<'a> Busy<'a> {
    fn acquire(flag: &'a AtomicBool) -> std::result::Result<Busy<'a>, String> {
        if flag.swap(true, Ordering::SeqCst) {
            Err("busy".into())
        } else {
            Ok(Busy(flag))
        }
    }
}

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl DiskService {
    pub fn new(ops: Box<dyn DiskOps>, rules: ProtectRules, log: Arc<ActionLog>) -> Self {
        DiskService { ops, rules, log, tree: Mutex::new(None), cancel: Mutex::new(CancelToken::new()), busy: AtomicBool::new(false) }
    }

    pub fn volumes(&self) -> Vec<VolumeInfo> {
        self.ops.volumes()
    }

    pub fn scan(&self, root: &str, progress: &dyn ScanProgress) -> std::result::Result<ScanSummary, String> {
        let _busy = Busy::acquire(&self.busy)?;
        let volume = self.ops.volumes().into_iter().find(|v| v.root.eq_ignore_ascii_case(root)).ok_or("unknown_volume")?;
        let token = CancelToken::new();
        *self.cancel.lock().map_err(|e| e.to_string())? = token.clone();
        let started = Instant::now();
        let path = Path::new(&volume.root);
        let mut plan = plan_for(&volume);
        let tree = match plan {
            ScanPlan::Mft => match self.ops.scan(path, true, &token, progress) {
                Ok(t) => t,
                Err(CoreError::Cancelled) => return Err("cancelled".into()),
                Err(e) => {
                    plan = ScanPlan::Walk { reason: WalkReason::MftFailed { message: e.to_string() } };
                    self.walk(path, &token, progress)?
                }
            },
            ScanPlan::Walk { .. } => self.walk(path, &token, progress)?,
        };
        let mut root_view = tree.view(tree.root());
        root_view.protected = true;
        *self.tree.lock().map_err(|e| e.to_string())? = Some(tree);
        Ok(ScanSummary { root: root_view, plan, elapsed_ms: started.elapsed().as_millis() as u64 })
    }

    fn walk(&self, path: &Path, token: &CancelToken, progress: &dyn ScanProgress) -> std::result::Result<DiskTree, String> {
        self.ops.scan(path, false, token, progress).map_err(|e| match e {
            CoreError::Cancelled => "cancelled".to_string(),
            e => e.to_string(),
        })
    }

    pub fn cancel(&self) {
        if let Ok(t) = self.cancel.lock() {
            t.cancel();
        }
    }

    fn with_view(&self, v: NodeView) -> NodeView {
        let protected = self.rules.is_protected(Path::new(&v.path));
        NodeView { protected, ..v }
    }

    pub fn children(&self, id: NodeId) -> std::result::Result<ChildrenPage, String> {
        let guard = self.tree.lock().map_err(|e| e.to_string())?;
        let tree = guard.as_ref().ok_or("no_tree")?;
        let page = tree.children(id, MAX_CHILDREN).ok_or("unknown_node")?;
        Ok(ChildrenPage {
            parent: self.with_view(page.parent),
            items: page.items.into_iter().map(|v| self.with_view(v)).collect(),
            rest: page.rest,
        })
    }

    fn path_of(&self, id: NodeId) -> std::result::Result<String, String> {
        let guard = self.tree.lock().map_err(|e| e.to_string())?;
        let tree = guard.as_ref().ok_or("no_tree")?;
        if !tree.contains(id) {
            return Err("unknown_node".into());
        }
        Ok(tree.path(id))
    }

    pub fn reveal(&self, id: NodeId) -> std::result::Result<(), String> {
        let path = self.path_of(id)?;
        self.ops.reveal(Path::new(&path)).map_err(|e| e.to_string())
    }

    /// Xóa vào Thùng rác. Luật bảo vệ kiểm lại ở đây dù giao diện đã khóa nút — không tin phía giao diện.
    pub fn delete(&self, id: NodeId) -> std::result::Result<DeleteResult, String> {
        let _busy = Busy::acquire(&self.busy)?;
        let path = self.path_of(id)?;
        if self.rules.is_protected(Path::new(&path)) {
            return Err("protected".into());
        }
        if let Err(e) = self.ops.recycle(Path::new(&path)) {
            let msg = e.to_string();
            let _ = self.log.line(&format!("DISK_RECYCLE_FAILED {path} -- {msg}"));
            return Err(msg);
        }
        let mut guard = self.tree.lock().map_err(|e| e.to_string())?;
        let tree = guard.as_mut().ok_or("no_tree")?;
        let removed = tree.remove(id).ok_or("unknown_node")?;
        let parent = tree.parent_of(id).map(|p| tree.view(p));
        drop(guard);
        let log_error = self.log.line(&format!("DISK_RECYCLE bytes={} files={} {path}", removed.bytes, removed.files)).err();
        Ok(DeleteResult { removed, parent: parent.map(|v| self.with_view(v)), log_error })
    }
}
```

`app.rs`:

```rust
//! Gom ba dịch vụ của tab Khám máy cho vỏ Tauri, dùng chung một nhật ký hành động.
use std::path::PathBuf;
use std::sync::Arc;

use crate::actlog::{log_dir, ActionLog};
use crate::disk::protect::ProtectRules;
use crate::disk::service::{DiskService, RealDiskOps};
use crate::health::service::HealthService;
use crate::health::startup::Startup;
use crate::perf::apps::EssentialRules;
use crate::perf::service::{PerfService, RealKiller};

pub struct KhamMay {
    pub disk: DiskService,
    pub health: HealthService,
    pub perf: PerfService,
    pub log: Arc<ActionLog>,
}

impl KhamMay {
    /// Dựng từ máy đang chạy. Lỗi (thiếu biến môi trường) là thông điệp nguyên văn.
    pub fn from_system() -> Result<KhamMay, String> {
        let local = std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty()).ok_or("environment variable LOCALAPPDATA is not set")?;
        let windir = std::env::var_os("SystemRoot").or_else(|| std::env::var_os("windir")).ok_or("environment variable SystemRoot is not set")?;
        let log = Arc::new(ActionLog::new(log_dir(&PathBuf::from(local))));
        let essential = EssentialRules::new(&PathBuf::from(windir).join("System32"), std::process::id());
        Ok(KhamMay {
            disk: DiskService::new(Box::new(RealDiskOps), ProtectRules::from_env()?, log.clone()),
            health: HealthService::new(Startup::from_env(log.clone())?),
            perf: PerfService::new(Arc::new(essential), Box::new(RealKiller), log.clone()),
            log,
        })
    }

    /// Đóng ứng dụng: dừng lấy mẫu và phiên ETW `WinFreeUp-Net` (spec 5.6).
    pub fn shutdown(&self) {
        self.perf.shutdown();
    }
}
```

- [ ] **Step 4: Chạy toàn bộ crate và clippy**

Run: `cargo test disk::service`, `cargo test app::`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
Expected: service `9`, app `1 passed`; toàn crate `123 passed; 0 failed; 2 ignored`; clippy sạch.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-diagnose/src/disk/service.rs
rtk git add crates/winfreeup-diagnose/src/app.rs
rtk git commit -m "feat(diagnose): dịch vụ Ổ đĩa (MFT lùi về duyệt, bảo vệ kiểm lại khi xóa) và gom ba dịch vụ cho vỏ Tauri

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 20: Tích hợp vào ứng dụng v0.1

Chỉ mở khi (a) `feat/kham-may` đã gộp xong Đợt K4 và (b) v0.1 đã gộp Task 18 vào `feat/v0.1-don-o-dia` (có `src-tauri/`, `cleaners/registry.rs`, `.github/workflows/ci.yml`). Đây là task **duy nhất** sửa file của v0.1.

**Files:**
- Modify: `Cargo.toml` (gốc — thêm member), `Cargo.lock`
- Modify: `crates/winfreeup-diagnose/Cargo.toml` (xóa bảng `[workspace]`); Delete: `crates/winfreeup-diagnose/Cargo.lock`
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/main.rs`, `src-tauri/tauri.conf.json`; Create: `src-tauri/src/kham_may.rs`
- Modify: `src/i18n/vi.json`, `src/features/kham-may/i18n.ts`, `src/App.tsx`, `src/App.test.tsx`, `src/styles.css`
- Delete: `src/features/kham-may/vi.json`, `src/features/kham-may/i18n.test.ts`

**Interfaces:**
- Consumes: Task 19 (`winfreeup_diagnose::app::KhamMay::{from_system, shutdown}` và các trường `disk`, `health`, `perf` với chữ ký ở Task 19/18/14); các kiểu payload `ScanStatus`, `ScanSummary`, `ChildrenPage`, `DeleteResult`, `VolumeInfo`, `Finding`, `StartupEntry`, `StartupList`, `PerfTick`, `Sample`, `KillResult`; Task 2 (tên lệnh/sự kiện của `khamMayTauriApi`); Task 16 (`KhamMayTab` với prop `active`); v0.1 Task 16/17 (`App`, `commands::*`, `webview_check::show_fatal`, `ErrorBoundary`, `Notify`).
- Produces: `WinFreeUp.exe` có hai tab «Dọn dẹp» / «Khám máy»; 16 lệnh Tauri mới; `App({ api?, khamMayApi? })`.

- [ ] **Step 1: Nhận code v0.1 mới nhất**

```bash
rtk git merge feat/v0.1-don-o-dia
```

Expected: gộp không xung đột (Task 1–19 không chạm file của v0.1). Rồi `npm ci`.

- [ ] **Step 2: Đưa crate vào workspace gốc**

`Cargo.toml` (gốc) — đổi dòng `members` (Task 17 của v0.1 đã có `src-tauri`):

```toml
members = ["crates/winfreeup-core", "crates/winfreeup-diagnose", "src-tauri"]
```

`crates/winfreeup-diagnose/Cargo.toml` — xóa bốn dòng sau (hai dòng chú thích, dòng `[workspace]` và dòng trống ngay sau nó):

```toml
# Tạm là workspace riêng để build/test độc lập khi nhánh v0.1 chưa gộp xong.
# Task Tích hợp xoá bảng này và thêm crate vào `members` của workspace gốc.
[workspace]

```

Xóa `crates/winfreeup-diagnose/Cargo.lock` (từ giờ dùng `Cargo.lock` gốc):

```bash
rtk git rm crates/winfreeup-diagnose/Cargo.lock
```

Run (gốc repo): `cargo test -p winfreeup-diagnose` rồi `cargo test -p winfreeup-core` rồi `cargo tree -p wmi -e normal --depth 1`
Expected: cả hai `0 failed`; `cargo tree` in `windows v0.61.3` và `windows-core v0.61.2` (CÙNG 0.61 — nếu thấy `windows-core v0.62.x` là `wmi` bị nâng khỏi bản ghim, xem Global Constraints).

- [ ] **Step 3: Nối lệnh Tauri**

`src-tauri/Cargo.toml` — thêm ngay dưới dòng `winfreeup-core = { path = "../crates/winfreeup-core" }`:

```toml
winfreeup-diagnose = { path = "../crates/winfreeup-diagnose" }
```

`src-tauri/src/kham_may.rs` (mới):

```rust
//! Lệnh và sự kiện của tab Khám máy. Mỏng: mọi logic nằm ở crate `winfreeup-diagnose`.
//! Việc chạy lâu (quét, xóa, khám, dừng lấy mẫu) đẩy sang luồng nền để cửa sổ không bị treo.
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use winfreeup_diagnose::app::KhamMay;
use winfreeup_diagnose::disk::scan::ScanStatus;
use winfreeup_diagnose::disk::service::{DeleteResult, ScanSummary};
use winfreeup_diagnose::disk::tree::ChildrenPage;
use winfreeup_diagnose::disk::volumes::VolumeInfo;
use winfreeup_diagnose::health::rules::Finding;
use winfreeup_diagnose::health::startup::{StartupEntry, StartupList};
use winfreeup_diagnose::perf::monitor::{PerfTick, Sample};
use winfreeup_diagnose::perf::service::KillResult;

pub type Km = Arc<KhamMay>;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn disk_volumes(km: State<'_, Km>) -> Result<Vec<VolumeInfo>, String> {
    let k = km.inner().clone();
    blocking(move || k.disk.volumes()).await
}

#[tauri::command]
pub async fn disk_scan(app: AppHandle, km: State<'_, Km>, root: String) -> Result<ScanSummary, String> {
    let k = km.inner().clone();
    blocking(move || {
        k.disk.scan(&root, &|s: &ScanStatus| {
            let _ = app.emit("disk-scan-progress", s);
        })
    })
    .await?
}

#[tauri::command]
pub fn disk_scan_cancel(km: State<'_, Km>) {
    km.disk.cancel();
}

#[tauri::command]
pub fn tree_children(km: State<'_, Km>, node_id: u32) -> Result<ChildrenPage, String> {
    km.disk.children(node_id)
}

#[tauri::command]
pub async fn disk_delete(km: State<'_, Km>, node_id: u32) -> Result<DeleteResult, String> {
    let k = km.inner().clone();
    blocking(move || k.disk.delete(node_id)).await?
}

#[tauri::command]
pub fn disk_reveal(km: State<'_, Km>, node_id: u32) -> Result<(), String> {
    km.disk.reveal(node_id)
}

#[tauri::command]
pub async fn health_check(app: AppHandle, km: State<'_, Km>) -> Result<Vec<Finding>, String> {
    let k = km.inner().clone();
    blocking(move || {
        k.health.check(&|f: Finding| {
            let _ = app.emit("health-finding", &f);
        })
    })
    .await?
}

#[tauri::command]
pub async fn health_throttle(km: State<'_, Km>) -> Result<Finding, String> {
    let k = km.inner().clone();
    blocking(move || k.health.throttle()).await
}

#[tauri::command]
pub async fn startup_list(km: State<'_, Km>) -> Result<StartupList, String> {
    let k = km.inner().clone();
    blocking(move || k.health.startup_list()).await
}

#[tauri::command]
pub async fn startup_set(km: State<'_, Km>, id: String, enabled: bool) -> Result<StartupEntry, String> {
    let k = km.inner().clone();
    blocking(move || k.health.set_startup(&id, enabled)).await?
}

#[tauri::command]
pub fn open_settings(km: State<'_, Km>, page: String) -> Result<(), String> {
    km.health.open_settings(&page)
}

#[tauri::command]
pub fn perf_start(app: AppHandle, km: State<'_, Km>) -> Vec<Sample> {
    let tick_app = app.clone();
    km.perf.start(
        Arc::new(move |t: &PerfTick| {
            let _ = tick_app.emit("perf-tick", t);
        }),
        Arc::new(move |e: &str| {
            let _ = app.emit("perf-error", e);
        }),
    )
}

#[tauri::command]
pub async fn perf_stop(km: State<'_, Km>) -> Result<(), String> {
    let k = km.inner().clone();
    blocking(move || k.perf.stop()).await
}

#[tauri::command]
pub async fn app_kill(km: State<'_, Km>, key: String) -> Result<KillResult, String> {
    let k = km.inner().clone();
    blocking(move || k.perf.kill_app(&key)).await?
}

#[tauri::command]
pub fn reveal_path(km: State<'_, Km>, path: String) -> Result<(), String> {
    km.perf.reveal(&path)
}

#[tauri::command]
pub async fn app_icon(km: State<'_, Km>, path: String) -> Result<Option<String>, String> {
    let k = km.inner().clone();
    blocking(move || k.perf.icon(&path)).await
}
```

`src-tauri/src/main.rs` — thay toàn bộ bằng bản dưới (giữ nguyên phần của v0.1: kiểm WebView2, `Env::from_system`, 7 lệnh cũ; thêm `mod kham_may`, `KhamMay::from_system`, 16 lệnh mới, và dừng lấy mẫu + phiên ETW khi thoát). Nếu `main.rs` trên nhánh đã khác bản của kế hoạch v0.1 Task 17, giữ phần khác đó và chỉ thêm các dòng có chữ `kham_may`/`km`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod kham_may;
mod webview_check;

fn main() {
    // Kiểm WebView2 TRƯỚC khi tạo cửa sổ (spec mục 2.2).
    if let Err(e) = tauri::webview_version() {
        webview_check::show_missing(&e.to_string());
        return;
    }
    let dry_run = std::env::args().any(|a| a == "--dry-run");
    let env = match winfreeup_core::Env::from_system() {
        Ok(env) => env,
        Err(e) => {
            webview_check::show_fatal(&e.to_string());
            return;
        }
    };
    let km: kham_may::Km = match winfreeup_diagnose::app::KhamMay::from_system() {
        Ok(k) => std::sync::Arc::new(k),
        Err(e) => {
            webview_check::show_fatal(&e);
            return;
        }
    };
    let app = tauri::Builder::default()
        .manage(commands::AppState::new(env, dry_run))
        .manage(km.clone())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::disk_free,
            commands::scan_all,
            commands::cancel_scan,
            commands::prepare_restore_point,
            commands::clean,
            commands::open_log_folder,
            kham_may::disk_volumes,
            kham_may::disk_scan,
            kham_may::disk_scan_cancel,
            kham_may::tree_children,
            kham_may::disk_delete,
            kham_may::disk_reveal,
            kham_may::health_check,
            kham_may::health_throttle,
            kham_may::startup_list,
            kham_may::startup_set,
            kham_may::open_settings,
            kham_may::perf_start,
            kham_may::perf_stop,
            kham_may::app_kill,
            kham_may::reveal_path,
            kham_may::app_icon,
        ])
        .build(tauri::generate_context!());
    match app {
        // Đóng ứng dụng ⇒ dừng lấy mẫu và phiên ETW WinFreeUp-Net (spec 5.6).
        Ok(app) => app.run(move |_, event| {
            if let tauri::RunEvent::Exit = event {
                km.shutdown();
            }
        }),
        Err(e) => webview_check::show_fatal(&e.to_string()),
    }
}
```

`src-tauri/tauri.conf.json` — đổi kích thước cửa sổ (bảng cây 5 cột và 4 biểu đồ cần rộng hơn):

```json
        "width": 1180,
        "height": 780,
```

Run: `npm run build` rồi `cargo build -p winfreeup`
Expected: không lỗi (không `cargo test` cho `src-tauri` — `os error 740`, xem Global Constraints của v0.1).

- [ ] **Step 4: Trộn chuỗi vào `src/i18n/vi.json`**

Tạo file tạm `%TEMP%\merge-vi.mjs`:

```js
import fs from 'node:fs';
const G = 'src/i18n/vi.json';
const K = 'src/features/kham-may/vi.json';
const g = fs.readFileSync(G, 'utf8');
const k = fs.readFileSync(K, 'utf8');
const a = JSON.parse(g);
const b = JSON.parse(k);
for (const key of Object.keys(b)) if (key in a) throw new Error(`trùng khoá ${key}`);
const nl = g.includes('\r\n') ? '\r\n' : '\n';
const head = g.replace(/\s*\}\s*$/, '').replace('"app.title": "WinFreeUp — Dọn ổ đĩa"', '"app.title": "WinFreeUp"');
const body = k.replace(/^\s*\{\s*/, '').replace(/\s*\}\s*$/, '');
const out = `${head},${nl}${nl}  "app.tab.clean": "Dọn dẹp",${nl}  "app.tab.diagnose": "Khám máy",${nl}${nl}  ${body.replace(/\r?\n/g, nl)}${nl}}${nl}`;
const merged = JSON.parse(out);
if (Object.keys(merged).length !== Object.keys(a).length + Object.keys(b).length + 2) throw new Error('số khoá sau khi trộn không khớp');
fs.writeFileSync(G, out);
console.log(`đã trộn ${Object.keys(b).length} khoá km.* vào ${G}; tổng ${Object.keys(merged).length} khoá`);
```

Run (gốc repo): `node $env:TEMP\merge-vi.mjs`
Expected: `đã trộn 167 khoá km.* vào src/i18n/vi.json; tổng 256 khoá` (256 khi `vi.json` của v0.1 có 87 khoá như kế hoạch v0.1; nếu v0.1 đã thêm khoá thì tổng lớn hơn tương ứng — điều kiện đúng là script không ném lỗi). Tiêu đề đổi thành «WinFreeUp» vì cửa sổ giờ có hai tab.

`src/features/kham-may/i18n.ts` — thay toàn bộ bằng:

```ts
// Chuỗi của tab Khám máy đã trộn vào src/i18n/vi.json (Task Tích hợp); giữ tên `tk` để các file của tab không đổi.
export { t as tk, hasKey } from '../../i18n';
```

```bash
rtk git rm src/features/kham-may/vi.json
rtk git rm src/features/kham-may/i18n.test.ts
```

(Test «mọi chuỗi đều khác rỗng» của `src/i18n/i18n.test.ts` giờ phủ luôn các khoá `km.*`.)

- [ ] **Step 5: Viết test hỏng cho App**

`src/App.test.tsx` — thêm dòng import ngay dưới `import { App } from './App';`:

```tsx
import { fakeApi as fakeKhamMayApi } from './features/kham-may/testing/fakeApi';
```

và thêm vào cuối file:

```tsx
describe('App — tab Khám máy', () => {
  it('chỉ khám khi mở tab; lỗi của tab lên cùng băng thông báo; nút Dọn dẹp quay về tab Dọn dẹp', async () => {
    const km = fakeKhamMayApi({
      healthCheck: vi.fn(async () => [{ id: 'disk_full' as const, level: 'critical' as const, value: 3, detail: null }]),
      startupList: vi.fn(async () => Promise.reject('RegOpenKeyExW: Access is denied.')),
    });
    render(<App api={fakeApi()} khamMayApi={km} />);
    await screen.findByText('Ổ C: còn trống 50 GB');
    expect(km.healthCheck).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('tab', { name: 'Khám máy' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Dọn dẹp' }, { timeout: 5000 }));
    expect(screen.getByRole('tab', { name: 'Dọn dẹp' }).getAttribute('aria-selected')).toBe('true');
    expect(await screen.findByText('Không đọc được danh sách khởi động: RegOpenKeyExW: Access is denied.')).toBeTruthy();
    fireEvent.click(screen.getByRole('tab', { name: 'Khám máy' }));
    expect(km.healthCheck).toHaveBeenCalledTimes(1);
  }, 20000);
});
```

Run: `npx vitest run src/App.test.tsx`
Expected: FAIL — không tìm thấy `tab` tên «Khám máy».

- [ ] **Step 6: Gắn tab vào `App.tsx`**

Sáu chỗ sửa trong `src/App.tsx` (mỗi «tìm» xuất hiện đúng một lần):

1. Tìm `import { Badge, FluentProvider, Title2, webDarkTheme, webLightTheme } from '@fluentui/react-components';` — thay bằng:

```tsx
import { Badge, FluentProvider, Tab, TabList, Title2, webDarkTheme, webLightTheme } from '@fluentui/react-components';
```

2. Tìm `import { ResultView } from './components/ResultView';` — thêm ngay dưới:

```tsx
import { KhamMayTab } from './features/kham-may/KhamMayTab';
import type { KhamMayApi } from './features/kham-may/api/types';
```

3. Tìm `export function App({ api = tauriApi }: { api?: Api }) {` — thay bằng:

```tsx
type AppTab = 'clean' | 'diagnose';

export function App({ api = tauriApi, khamMayApi }: { api?: Api; khamMayApi?: KhamMayApi }) {
  const [tab, setTab] = useState<AppTab>('clean');
  // Tab Khám máy chỉ dựng lần đầu được mở, sau đó giữ nguyên (ẩn) để không khám/quét lại mỗi lần quay về.
  const [diagnoseOpened, setDiagnoseOpened] = useState(false);
  const openTab = (v: AppTab) => {
    if (v === 'diagnose') setDiagnoseOpened(true);
    setTab(v);
  };
```

4. Tìm `          <Title2 as="h1">{t('app.title')}</Title2>` — thêm ngay dưới:

```tsx
          <TabList selectedValue={tab} onTabSelect={(_, d) => openTab(d.value as AppTab)}>
            <Tab value="clean">{t('app.tab.clean')}</Tab>
            <Tab value="diagnose">{t('app.tab.diagnose')}</Tab>
          </TabList>
```

5. Tìm `        <ErrorBoundary onReset={() => run(c.goHome(deps))}>` — thêm ngay TRÊN dòng đó:

```tsx
        <div hidden={tab !== 'clean'}>
```

6. Tìm hai dòng `        </ErrorBoundary>` + `      </main>` — thay bằng:

```tsx
        </ErrorBoundary>
        </div>
        {diagnoseOpened && (
          <div hidden={tab !== 'diagnose'}>
            <ErrorBoundary onReset={() => setTab('clean')}>
              <KhamMayTab api={khamMayApi} notify={notify} onGoClean={() => setTab('clean')} active={tab === 'diagnose'} />
            </ErrorBoundary>
          </div>
        )}
      </main>
```

`active={tab === 'diagnose'}` là chỗ Review Focus 4: sang tab Dọn dẹp thì mục Bộ nhớ & Hiệu năng dừng lấy mẫu.

`src/styles.css` — trong khối `.wfu-app`, đổi `max-width: 880px;` thành `max-width: 1180px;`.

- [ ] **Step 7: Chạy toàn bộ**

Run (PowerShell, gốc repo):

```powershell
npm test
npm run typecheck
cargo test -p winfreeup-core
cargo test -p winfreeup-diagnose
cargo clippy -p winfreeup-diagnose --all-targets -- -D warnings
npx tauri build --no-bundle
cmd /c "findstr /m requireAdministrator target\release\WinFreeUp.exe"
(Get-Item target\release\WinFreeUp.exe).Length / 1MB
```

Expected: `npm test` `0 failed` (đo khi viết kế hoạch trên bản dựng thử v0.1 + Khám máy: `Test Files 19 passed`, `Tests 142 passed`); typecheck sạch; hai `cargo test` `0 failed`; `tauri build` xong; `findstr` in đường dẫn exe; kích thước khoảng 7 MB (đo khi viết kế hoạch: 7 113 728 byte = 6,8 MB, so với 6,01 MB của riêng v0.1). Ghi số thật vào commit message.

- [ ] **Step 8: Thử khởi động (tay, máy dev, terminal/Explorer)**

Mở `target\release\WinFreeUp.exe`, đồng ý UAC. Expected: tiêu đề «WinFreeUp», hai tab; tab «Dọn dẹp» như v0.1; tab «Khám máy» mở mục Tổng quan và các dòng hiện dần; mục Ổ đĩa quét `C:` bằng MFT (không có băng «quét chậm»); mục Bộ nhớ & Hiệu năng có biểu đồ chạy và cột Mạng có số (Admin ⇒ ETW chạy). **Không** xóa gì, không kết thúc app nào trên máy chính. Đóng cửa sổ rồi chạy `logman query "WinFreeUp-Net" -ets` ⇒ «Data Collector Set was not found» (phiên ETW đã dừng).

- [ ] **Step 9: Commit**

```bash
rtk git add Cargo.toml
rtk git add Cargo.lock
rtk git add crates/winfreeup-diagnose/Cargo.toml
rtk git add src-tauri/Cargo.toml
rtk git add src-tauri/src/main.rs
rtk git add src-tauri/src/kham_may.rs
rtk git add src-tauri/tauri.conf.json
rtk git add src/i18n/vi.json
rtk git add src/features/kham-may/i18n.ts
rtk git add src/App.tsx
rtk git add src/App.test.tsx
rtk git add src/styles.css
rtk git commit -m "feat: tích hợp tab Khám máy vào WinFreeUp — 16 lệnh Tauri, chuỗi vào vi.json, dừng ETW khi thoát

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

(`Cargo.lock` riêng của crate, `src/features/kham-may/vi.json`, `i18n.test.ts` đã được `git rm` ở Step 2/4 — nằm sẵn trong commit này.)

---

### Task 21: CI cho crate Khám máy và danh sách thử tay

**Files:**
- Modify: `.github/workflows/ci.yml`
- Create: `docs/thu-tay-kham-may.md`

**Interfaces:**
- Consumes: workflow `CI` của v0.1 Task 18; `cargo test -p winfreeup-diagnose` (Task 20); test `#[ignore]` `mft_matches_walk` (Task 17).
- Produces: CI chạy test, clippy và test VHD của crate Khám máy; bảng thử tay các phần cần Admin/máy thật.

- [ ] **Step 1: Thêm bước CI**

`.github/workflows/ci.yml` — thêm ngay sau bước `clippy (lõi)`:

```yaml
      - name: cargo test (khám máy)
        run: cargo test -p winfreeup-diagnose
      - name: clippy (khám máy)
        run: cargo clippy -p winfreeup-diagnose --all-targets -- -D warnings
      - name: MFT đối chiếu duyệt thư mục trên VHD (runner có Admin)
        run: cargo test -p winfreeup-diagnose -- --ignored mft_matches_walk
```

- [ ] **Step 2: Kiểm cục bộ đúng các lệnh CI**

Run (PowerShell, gốc repo, terminal **Run as administrator** để chạy được bước VHD): `cargo test -p winfreeup-diagnose`, `cargo clippy -p winfreeup-diagnose --all-targets -- -D warnings`, `cargo test -p winfreeup-diagnose -- --ignored mft_matches_walk`
Expected: tất cả thoát mã 0. Workflow chỉ chạy thật trên GitHub sau khi **người dùng đồng ý push**.

- [ ] **Step 3: Viết danh sách thử tay**

`docs/thu-tay-kham-may.md`:

````markdown
# Thử tay WinFreeUp — tab Khám máy

Chỉ thử xóa/kết thúc/đổi khởi động trong **Windows Sandbox** hoặc máy ảo. Phần chỉ xem (quét, biểu đồ) chạy được trên máy thật.
Sau mỗi hành động mở `%LOCALAPPDATA%\WinFreeUp\logs\` và đọc dòng `DISK_RECYCLE` / `STARTUP_*` / `APP_KILL`.

| # | Việc làm | Kỳ vọng |
|---|---|---|
| 1 | Tab Khám máy → Tổng quan, bấm đồng hồ | Các dòng hiện dần; 8 dòng xong trong khoảng 5 giây (dòng «Sức khỏe ổ cứng» có thể chậm hơn — ghi số giây thật); dòng hạ xung xong sau ~10 giây |
| 2 | Ổ đĩa → chọn `C:` → Quét, bấm đồng hồ | Có thanh % (MFT), **không** có băng «quét chậm»; xong dưới 1 phút; tổng gần bằng «Used space» của ổ trong Explorer |
| 3 | Cắm USB FAT32/exFAT, quét USB | Băng hổ phách «Đang dùng chế độ quét chậm vì ổ dùng hệ tệp FAT32…»; số file khớp Explorer (Properties của ổ) |
| 4 | Mở `C:\Windows`, `C:\Program Files`, `C:\Users` | Có 🔒, nút xóa bị khóa; `hiberfil.sys` (nếu có) hiện «Tắt ngủ đông để lấy lại … GB» + «Cách làm» |
| 5 | Tạo `Downloads\thu\a.bin` 100 MB (`fsutil file createnew`), quét, xóa `thu` | Hộp xác nhận ghi tên, dung lượng, số file; xong dòng biến mất, cây trừ ngay; `thu` nằm trong Thùng rác |
| 6 | Tạo file lớn hơn dung lượng tối đa của Thùng rác, xóa | Windows hỏi «xóa hẳn?»; bấm Hủy ⇒ băng đỏ «Bạn đã hủy thao tác xóa…», file còn nguyên |
| 7 | `mklink /J %USERPROFILE%\Downloads\sys C:\Windows\System32`, quét, thử xóa `sys` | Nút xóa bị khóa 🔒 (đường thật nằm trong System32) |
| 8 | Bộ nhớ & Hiệu năng: mở Notepad, bấm «Kết thúc app» trên dòng Notepad | Hộp xác nhận; Notepad đóng; dòng «Đã kết thúc «Notepad»» |
| 9 | Tìm dòng Service Host / Desktop Window Manager | Nút «Kết thúc app» bị khóa, rê chuột thấy «Tiến trình thiết yếu của Windows» |
| 10 | Tải một file lớn trong trình duyệt khi đang xem mục Bộ nhớ & Hiệu năng | Cột Mạng (KB/s) của trình duyệt có số; biểu đồ Mạng đi lên |
| 11 | Chạy stress CPU (vd `winsat cpuformal` hoặc mở vài tab video) ≥ 5 giây | Dải đỏ trên cả 4 biểu đồ; mục «Cơn giật vừa ghi nhận» có giờ, thời lượng, 3 app ngốn nhất |
| 12 | Máy xách tay: rút sạc, bật Energy saver, cắm sạc lại, Khám lại | Dòng «Tiết kiệm pin khi đang cắm sạc» 🟠 + nút mở Cài đặt Nguồn |
| 13 | Tổng quan → tắt một app khởi động, mở Task Manager → Startup apps | App đó «Disabled» trong Task Manager; bật lại trong WinFreeUp ⇒ «Enabled» |
| 14 | Máy có cảm biến ACPI thật (máy xách tay) | «Nhiệt độ CPU: … °C» đổi theo tải; máy ảo ⇒ «Máy này không cho đọc nhiệt độ» |
| 15 | Windows hiển thị tiếng Việt (Language pack vi-VN) | Biểu đồ CPU/Đĩa và chỉ báo hạ xung vẫn có số (bộ đếm PDH thêm bằng tên tiếng Anh) |
| 16 | Sang tab «Dọn dẹp» khi đang ở mục Bộ nhớ & Hiệu năng, mở Task Manager xem CPU của WinFreeUp | CPU của WinFreeUp về ~0% (đã dừng lấy mẫu); quay lại ⇒ biểu đồ chạy tiếp |
| 17 | Đóng WinFreeUp, chạy `logman query "WinFreeUp-Net" -ets` | «Data Collector Set was not found» |
| 18 | `Settings → Accessibility → Visual effects → Animation effects: Off`, quét ổ | Vòng quay đổi thành nhịp mờ tỏ, vẫn chuyển động |

Ghi kết quả từng dòng (đạt/không đạt + ảnh chụp + số đo) vào PR của bản phát hành.
````

- [ ] **Step 4: Commit**

```bash
rtk git add .github/workflows/ci.yml
rtk git add docs/thu-tay-kham-may.md
rtk git commit -m "ci: test, clippy và đối chiếu MFT trên VHD cho crate Khám máy + danh sách thử tay

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Sau Đợt K6 — trước khi báo xong

- [ ] Trên `feat/kham-may` đã gộp đủ: `cargo test -p winfreeup-core`, `cargo test -p winfreeup-diagnose` (Expected `123 passed; 0 failed; 2 ignored`), `npm test`, `npx tauri build --no-bundle` thành công.
- [ ] Quét dấu hiệu làm dở: `rtk grep -n "TODO\|todo!\|unimplemented!\|\.only(\|\.skip(\|khung — nội dung" crates src src-tauri` ⇒ không có kết quả (dòng khung nào còn sót là task chưa làm).
- [ ] Rà toàn nhánh bằng một reviewer mới (`superpowers:requesting-code-review`).
- [ ] **Dừng và hỏi người dùng** trước khi gộp `feat/kham-may` vào `feat/v0.1-don-o-dia`/`main` hoặc push (CLAUDE.md mục 6).
