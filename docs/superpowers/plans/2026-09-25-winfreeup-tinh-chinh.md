# WinFreeUp — Dự án con 3 «Tinh chỉnh» — Kế hoạch triển khai

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Thêm tab **«Tinh chỉnh»** vào `WinFreeUp.exe`: người dùng bấm một mức sẵn (Cơ bản / Khuyến nghị / Triệt để), xem danh sách thay đổi (gỡ app cài sẵn + tắt quảng cáo/thu thập dữ liệu), bấm **Áp dụng**; mọi mục báo trạng thái đọc lại từ máy thật và **hoàn tác được từ trong app**.

**Architecture:** Crate Rust mới `crates/winfreeup-tweaks` (không phụ thuộc Tauri, không phụ thuộc `winfreeup-core`) chứa kiểu danh mục, danh mục `catalog.toml` nhúng bằng `include_str!`, danh sách cấm gỡ, kho ảnh chụp hoàn tác, bộ tính trạng thái và bộ máy áp dụng/hoàn tác; mọi lời gọi hệ thống đi qua trait riêng `TweakOps` (registry, dịch vụ, tác vụ theo lịch, gói app) — bản thật ở `sys_windows/` (windows-rs 0.62), bản giả `fake::FakeOps` cho test. Giao diện nằm trọn trong `src/features/tinh-chinh/` (reducer thuần + bộ điều khiển + component Fluent UI v9), chuỗi tiếng Việt tạm ở JSON riêng của tính năng. Chỉ **Task 12 «Tích hợp»** chạm file dùng chung với v0.1 (workspace, vỏ Tauri, `vi.json`, `App.tsx`, CI).

**Tech Stack:** Rust 1.90 (edition 2021), windows 0.62.2 + windows-future 0.3.2 (Registry, SCM, Task Scheduler COM, WinRT `PackageManager`, `ShellExecuteW`), toml 1.1.6, serde 1.0.229, serde_json 1.0.151, tempfile 3.27 (dev); Tauri 2.11.6 (chỉ ở Task 12); React 19.3, TypeScript 5.9, Fluent UI React v9 9.74, Vitest 5 + Testing Library.

**Spec:** `docs/superpowers/specs/2026-09-25-winfreeup-tinh-chinh-design.md` — kế hoạch dùng lại khung v0.1 theo `docs/superpowers/plans/2026-09-25-winfreeup-don-o-dia.md` (Task 8 `log`/`engine::RestorePointStatus`, Task 9 `Env`/`RealSystem`, Task 10 `vi.json`/`api/types.ts`, Task 12 `Busy`/`errors`, Task 12 `state/store.ts`, Task 16 `App.tsx`, Task 17 vỏ Tauri, Task 18 CI).

## Global Constraints

Giá trị chép nguyên văn từ spec; mọi task ngầm bao gồm mục này.

- Nền tảng (kế thừa v0.1): «Một file `.exe`, `requireAdministrator`, Windows 10 1903+ / 11, 64-bit.» «Tauri 2 + React + TypeScript + Fluent UI React v9; lõi Rust `winfreeup-core`.» — kế hoạch đặt phần Tinh chỉnh ở crate **anh em** `winfreeup-tweaks` (lý do ở mục «Quyết định kiến trúc» dưới); lõi vẫn không phụ thuộc Tauri.
- Chuỗi: «Tiếng Việt, chuỗi trong `src/i18n/vi.json`; luật lỗi và nhật ký chung.» Trước Task 12 chuỗi nằm ở `src/features/tinh-chinh/vi.json` + `catalog.vi.json`; Task 12 trộn vào `src/i18n/vi.json`. Lõi không chứa chữ hiển thị — chỉ mã (`build_min:26100`, `undo_corrupt:…`) và lỗi kỹ thuật nguyên văn.
- «**Không đụng**: Defender, Windows Update, SmartScreen, Firewall.» «**Không gỡ**: thành phần mà thiếu nó phải cài lại Windows (Store, Edge, framework, shell).» — Task 2 chặn bằng test danh mục.
- Danh mục: «`tweaks/catalog.toml`, nhúng vào exe lúc build (`include_str!`).» (đặt ở `crates/winfreeup-tweaks/catalog.toml`). «`max_build = 0`: không giới hạn.» «`default = "absent"` nghĩa là mặc định khóa không tồn tại.» «Tên gói, ProductId Store và khóa registry trong danh mục **phải được xác minh trên máy thật** (Win 10 22H2, Win 11 23H2/24H2)» — mục «Xác minh danh mục» ghi rõ cái nào đã đo, cái nào còn thử tay.
- Thao tác: đúng bảng spec 3.1 — `registry_set`, `registry_delete`, `service_startup` («đổi kiểu + dừng dịch vụ nếu `disabled`» / hoàn tác «đổi về kiểu cũ (không tự khởi động dịch vụ)»), `scheduled_task_disable` («bật lại nếu ảnh chụp là bật»), `appx_remove` (hoàn tác «mở `ms-windows-store://pdp/?ProductId=<id>`»).
- Trạng thái: «Luôn **tính từ máy thật**, không từ ký ức của app» — Đã áp dụng / Chưa áp dụng / Một phần / Không hỗ trợ trên máy này / Bị chính sách tổ chức quản lý («máy vào domain hoặc đăng ký MDM **và** giá trị dưới `...\Policies\...` đã tồn tại với giá trị khác đích trước khi áp dụng»).
- Hoàn tác: «`%LOCALAPPDATA%\WinFreeUp\tweaks-undo.json` (theo `tweak_id` + chỉ số thao tác). **Chỉ ghi lần đầu**», «Hoàn tác thành công ⇒ xóa ảnh chụp của mục đó», «Không có ảnh chụp ⇒ dùng `default`», «ghi file tạm rồi đổi tên».
- Lượt áp dụng: «Tạo điểm khôi phục (không được ⇒ băng hổ phách, người dùng chọn tiếp/dừng — như v0.1)», «lỗi ⇒ ghi lại, chạy tiếp thao tác khác», «Đọc lại trạng thái thật», «`explorer` ⇒ nút "Khởi động lại Explorer"; `logoff`/`reboot` ⇒ nhắc», «Ghi nhật ký chung».
- Gỡ app: «**Mặc định: tài khoản hiện tại**», «**Nâng cao: mọi tài khoản + chặn cài lại** (`RemovePackageOptions.RemoveForAllUsers` + `DeprovisionPackageForAllUsersAsync`): ô tích riêng, cảnh báo "Khó hoàn tác; bản cập nhật Windows lớn có thể vẫn cài lại một số app"». Danh sách cấm gỡ «mã hóa cứng trong lõi (danh mục không ghi đè được; test chặn danh mục vi phạm)».
- Giao diện: «Mở tab ⇒ đọc trạng thái mọi mục (vòng quay; liệt kê app 1–3 giây).» «Bấm mức sẵn ⇒ tích mọi mục có `level` ≤ mức đó … mục không hỗ trợ/bị quản lý không bao giờ được tích.» «Nút **Áp dụng** đếm số thay đổi thật.» «**Xác nhận**: có mục `caution` hoặc bật "mọi tài khoản" ⇒ hộp liệt kê các mục đó.» «Đang áp dụng ⇒ khóa mọi nút, tiến độ từng mục; xong ⇒ kết quả ✓ / ⚠ một phần / ✗ lỗi nguyên văn».
- Lỗi: «Luật chung như v0.1 (băng đỏ / hổ phách, móc lỗi toàn cục, gộp trùng, tối đa 5 dòng, log đầy đủ).» «File ảnh chụp hỏng/không đọc được ⇒ băng hổ phách "Không đọc được dữ liệu hoàn tác; hoàn tác sẽ dùng giá trị mặc định của Windows", đổi tên file hỏng thành `.bak`, không xóa.»
- Test: «mỗi `kind` chạy trên khóa thật riêng `HKCU\Software\WinFreeUpTest\<uuid>` (dọn sau mỗi test); dịch vụ/tác vụ/gói app qua lớp giả». Test danh mục: «`id` duy nhất; mọi `id` có chuỗi trong `vi.json`; mọi `registry_set` có `default`; mọi `appx_remove` có `store_product_id`; **không gói nào khớp danh sách cấm gỡ**; `min_build ≤ max_build` khi `max_build ≠ 0`.» Test máy trạng thái mức sẵn bằng Vitest.
- **Chạy song song với v0.1 và `feat/kham-may`:** trước Task 12, **không task nào sửa file do v0.1 sở hữu** (`Cargo.toml` gốc, `Cargo.lock` gốc, `crates/winfreeup-core/**`, `src-tauri/**`, `src/i18n/vi.json`, `src/App.tsx`, `package.json`, `.github/**`). Không sửa trait `SystemOps` của v0.1 — phần Tinh chỉnh có trait riêng `TweakOps`.
- **Phiên bản đo khi viết kế hoạch (2026-09-25, máy dev Win 11 Pro 25H2 build 26200.9445, terminal KHÔNG Admin):** rustc 1.90.0; windows 0.62.2, windows-future 0.3.2, toml 1.1.6+spec-1.1.0, serde 1.0.229, serde_json 1.0.151, tempfile 3.27.0; tauri 2.11.6 (Task 12). Toàn bộ `cargo test` của crate (58 test, gồm registry thật, SCM, Task Scheduler, PackageManager chỉ-đọc) và `cargo clippy --all-targets -- -D warnings` **qua trên terminal thường, không cần Admin**. Không dùng `windows` 0.61 hay `windows-sys` cho crate này (0.62 gom COM + WinRT + Win32 trong một crate).
- `cargo test` của crate này trước Task 12: `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml` (crate có bảng `[workspace]` riêng nên không đụng workspace gốc). Sau Task 12: `cargo test -p winfreeup-tweaks`. Không chạy `cargo test` cho `src-tauri` (lý do `os error 740` — Global Constraints của v0.1).
- Git: tiền tố `rtk` cho lệnh git; `git add` **từng tệp một**; mọi commit kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; mỗi task một worktree + nhánh `task/tc-<số>-<tên-ngắn>` tách từ `feat/tinh-chinh`; không push, không merge vào `main` khi chưa hỏi người dùng.
- Luật chung của người dùng: vòng quay cho mọi thao tác chờ lõi (kể cả đọc lại, «Cài lại từ Store», khởi động lại Explorer) — **áp dụng**; phủ mờ khi đọc lại — **áp dụng**; báo lỗi lên màn, hai mức đỏ/hổ phách, nguyên văn — **áp dụng**. Bộ lọc trong URL, đánh dấu T7/CN, tên người/ID — **không áp dụng** (ứng dụng máy tính, không có bộ lọc, trục ngày hay tên người).

### Quyết định kiến trúc (đã tự chọn — ghi để người rà khỏi hỏi)

1. **Crate riêng `winfreeup-tweaks` thay vì module trong `winfreeup-core`.** `crates/winfreeup-core/src/lib.rs` do Task 1 của v0.1 sở hữu và «các task sau không sửa»; thêm `pub mod tweaks;` trước khi v0.1 xong sẽ đụng file đó giữa lúc ba nhánh chạy song song. Crate riêng có bảng `[workspace]` rỗng nên build/test độc lập từ Đợt A; Task 12 chỉ thêm một dòng `members` và xoá bảng đó. Crate không phụ thuộc `winfreeup-core` — điểm khôi phục và nhật ký dùng hàm của v0.1 **ở vỏ Tauri** (Task 12), nên đổi interface lõi v0.1 chỉ ảnh hưởng một file.
2. **Mặc định khi mở tab tích sẵn mức «Cơ bản»** (toàn mục an toàn, giống v0.1 tích sẵn nhóm An toàn). Spec không nói mặc định. Người dùng đổi lựa chọn rồi đọc lại (sau khi áp dụng) thì giữ lựa chọn của họ.
3. **Hoàn tác app đã gỡ giữ ảnh chụp cho tới khi app được cài lại** (spec: «Hoàn tác thành công ⇒ xóa ảnh chụp»; mở trang Store chưa phải là đã cài lại). Nhờ vậy nút «Cài lại từ Store» không biến mất nếu người dùng đóng Store giữa chừng.
4. **Chạy bằng tài khoản admin khác** (UAC nhập mật khẩu admin khác): phát hiện bằng `WTSQuerySessionInformationW` so với `GetUserNameW`, hiện băng hổ phách; không chặn. **Máy «do tổ chức quản lý»** = vào domain (`NetGetJoinInformation`) hoặc có key `Enrollments` với ProviderID `MS DM Server` — máy dev có 35 key Enrollments của chính Windows, không key nào là MDM thật.
5. **Chạy thử `--dry-run`**: tab Tinh chỉnh chỉ xem; lệnh áp dụng/hoàn tác/khởi động lại Explorer trả lỗi `dry_run`, điểm khôi phục trả `skipped`.
6. Tab «Dọn ổ đĩa» và «Tinh chỉnh» khoá lẫn nhau khi một bên đang chạy (hai thao tác cùng muốn tạo điểm khôi phục; Windows chỉ cho 1 điểm/24 giờ). `app.title` đổi thành «WinFreeUp» vì cửa sổ giờ có hai tab.
7. **Teams mới `MSTeams` nằm trong danh sách cấm gỡ** (dùng chung công việc + cá nhân). **Teams cá nhân cũ `MicrosoftTeams` tạm KHÔNG có trong danh mục** — Microsoft đã bỏ app này, không còn trang Store nên gỡ rồi không hoàn tác được; đang chờ người dùng chọn (mục «Câu hỏi còn mở» cuối kế hoạch).
8. Mục «Win 10» trong bảng gỡ app của spec **không** bị giới hạn `max_build` — máy nâng cấp lên Win 11 vẫn có thể còn 3D Viewer/Paint 3D; app không có trên máy thì tự ẩn.
9. `task_appraiser` tắt cả hai tên tác vụ: `Microsoft Compatibility Appraiser` (Win 10/11 cũ) và `Microsoft Compatibility Appraiser Exp` (đo trên build 26200: chỉ còn tên này). Tác vụ không có trên máy được bỏ qua khi tính trạng thái.

## Review Focus

1. Người dùng thường chạy WinFreeUp và UAC hỏi mật khẩu **một tài khoản admin khác** ⇒ `HKCU` và danh sách app là của tài khoản admin, không phải của người đang ngồi máy. Người dùng phải thấy băng hổ phách nói rõ điều đó trước khi áp dụng, thay vì «áp dụng xong» mà máy mình không đổi gì. (Test: Task 6 `info::tests::reads_this_machine` khẳng định máy dev `other_user = false`; Task 11 `chạy bằng tài khoản admin khác ⇒ băng hổ phách cảnh báo`.)
2. App **không có trên máy** và **sai build** (Widgets `min_build = 22000` trên Win 10) ⇒ mục bị ẩn, không hiện dòng «Không hỗ trợ trên máy này» cho một app người dùng chưa từng thấy. (Test: Task 4 `absent_app_on_unsupported_build_is_hidden_not_unsupported`.)
3. Máy cá nhân có hàng chục key dưới `HKLM\SOFTWARE\Microsoft\Enrollments` (Local/Cloud/Deploy Authority — đo trên máy dev: 35 key) ⇒ **không** được coi là «do tổ chức quản lý»; coi sai thì mọi mục `\Policies\` mà công cụ khác từng đặt bị khoá. (Test: Task 6 `only_real_mdm_counts_as_managed`; Task 4 `managed_only_when_org_policy_already_differs`.)
4. Không ghi được file hoàn tác (ổ đầy, thư mục `%LOCALAPPDATA%\WinFreeUp` bị chặn, file trùng tên thư mục) ⇒ thao tác **không được chạy** — thà không đổi gì còn hơn đổi mà không có đường quay lại. (Test: Task 5 `op_is_not_run_when_snapshot_cannot_be_saved`.)
5. Value registry do công cụ khác ghi **sai kiểu** (chuỗi `"0"` thay vì DWORD) ⇒ áp dụng ghi đúng kiểu, hoàn tác trả lại **đúng chuỗi cũ** chứ không đổi sang `default` hay DWORD. (Test: Task 5 `value_of_unexpected_type_is_snapshotted_and_restored_as_is`.)

---


## File Structure

```
WinFreeUp/
├─ crates/winfreeup-tweaks/                  CRATE MỚI — chỉ phần Tinh chỉnh sở hữu
│  ├─ Cargo.toml                             ĐỦ mọi phụ thuộc + bảng [workspace] tạm (Task 1; Task 12 xoá bảng đó)
│  ├─ Cargo.lock                             lock riêng tạm thời (Task 1; Task 12 xoá — dùng lock gốc)
│  ├─ catalog.toml                           danh mục v1: 20 mục quyền riêng tư + 28 mục gỡ app (Task 2)
│  └─ src/
│     ├─ lib.rs                              khai báo SẴN mọi module (Task 1) — task sau không sửa
│     ├─ model.rs                            Group, Level, Risk, Restart, StartType, RegType, RegValue, WindowsReq, Op, Tweak, parse_catalog, to_reg_data, default_data (Task 1)
│     ├─ ops.rs                              RegData, PackageInfo, SystemInfo, edition_of, trait TweakOps (Task 1)
│     ├─ fake.rs                             #[cfg(test)] FakeOps — registry/dịch vụ/tác vụ/gói trong bộ nhớ (Task 1)
│     ├─ blocklist.rs                        khung (Task 1) → BLOCKED_PACKAGES, FORBIDDEN_*, is_blocked_package… (Task 2)
│     ├─ catalog.rs                          khung (Task 1) → CATALOG_TOML, builtin, validate, name_key, desc_key (Task 2)
│     ├─ undo.rs                             khung (Task 1) → Snapshot, UndoStore, default_path (Task 3)
│     ├─ state.rs                            khung (Task 1) → TweakStatus, OpState, supported, op_state, tweak_status (Task 4)
│     ├─ engine.rs                           khung (Task 1) → TweakView, ReadResult, TweakOutcome, RunReport, TweakEvent, read_all, apply, revert, log_lines (Task 5)
│     └─ sys_windows/
│        ├─ mod.rs                           khai báo SẴN 7 module con (Task 1) — task sau không sửa
│        ├─ registry.rs, info.rs, shell.rs   khung (Task 1) → Task 6
│        ├─ services.rs, tasks.rs            khung (Task 1) → Task 7
│        └─ appx.rs, real.rs                 khung (Task 1) → Task 8 (RealTweakOps)
├─ src/features/tinh-chinh/                  THƯ MỤC MỚI — chỉ phần Tinh chỉnh sở hữu
│  ├─ catalog.vi.json                        tên + mô tả 48 mục (Task 2; Task 12 trộn vào src/i18n/vi.json rồi xoá)
│  ├─ vi.json                                chuỗi giao diện (Task 9; Task 12 trộn rồi xoá)
│  ├─ strings.ts                             tt(), hasKey() đọc hai JSON trên (Task 9; Task 12 đổi thành re-export `t`)
│  ├─ types.ts, testdata.ts                  kiểu khớp serde + dữ liệu mẫu cho test (Task 9)
│  ├─ presets.ts, presets.test.ts            mức sẵn, đếm thay đổi, danh sách xác nhận (Task 9)
│  ├─ labels.ts, labels.test.ts              nhãn trạng thái/lý do/kết quả (Task 9)
│  ├─ reducer.ts, reducer.test.ts            máy trạng thái của tab (Task 10)
│  ├─ api.ts, api.test.ts                    tauriTweakApi — lệnh tweaks_* và sự kiện tweak-progress (Task 10)
│  ├─ controller.ts, controller.test.ts      bộ điều khiển: đọc, áp dụng, hoàn tác, điểm khôi phục (Task 10)
│  ├─ TweakList.tsx, Dialogs.tsx, RunPanel.tsx, TinhChinhView.tsx, tinh-chinh.css, views.test.tsx (Task 11)
├─ src-tauri/src/tweak_commands.rs           5 lệnh tweaks_* (Task 12)
├─ docs/thu-tay-tinh-chinh.md                danh sách thử tay Sandbox/VM (Task 12)
└─ SỬA ở Task 12 (file của v0.1): Cargo.toml gốc, Cargo.lock gốc, src-tauri/Cargo.toml, src-tauri/src/main.rs,
   src/i18n/vi.json, src/App.tsx, src/App.test.tsx, .github/workflows/ci.yml
```

Nguyên tắc chia: mỗi mảng chạm hệ thống thật một file trong `sys_windows/`, sau trait `TweakOps`; logic (trạng thái, ảnh chụp, bộ máy) chỉ thấy trait nên test chạy trên `FakeOps`, riêng registry chạy thêm trên khoá thật như spec yêu cầu. Giao diện tách logic thuần (`presets`, `labels`, `reducer`, `controller`) khỏi component.

Nguyên tắc chống đụng file: như v0.1 — `lib.rs` và `sys_windows/mod.rs` khai báo sẵn mọi module ở Task 1, module chưa tới lượt là **file khung một dòng `//!`**; task chủ sở hữu thay toàn bộ dòng khung. Mọi phụ thuộc crate khai báo đủ ở Task 1 nên `Cargo.toml`/`Cargo.lock` của crate không đổi sau đó.

## Đợt thi công

Theo `CLAUDE.md` «Quy trình thi công»: mỗi task một worktree + nhánh `task/tc-<số>-<tên-ngắn>` tách từ `feat/tinh-chinh`; hết mỗi đợt gộp vào `feat/tinh-chinh` rồi chạy **toàn bộ** `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml` + `npm test`; mỗi task một lượt rà riêng trước khi gộp.

| Đợt | Task (song song trong đợt) | Chờ | File dùng chung — ai sở hữu |
|---|---|---|---|
| A | **Task 1** (khung crate, model, ops, FakeOps) | `feat/tinh-chinh` đã tách (sau Đợt 0 của v0.1) | Task 1: `Cargo.toml`/`Cargo.lock` của crate, `lib.rs`, `sys_windows/mod.rs`, mọi file khung |
| B | Task 2 (blocklist, catalog, `catalog.vi.json`) · Task 3 (undo) · Task 7 (services, tasks) | Task 1 | mỗi task file riêng |
| C | Task 4 (state) · Task 9 (FE: types, strings, vi.json, presets, labels) | Task 4 ← 3; Task 9 ← 2 | không chung file |
| D | Task 5 (engine) · Task 10 (FE: reducer, api, controller) | Task 5 ← 2, 3, 4; Task 10 ← 9 **và** v0.1 Task 12 + 13 đã có trên `feat/tinh-chinh` (xem «Đồng bộ với v0.1») | không chung file |
| E | Task 6 (registry, info, shell) · Task 11 (FE: các component) | Task 6 ← 5; Task 11 ← 10 | không chung file |
| F | Task 8 (appx, `RealTweakOps`) | Task 6, 7 | — |
| G | **Task 12** Tích hợp (workspace, vỏ Tauri, `vi.json`, `App.tsx`, CI, thử tay) | mọi task trên **và** v0.1 đã gộp đủ Task 16, 17, 18 | task duy nhất sửa file của v0.1 |

Đường găng: 1 → 3 → 4 → 5 → 6 → 8 → 12. Nhánh giao diện 2 → 9 → 10 → 11 chạy song song.

Mỗi task báo số test **của riêng nó** (vd `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml undo` → `5 passed`) và yêu cầu toàn bộ bộ test «0 failed». Số đo khi viết kế hoạch (chạy trên mã chép nguyên từ kế hoạch): Rust 58 test, Vitest 45 test của tính năng (13 + 22 + 10).

### Đồng bộ với v0.1

- `feat/tinh-chinh` tách từ `feat/v0.1-don-o-dia` khi Đợt 0 của v0.1 đã gộp. Lúc viết kế hoạch, `feat/v0.1-don-o-dia` đã có Task 1–3, 7–12, 14, 15 (Task 13 còn ở worktree).
- **Trước Đợt D** (Task 10 dùng `src/state/store.ts` của v0.1 Task 13 và `src/errors/errors.ts` của Task 12) và **trước Đợt G**: gộp nhánh tính năng v0.1 vào nhánh này — đây là gộp giữa hai nhánh tính năng, không đụng `main`:

```bash
cd D:/META/wfu-wt/feat-tinh-chinh
rtk git merge --no-ff feat/v0.1-don-o-dia -m "merge: feat/v0.1-don-o-dia vào feat/tinh-chinh (đồng bộ trước Đợt D)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
npm ci
cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml
npm test
```

Expected: merge không xung đột (phần Tinh chỉnh chỉ có file mới); hai lệnh test `0 failed`. Nếu v0.1 còn đang sửa `store.ts`/`errors.ts` thì chờ tới khi task đó gộp.

## Chuẩn bị (một lần, trước Đợt A)

- [ ] Tạo nhánh tính năng và worktree riêng (không làm trên `D:/META/WinFreeUp` — thư mục đó đang ở `main`):

```bash
cd D:/META/WinFreeUp
rtk git worktree add -b feat/tinh-chinh D:/META/wfu-wt/feat-tinh-chinh feat/v0.1-don-o-dia
cd D:/META/wfu-wt/feat-tinh-chinh
npm ci
```

Expected: `Preparing worktree (new branch 'feat/tinh-chinh')`; `npm ci` xong không lỗi (mất vài phút — gói `@fluentui/react-icons` có hàng chục nghìn file).

- [ ] Mỗi task: dựng worktree riêng bằng skill `superpowers:using-git-worktrees`, nhánh `task/tc-<số>-<tên-ngắn>` tách từ `feat/tinh-chinh` (vd `task/tc-03-undo`). Mọi lệnh trong task chạy ở thư mục worktree đó. Task giao diện (9–11) cần `npm ci` một lần trong worktree.

---

### Task 1: Khung crate `winfreeup-tweaks`, kiểu danh mục và trait `TweakOps`

**Files:**
- Create: `crates/winfreeup-tweaks/Cargo.toml`
- Create: `crates/winfreeup-tweaks/Cargo.lock` (cargo sinh ra)
- Create: `crates/winfreeup-tweaks/src/lib.rs`
- Create: `crates/winfreeup-tweaks/src/model.rs`
- Create: `crates/winfreeup-tweaks/src/ops.rs`
- Create: `crates/winfreeup-tweaks/src/fake.rs`
- Create (file khung, một dòng `//!`): `crates/winfreeup-tweaks/src/{blocklist,catalog,undo,state,engine}.rs`
- Create: `crates/winfreeup-tweaks/src/sys_windows/mod.rs`
- Create (file khung): `crates/winfreeup-tweaks/src/sys_windows/{appx,info,real,registry,services,shell,tasks}.rs`
- Test: `model.rs`, `ops.rs` (module `tests`)

**Interfaces:**
- Consumes: không có (crate độc lập, không phụ thuộc `winfreeup-core`).
- Produces (mọi task sau dùng nguyên văn):
  - `model`: `enum Group { Privacy, Bloatware }`, `enum Level { Basic, Recommended, Aggressive }` (có `Ord`), `enum Risk { Safe, Caution }`, `enum Restart { None, Explorer, Logoff, Reboot }` (`Default = None`, có `Ord`), `enum StartType { Disabled, Manual, Auto }`, `enum RegType { Dword, Qword, Sz }`, `enum RegValue { Int(i64), Str(String) }` (untagged), `const ABSENT: &str = "absent"`, `struct WindowsReq { min_build: u32, max_build: u32, editions: Vec<String> }`, `enum Op` (serde tag `kind`: `RegistrySet { path, name, value_type (toml: type), value: RegValue, default: RegValue }`, `RegistryDelete { path, name }`, `ServiceStartup { service, start: StartType, default: StartType }`, `ScheduledTaskDisable { task }`, `AppxRemove { package_family, store_product_id }`), `struct Tweak { id, group, level, risk, windows, needs_restart, ops }`, `fn parse_catalog(text: &str) -> Result<Vec<Tweak>, String>`, `fn to_reg_data(RegType, &RegValue) -> Result<RegData, String>`, `fn default_data(RegType, &RegValue) -> Result<Option<RegData>, String>` (`None` = absent). Mọi kiểu serde `lowercase`/`snake_case`; `deny_unknown_fields`.
  - `ops`: `enum RegData { Dword(u32), Qword(u64), Sz(String) }` (JSON `{"type":"dword","value":1}`), `struct PackageInfo { full_name, family, is_framework: bool, non_removable: bool }`, `struct SystemInfo { build: u32, edition: String, managed: bool, other_user: bool }`, `fn edition_of(edition_id: &str) -> &'static str` (`Home|Pro|Enterprise|Education|Other`), `trait TweakOps: Send + Sync` (14 hàm — xem mã; lỗi là `String` nguyên văn).
  - `#[cfg(test)] fake::FakeOps` (`Default`: build 26100, edition `Pro`, không managed) + builder `with_reg(path, name, RegData)`, `with_service(name, StartType)`, `with_task(path, bool)`, `with_package(family)` (full name = `<family>!1`), `failing("<hàm>:<khoá>")`; `get(path, name) -> Option<RegData>`; `calls() -> Vec<String>` ghi `reg_write:<path>|<name>`, `reg_delete:<path>|<name>`, `set_service_start:<name>:<Disabled|Manual|Auto>`, `stop_service:<name>`, `set_task_enabled:<path>:<bool>`, `remove_package:<full>:<all_users>`, `deprovision:<family>`, `open_uri:<uri>`, `restart_explorer`. `failing("reg_read:<path>|<name>")` làm `reg_read` lỗi (không ghi vào `calls`).
  - `lib.rs` khai báo sẵn `blocklist, catalog, engine, model, ops, state, sys_windows, undo` và `#[cfg(test)] fake`; `sys_windows/mod.rs` khai báo sẵn `appx, info, real, registry, services, shell, tasks` (cả module chỉ biên dịch trên Windows). Không task nào sau sửa hai file này hay `Cargo.toml`.

- [ ] **Step 1: Tạo crate và các file khung**

`crates/winfreeup-tweaks/Cargo.toml` (bảng `[workspace]` rỗng giữ crate NGOÀI workspace gốc tới Task 12 — `Cargo.toml` gốc là của v0.1):

```toml
[package]
name = "winfreeup-tweaks"
version = "0.1.0"
edition = "2021"

# Tách khỏi workspace gốc cho tới Task 12 (Tích hợp) — Cargo.toml gốc do v0.1 sở hữu. Task 12 xoá bảng này.
[workspace]

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "1"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.62", features = [
  "ApplicationModel",
  "Foundation",
  "Foundation_Collections",
  "Management_Deployment",
  "Win32_Foundation",
  "Win32_NetworkManagement_NetManagement",
  "Win32_Security",
  "Win32_System_Com",
  "Win32_System_Ole",
  "Win32_System_Registry",
  "Win32_System_RemoteDesktop",
  "Win32_System_Services",
  "Win32_System_TaskScheduler",
  "Win32_System_Variant",
  "Win32_System_WindowsProgramming",
  "Win32_UI_Shell",
  "Win32_UI_WindowsAndMessaging",
] }
windows-future = "0.3"

[dev-dependencies]
tempfile = "3"
```

`crates/winfreeup-tweaks/src/lib.rs`:

```rust
//! Tinh chỉnh Windows: gỡ app cài sẵn và tắt quảng cáo/thu thập dữ liệu.
//! Mọi thao tác chạm hệ thống đi qua trait `TweakOps`; lõi không chứa chữ hiển thị.

pub mod blocklist;
pub mod catalog;
pub mod engine;
pub mod model;
pub mod ops;
pub mod state;
pub mod sys_windows;
pub mod undo;

#[cfg(test)]
pub(crate) mod fake;

pub use model::{Group, Level, Op, RegType, RegValue, Restart, Risk, StartType, Tweak, WindowsReq};
pub use ops::{PackageInfo, RegData, SystemInfo, TweakOps};
```

`crates/winfreeup-tweaks/src/sys_windows/mod.rs`:

```rust
//! Lời gọi Windows thật cho phần Tinh chỉnh. Mỗi mảng một file; `real::RealTweakOps` chỉ chuyển tiếp.
#![cfg(windows)]

pub mod appx;
pub mod info;
pub mod real;
pub mod registry;
pub mod services;
pub mod shell;
pub mod tasks;
```

File khung — mỗi file đúng MỘT dòng, task chủ sở hữu thay toàn bộ:

| File (từ `crates/winfreeup-tweaks/src/`) | Dòng khung |
|---|---|
| `blocklist.rs` | `//! Danh sách cấm gỡ — nội dung ở Task 2.` |
| `catalog.rs` | `//! Danh mục nhúng và luật kiểm — nội dung ở Task 2.` |
| `undo.rs` | `//! Ảnh chụp hoàn tác — nội dung ở Task 3.` |
| `state.rs` | `//! Trạng thái một mục — nội dung ở Task 4.` |
| `engine.rs` | `//! Đọc, áp dụng, hoàn tác — nội dung ở Task 5.` |
| `sys_windows/registry.rs`, `info.rs`, `shell.rs` | `//! Windows thật — nội dung ở Task 6.` |
| `sys_windows/services.rs`, `tasks.rs` | `//! Windows thật — nội dung ở Task 7.` |
| `sys_windows/appx.rs`, `real.rs` | `//! Windows thật — nội dung ở Task 8.` |

- [ ] **Step 2: Viết test hỏng cho `model.rs` và `ops.rs`**

`crates/winfreeup-tweaks/src/model.rs` — chỉ phần test (mã viết ở Step 4):

```rust
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
```

`crates/winfreeup-tweaks/src/ops.rs` — chỉ phần test:

```rust
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
```

- [ ] **Step 3: Chạy test, thấy hỏng**

Run: `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml`
Expected: FAIL biên dịch — `cannot find function parse_catalog`, `cannot find type RegData` (và `fake.rs` chưa có). Lần chạy đầu tải và biên dịch crate `windows` 0.62 — khoảng 1–2 phút.

- [ ] **Step 4: Viết mã (đặt TRÊN khối `#[cfg(test)]` của từng file) và `fake.rs`**

`model.rs`:

```rust
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
```

`ops.rs`:

```rust
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
```

`crates/winfreeup-tweaks/src/fake.rs` (dùng từ Task 4 trở đi):

```rust
//! Hệ thống giả cho test: registry, dịch vụ, tác vụ, gói app trong bộ nhớ. Không chạm máy thật.
#![allow(dead_code)]
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use crate::model::StartType;
use crate::ops::{PackageInfo, RegData, SystemInfo, TweakOps};

pub struct FakeOps {
    pub reg: Mutex<HashMap<(String, String), RegData>>,
    pub services: Mutex<HashMap<String, StartType>>,
    pub tasks: Mutex<HashMap<String, bool>>,
    pub packages: Mutex<Vec<PackageInfo>>,
    pub sys: SystemInfo,
    /// Tên thao tác sẽ hỏng, dạng `"<hàm>:<khoá>"`, vd `"reg_write:HKLM\X|V"`, `"remove_package:A_1"`.
    pub fail: HashSet<String>,
    pub calls: Mutex<Vec<String>>,
}

impl Default for FakeOps {
    fn default() -> Self {
        FakeOps {
            reg: Mutex::default(),
            services: Mutex::default(),
            tasks: Mutex::default(),
            packages: Mutex::default(),
            sys: SystemInfo { build: 26100, edition: "Pro".into(), managed: false, other_user: false },
            fail: HashSet::new(),
            calls: Mutex::default(),
        }
    }
}

fn key(path: &str, name: &str) -> (String, String) {
    (path.to_ascii_lowercase(), name.to_ascii_lowercase())
}

impl FakeOps {
    pub fn with_reg(self, path: &str, name: &str, data: RegData) -> Self {
        self.reg.lock().unwrap().insert(key(path, name), data);
        self
    }
    pub fn with_service(self, name: &str, start: StartType) -> Self {
        self.services.lock().unwrap().insert(name.to_ascii_lowercase(), start);
        self
    }
    pub fn with_task(self, path: &str, enabled: bool) -> Self {
        self.tasks.lock().unwrap().insert(path.to_ascii_lowercase(), enabled);
        self
    }
    /// Gói bình thường của người dùng: full name = `<family>!1`.
    pub fn with_package(self, family: &str) -> Self {
        self.packages.lock().unwrap().push(PackageInfo {
            full_name: format!("{family}!1"),
            family: family.into(),
            is_framework: false,
            non_removable: false,
        });
        self
    }
    pub fn failing(mut self, what: &str) -> Self {
        self.fail.insert(what.to_string());
        self
    }
    pub fn get(&self, path: &str, name: &str) -> Option<RegData> {
        self.reg.lock().unwrap().get(&key(path, name)).cloned()
    }
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    fn call(&self, what: String) -> Result<(), String> {
        self.calls.lock().unwrap().push(what.clone());
        if self.fail.contains(&what) {
            Err(format!("fake failure: {what}"))
        } else {
            Ok(())
        }
    }
}

impl TweakOps for FakeOps {
    fn reg_read(&self, path: &str, name: &str) -> Result<Option<RegData>, String> {
        if self.fail.contains(&format!("reg_read:{path}|{name}")) {
            return Err(format!("fake failure: reg_read:{path}|{name}"));
        }
        Ok(self.get(path, name))
    }
    fn reg_write(&self, path: &str, name: &str, data: &RegData) -> Result<(), String> {
        self.call(format!("reg_write:{path}|{name}"))?;
        self.reg.lock().unwrap().insert(key(path, name), data.clone());
        Ok(())
    }
    fn reg_delete(&self, path: &str, name: &str) -> Result<(), String> {
        self.call(format!("reg_delete:{path}|{name}"))?;
        self.reg.lock().unwrap().remove(&key(path, name));
        Ok(())
    }
    fn service_start(&self, name: &str) -> Result<Option<StartType>, String> {
        Ok(self.services.lock().unwrap().get(&name.to_ascii_lowercase()).copied())
    }
    fn set_service_start(&self, name: &str, start: StartType) -> Result<(), String> {
        self.call(format!("set_service_start:{name}:{start:?}"))?;
        self.services.lock().unwrap().insert(name.to_ascii_lowercase(), start);
        Ok(())
    }
    fn stop_service(&self, name: &str) -> Result<(), String> {
        self.call(format!("stop_service:{name}"))
    }
    fn task_enabled(&self, path: &str) -> Result<Option<bool>, String> {
        Ok(self.tasks.lock().unwrap().get(&path.to_ascii_lowercase()).copied())
    }
    fn set_task_enabled(&self, path: &str, enabled: bool) -> Result<(), String> {
        self.call(format!("set_task_enabled:{path}:{enabled}"))?;
        self.tasks.lock().unwrap().insert(path.to_ascii_lowercase(), enabled);
        Ok(())
    }
    fn packages(&self, family: &str) -> Result<Vec<PackageInfo>, String> {
        Ok(self.packages.lock().unwrap().iter().filter(|p| p.family.eq_ignore_ascii_case(family)).cloned().collect())
    }
    fn remove_package(&self, full_name: &str, all_users: bool) -> Result<(), String> {
        self.call(format!("remove_package:{full_name}:{all_users}"))?;
        self.packages.lock().unwrap().retain(|p| p.full_name != full_name);
        Ok(())
    }
    fn deprovision(&self, family: &str) -> Result<(), String> {
        self.call(format!("deprovision:{family}"))
    }
    fn open_uri(&self, uri: &str) -> Result<(), String> {
        self.call(format!("open_uri:{uri}"))
    }
    fn system_info(&self) -> Result<SystemInfo, String> {
        Ok(self.sys.clone())
    }
    fn restart_explorer(&self) -> Result<(), String> {
        self.call("restart_explorer".into())
    }
}
```

- [ ] **Step 5: Chạy test và clippy, thấy qua**

Run: `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml` rồi `cargo clippy --manifest-path crates/winfreeup-tweaks/Cargo.toml --all-targets -- -D warnings`
Expected: `test result: ok. 7 passed; 0 failed`; clippy không cảnh báo.

- [ ] **Step 6: Commit**

```bash
rtk git add crates/winfreeup-tweaks/Cargo.toml
rtk git add crates/winfreeup-tweaks/Cargo.lock
rtk git add crates/winfreeup-tweaks/src/lib.rs
rtk git add crates/winfreeup-tweaks/src/model.rs
rtk git add crates/winfreeup-tweaks/src/ops.rs
rtk git add crates/winfreeup-tweaks/src/fake.rs
rtk git add crates/winfreeup-tweaks/src/blocklist.rs
rtk git add crates/winfreeup-tweaks/src/catalog.rs
rtk git add crates/winfreeup-tweaks/src/undo.rs
rtk git add crates/winfreeup-tweaks/src/state.rs
rtk git add crates/winfreeup-tweaks/src/engine.rs
rtk git add crates/winfreeup-tweaks/src/sys_windows/mod.rs
rtk git add crates/winfreeup-tweaks/src/sys_windows/appx.rs
rtk git add crates/winfreeup-tweaks/src/sys_windows/info.rs
rtk git add crates/winfreeup-tweaks/src/sys_windows/real.rs
rtk git add crates/winfreeup-tweaks/src/sys_windows/registry.rs
rtk git add crates/winfreeup-tweaks/src/sys_windows/services.rs
rtk git add crates/winfreeup-tweaks/src/sys_windows/shell.rs
rtk git add crates/winfreeup-tweaks/src/sys_windows/tasks.rs
rtk git commit -m "feat(tweaks): khung crate winfreeup-tweaks — kiểu danh mục, trait TweakOps, FakeOps

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Danh sách cấm gỡ, danh mục v1 và luật kiểm danh mục

**Files:**
- Modify: `crates/winfreeup-tweaks/src/blocklist.rs` (thay dòng khung)
- Modify: `crates/winfreeup-tweaks/src/catalog.rs` (thay dòng khung)
- Create: `crates/winfreeup-tweaks/catalog.toml`
- Create: `src/features/tinh-chinh/catalog.vi.json`
- Test: `blocklist.rs`, `catalog.rs` (module `tests`)

**Interfaces:**
- Consumes (Task 1): `parse_catalog(text: &str) -> Result<Vec<Tweak>, String>`, `to_reg_data`, `default_data`, `Op::{RegistrySet, RegistryDelete, ServiceStartup, ScheduledTaskDisable, AppxRemove}`, `Group`, `Level`, `Tweak { id, group, level, windows: WindowsReq { min_build, max_build, editions }, ops, .. }`.
- Produces:
  - `blocklist::{BLOCKED_PACKAGES, FORBIDDEN_REG_FRAGMENTS, FORBIDDEN_SERVICES}`, `package_name(family: &str) -> &str` (phần trước `_` cuối), `is_blocked_package(name_or_family: &str) -> bool` (không phân biệt hoa thường; mẫu có `*` cuối = tiền tố), `is_forbidden_reg_path(path: &str) -> bool`, `is_forbidden_service(name: &str) -> bool`.
  - `catalog::CATALOG_TOML: &str` (`include_str!("../catalog.toml")`), `builtin() -> Result<Vec<Tweak>, String>`, `name_key(id) -> String` (= `tweaks.item.<id>.name`), `desc_key(id) -> String` (= `tweaks.item.<id>.desc`), `validate(catalog: &[Tweak], has_key: &dyn Fn(&str) -> bool) -> Vec<String>` (rỗng = hợp lệ), `is_product_id(s: &str) -> bool`.
  - `catalog.toml`: 48 mục — 20 `privacy`, 28 `bloatware`; `id` dùng ở giao diện (Task 9–11) và `testdata.ts`: `ads_id`, `svc_diagtrack`, `recall_off`, `cloud_clipboard`, `app_clipchamp`, `app_weather`, `app_game_bar`, `app_tiktok` (và các id còn lại trong file).
  - `src/features/tinh-chinh/catalog.vi.json`: phẳng, khoá `tweaks.item.<id>.name` / `.desc` cho đủ 48 mục (Task 9 đọc; Task 12 trộn vào `src/i18n/vi.json`).

Luật kiểm (spec mục 7 + Global Constraints): `id` duy nhất và `[a-z0-9_]+`; đủ chuỗi tên/mô tả; `min_build ≤ max_build` khi `max_build ≠ 0`; `editions` ∈ `* Home Pro Enterprise Education`; mục `bloatware` chỉ chứa `appx_remove` và ngược lại; value/default đúng kiểu (thiếu `default` ⇒ `parse_catalog` đã báo lỗi); đường dẫn bắt đầu `HKCU\`/`HKLM\` và không chạm Defender/Windows Update/SmartScreen/Firewall; dịch vụ không thuộc danh sách cấm đụng; `package_family` có dạng `Name_PublisherId`, không khớp danh sách cấm gỡ; `store_product_id` là ProductId hợp lệ (12 ký tự `A-Z0-9`, hoặc 14 ký tự bắt đầu `XP` — app Win32 trên Store).

- [ ] **Step 1: Viết test hỏng**

`crates/winfreeup-tweaks/src/blocklist.rs` — chỉ phần test:

```rust
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
```

`crates/winfreeup-tweaks/src/catalog.rs` — chỉ phần test:

```rust
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

    #[test]
    fn group_and_op_kind_must_agree() {
        let e = one(&format!("{HEAD}ops = [{{ kind = \"scheduled_task_disable\", task = '\\A\\B' }}]"));
        assert!(e.iter().any(|m| m.contains("bloatware tweaks hold only")), "{e:?}");
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml -- blocklist:: catalog::`
Expected: FAIL biên dịch — `cannot find function is_blocked_package`, `couldn't read ...catalog.vi.json`.

- [ ] **Step 3: Viết mã `blocklist.rs` và `catalog.rs` (TRÊN khối test)**

`blocklist.rs`:

```rust
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
```

`catalog.rs`:

```rust
//! Danh mục nhúng vào exe và luật kiểm danh mục (spec mục 7 «Danh mục»). Danh mục sai ⇒ test đỏ ⇒ CI đỏ.
use std::collections::HashSet;

use crate::blocklist::{is_blocked_package, is_forbidden_reg_path, is_forbidden_service};
use crate::model::{default_data, parse_catalog, to_reg_data, Group, Op, Tweak};

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
                    if let Err(e) = to_reg_data(*value_type, value) {
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
```

- [ ] **Step 4: Viết danh mục `crates/winfreeup-tweaks/catalog.toml`**

Nguồn: bảng spec mục 4.1–4.2; giá trị `default` lấy theo giá trị đo trên máy dev (xem «Xác minh danh mục» cuối kế hoạch) — value vắng trên máy sạch ⇒ `"absent"`. ProductId/PackageFamily chưa có trên máy dev được ghi rõ ở bảng xác minh là **phải thử tay**.

```toml
# Danh mục Tinh chỉnh v1 — spec mục 4. Nhúng vào exe (include_str!). Mỗi mục cần chuỗi
# `tweaks.item.<id>.name` và `.desc` trong file tiếng Việt; test `builtin_catalog_is_valid` chặn danh mục sai.
# Bảng «Xác minh danh mục» trong kế hoạch ghi mục nào đã xác minh (máy thật / API Store), mục nào còn phải thử tay.
# Không có «Teams cá nhân» (MicrosoftTeams): Microsoft đã bỏ app này, không còn trang Store để cài lại ⇒ không hoàn tác được — xem «Câu hỏi còn mở» trong kế hoạch.

# ───────────── Quyền riêng tư & quảng cáo — Cơ bản ─────────────

[[tweak]]
id = "ads_id"
group = "privacy"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [
  { kind = "registry_set", path = 'HKCU\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo', name = "Enabled", type = "dword", value = 0, default = 1 },
]

[[tweak]]
id = "ads_start_suggestions"
group = "privacy"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [
  { kind = "registry_set", path = 'HKCU\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager', name = "SystemPaneSuggestionsEnabled", type = "dword", value = 0, default = 1 },
  { kind = "registry_set", path = 'HKCU\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager', name = "SubscribedContent-338388Enabled", type = "dword", value = 0, default = "absent" },
  { kind = "registry_set", path = 'HKCU\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager', name = "SubscribedContent-338389Enabled", type = "dword", value = 0, default = 1 },
  { kind = "registry_set", path = 'HKCU\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager', name = "SubscribedContent-338387Enabled", type = "dword", value = 0, default = "absent" },
  { kind = "registry_set", path = 'HKCU\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager', name = "SubscribedContent-353694Enabled", type = "dword", value = 0, default = "absent" },
  { kind = "registry_set", path = 'HKCU\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager', name = "SubscribedContent-353696Enabled", type = "dword", value = 0, default = "absent" },
  { kind = "registry_set", path = 'HKCU\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager', name = "RotatingLockScreenOverlayEnabled", type = "dword", value = 0, default = 1 },
  { kind = "registry_set", path = 'HKCU\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager', name = "SilentInstalledAppsEnabled", type = "dword", value = 0, default = 1 },
]

[[tweak]]
id = "ads_explorer"
group = "privacy"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
needs_restart = "explorer"
ops = [
  { kind = "registry_set", path = 'HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced', name = "ShowSyncProviderNotifications", type = "dword", value = 0, default = "absent" },
]

[[tweak]]
id = "tailored_experiences"
group = "privacy"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [
  { kind = "registry_set", path = 'HKCU\Software\Microsoft\Windows\CurrentVersion\Privacy', name = "TailoredExperiencesWithDiagnosticDataEnabled", type = "dword", value = 0, default = 1 },
]

[[tweak]]
id = "scoobe"
group = "privacy"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [
  { kind = "registry_set", path = 'HKCU\Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement', name = "ScoobeSystemSettingEnabled", type = "dword", value = 0, default = "absent" },
]

# ───────────── Quyền riêng tư & quảng cáo — Khuyến nghị ─────────────

[[tweak]]
id = "telemetry_min_security"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 19041, editions = ["Enterprise", "Education"] }
ops = [
  { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\Microsoft\Windows\DataCollection', name = "AllowTelemetry", type = "dword", value = 0, default = "absent" },
]

[[tweak]]
id = "telemetry_min_required"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 19041, editions = ["Home", "Pro"] }
ops = [
  { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\Microsoft\Windows\DataCollection', name = "AllowTelemetry", type = "dword", value = 1, default = "absent" },
]

[[tweak]]
id = "svc_diagtrack"
group = "privacy"
level = "recommended"
risk = "caution"
windows = { min_build = 19041 }
ops = [
  { kind = "service_startup", service = "DiagTrack", start = "disabled", default = "auto" },
]

[[tweak]]
id = "search_no_bing"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 19041 }
needs_restart = "explorer"
ops = [
  { kind = "registry_set", path = 'HKCU\Software\Policies\Microsoft\Windows\Explorer', name = "DisableSearchBoxSuggestions", type = "dword", value = 1, default = "absent" },
]

[[tweak]]
id = "widgets_feed_win11"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 22000 }
needs_restart = "explorer"
ops = [
  { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\Microsoft\Dsh', name = "AllowNewsAndInterests", type = "dword", value = 0, default = "absent" },
]

[[tweak]]
id = "widgets_feed_win10"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 19041, max_build = 19045 }
needs_restart = "explorer"
ops = [
  { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\Microsoft\Windows\Windows Feeds', name = "EnableFeeds", type = "dword", value = 0, default = "absent" },
]

[[tweak]]
id = "activity_history"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 19041 }
ops = [
  { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\Microsoft\Windows\System', name = "PublishUserActivities", type = "dword", value = 0, default = "absent" },
  { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\Microsoft\Windows\System', name = "UploadUserActivities", type = "dword", value = 0, default = "absent" },
]

[[tweak]]
id = "inking_typing"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 19041 }
ops = [
  { kind = "registry_set", path = 'HKCU\Software\Microsoft\Input\TIPC', name = "Enabled", type = "dword", value = 0, default = 1 },
  { kind = "registry_set", path = 'HKCU\Software\Microsoft\InputPersonalization', name = "RestrictImplicitInkCollection", type = "dword", value = 1, default = 0 },
  { kind = "registry_set", path = 'HKCU\Software\Microsoft\InputPersonalization', name = "RestrictImplicitTextCollection", type = "dword", value = 1, default = 0 },
]

[[tweak]]
id = "ceip_tasks"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 19041 }
ops = [
  { kind = "scheduled_task_disable", task = '\Microsoft\Windows\Customer Experience Improvement Program\Consolidator' },
  { kind = "scheduled_task_disable", task = '\Microsoft\Windows\Customer Experience Improvement Program\UsbCeip' },
]

[[tweak]]
id = "do_lan_only"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 19041 }
ops = [
  { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization', name = "DODownloadMode", type = "dword", value = 1, default = "absent" },
]

[[tweak]]
id = "recall_off"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 26100 }
needs_restart = "reboot"
ops = [
  { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\Microsoft\Windows\WindowsAI', name = "DisableAIDataAnalysis", type = "dword", value = 1, default = "absent" },
]

# ───────────── Quyền riêng tư & quảng cáo — Triệt để ─────────────

[[tweak]]
id = "task_appraiser"
group = "privacy"
level = "aggressive"
risk = "caution"
windows = { min_build = 19041 }
ops = [
  { kind = "scheduled_task_disable", task = '\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser' },
  { kind = "scheduled_task_disable", task = '\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser Exp' },
]

[[tweak]]
id = "svc_dmwappush"
group = "privacy"
level = "aggressive"
risk = "caution"
windows = { min_build = 19041 }
ops = [
  { kind = "service_startup", service = "dmwappushservice", start = "disabled", default = "manual" },
]

[[tweak]]
id = "cloud_clipboard"
group = "privacy"
level = "aggressive"
risk = "safe"
windows = { min_build = 19041 }
ops = [
  { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\Microsoft\Windows\System', name = "AllowCrossDeviceClipboard", type = "dword", value = 0, default = "absent" },
]

[[tweak]]
id = "settings_sync"
group = "privacy"
level = "aggressive"
risk = "safe"
windows = { min_build = 19041 }
ops = [
  { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\Microsoft\Windows\SettingSync', name = "DisableSettingSync", type = "dword", value = 2, default = "absent" },
  { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\Microsoft\Windows\SettingSync', name = "DisableSettingSyncUserOverride", type = "dword", value = 1, default = "absent" },
]

# ───────────── Gỡ app — Cơ bản ─────────────

[[tweak]]
id = "app_candy_crush"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [
  { kind = "appx_remove", package_family = "king.com.CandyCrushSaga_kgqvnymyfvs32", store_product_id = "9NBLGGH18846" },
  { kind = "appx_remove", package_family = "king.com.CandyCrushSodaSaga_kgqvnymyfvs32", store_product_id = "9NBLGGH1ZRPV" },
  { kind = "appx_remove", package_family = "king.com.CandyCrushFriends_kgqvnymyfvs32", store_product_id = "9PL3B0VQLQQ8" },
]

[[tweak]]
id = "app_tiktok"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "BytedancePte.Ltd.TikTok_6yccndn6064se", store_product_id = "9NH2GPH4JZS4" } ]

[[tweak]]
id = "app_instagram"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Facebook.InstagramBeta_8xx8rvfyw5nnt", store_product_id = "9NBLGGH5L9XT" } ]

[[tweak]]
id = "app_disney"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Disney.37853FC22B2CE_6rarf9sa4v8jt", store_product_id = "9NXQXXLFST89" } ]

[[tweak]]
id = "app_spotify"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "SpotifyAB.SpotifyMusic_zpdnekdrzrea0", store_product_id = "9NCBCSZSJRSB" } ]

[[tweak]]
id = "app_tips"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.Getstarted_8wekyb3d8bbwe", store_product_id = "9WZDNCRDTBJJ" } ]

[[tweak]]
id = "app_feedback_hub"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.WindowsFeedbackHub_8wekyb3d8bbwe", store_product_id = "9NBLGGH4R32N" } ]

[[tweak]]
id = "app_news"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.BingNews_8wekyb3d8bbwe", store_product_id = "9WZDNCRFHVFW" } ]

[[tweak]]
id = "app_office_hub"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.MicrosoftOfficeHub_8wekyb3d8bbwe", store_product_id = "9WZDNCRD29V9" } ]

[[tweak]]
id = "app_clipchamp"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Clipchamp.Clipchamp_yxz26nhyzhsrt", store_product_id = "9P1J8S7CCWWT" } ]

[[tweak]]
id = "app_power_automate"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.PowerAutomateDesktop_8wekyb3d8bbwe", store_product_id = "9NFTCH6J7FHV" } ]

[[tweak]]
id = "app_mixed_reality"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.MixedReality.Portal_8wekyb3d8bbwe", store_product_id = "9NG1H8B3ZC7M" } ]

[[tweak]]
id = "app_3d_viewer"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.Microsoft3DViewer_8wekyb3d8bbwe", store_product_id = "9NBLGGH42THS" } ]

[[tweak]]
id = "app_paint_3d"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.MSPaint_8wekyb3d8bbwe", store_product_id = "9NBLGGH5FV99" } ]

[[tweak]]
id = "app_skype"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.SkypeApp_kzf8qxf38zg5c", store_product_id = "9WZDNCRFJ364" } ]

[[tweak]]
id = "app_people"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.People_8wekyb3d8bbwe", store_product_id = "9NBLGGH10PG8" } ]

# ───────────── Gỡ app — Khuyến nghị ─────────────

[[tweak]]
id = "app_weather"
group = "bloatware"
level = "recommended"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.BingWeather_8wekyb3d8bbwe", store_product_id = "9WZDNCRFJ3Q2" } ]

[[tweak]]
id = "app_maps"
group = "bloatware"
level = "recommended"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.WindowsMaps_8wekyb3d8bbwe", store_product_id = "9WZDNCRDTBVB" } ]

[[tweak]]
id = "app_solitaire"
group = "bloatware"
level = "recommended"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.MicrosoftSolitaireCollection_8wekyb3d8bbwe", store_product_id = "9WZDNCRFHWD2" } ]

[[tweak]]
id = "app_copilot"
group = "bloatware"
level = "recommended"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.Copilot_8wekyb3d8bbwe", store_product_id = "9NHT9RB2F4HD" } ]

[[tweak]]
id = "app_widgets"
group = "bloatware"
level = "recommended"
risk = "safe"
windows = { min_build = 22000 }
ops = [ { kind = "appx_remove", package_family = "MicrosoftWindows.Client.WebExperience_cw5n1h2txyewy", store_product_id = "9MSSGKG348SP" } ]

[[tweak]]
id = "app_family"
group = "bloatware"
level = "recommended"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "MicrosoftCorporationII.MicrosoftFamily_8wekyb3d8bbwe", store_product_id = "9PDJDJS743XF" } ]

[[tweak]]
id = "app_cortana"
group = "bloatware"
level = "recommended"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.549981C3F5F10_8wekyb3d8bbwe", store_product_id = "9NFFX4SZZ23L" } ]

# ───────────── Gỡ app — Triệt để ─────────────

[[tweak]]
id = "app_xbox"
group = "bloatware"
level = "aggressive"
risk = "safe"
windows = { min_build = 19041 }
ops = [
  { kind = "appx_remove", package_family = "Microsoft.GamingApp_8wekyb3d8bbwe", store_product_id = "9MV0B5HZVK9Z" },
  { kind = "appx_remove", package_family = "Microsoft.XboxApp_8wekyb3d8bbwe", store_product_id = "9WZDNCRFJBD8" },
]

[[tweak]]
id = "app_game_bar"
group = "bloatware"
level = "aggressive"
risk = "caution"
windows = { min_build = 19041 }
ops = [
  { kind = "appx_remove", package_family = "Microsoft.XboxGamingOverlay_8wekyb3d8bbwe", store_product_id = "9NZKPSTSNW4P" },
  { kind = "appx_remove", package_family = "Microsoft.XboxGameOverlay_8wekyb3d8bbwe", store_product_id = "9NZKPSTSNW4P" },
  { kind = "appx_remove", package_family = "Microsoft.XboxSpeechToTextOverlay_8wekyb3d8bbwe", store_product_id = "9NZKPSTSNW4P" },
]

[[tweak]]
id = "app_phone_link"
group = "bloatware"
level = "aggressive"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.YourPhone_8wekyb3d8bbwe", store_product_id = "9NMPJ99VJBWV" } ]

[[tweak]]
id = "app_outlook_new"
group = "bloatware"
level = "aggressive"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "Microsoft.OutlookForWindows_8wekyb3d8bbwe", store_product_id = "9NRX63209R7B" } ]

[[tweak]]
id = "app_mail_calendar"
group = "bloatware"
level = "aggressive"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "microsoft.windowscommunicationsapps_8wekyb3d8bbwe", store_product_id = "9WZDNCRFHVQM" } ]
```

- [ ] **Step 5: Viết chuỗi tiếng Việt `src/features/tinh-chinh/catalog.vi.json`**

```json
{
  "tweaks.item.ads_id.name": "Tắt ID quảng cáo",
  "tweaks.item.ads_id.desc": "Ứng dụng không còn dùng mã riêng của máy để chọn quảng cáo cho bạn.",
  "tweaks.item.ads_start_suggestions.name": "Tắt gợi ý và quảng cáo trong Start, Cài đặt, màn hình khóa",
  "tweaks.item.ads_start_suggestions.desc": "Bỏ «ứng dụng được đề xuất», mẹo quảng cáo, và không để Windows tự cài app quảng cáo.",
  "tweaks.item.ads_explorer.name": "Tắt quảng cáo trong File Explorer",
  "tweaks.item.ads_explorer.desc": "Bỏ thông báo mời dùng dịch vụ đám mây trong cửa sổ thư mục. Explorer sẽ khởi động lại.",
  "tweaks.item.tailored_experiences.name": "Tắt trải nghiệm cá nhân hóa theo dữ liệu chẩn đoán",
  "tweaks.item.tailored_experiences.desc": "Microsoft không dùng dữ liệu chẩn đoán của máy để gợi ý và quảng cáo riêng cho bạn.",
  "tweaks.item.scoobe.name": "Tắt màn «Hãy hoàn tất thiết lập thiết bị»",
  "tweaks.item.scoobe.desc": "Không còn màn hình mời đăng ký dịch vụ Microsoft sau mỗi bản cập nhật lớn.",
  "tweaks.item.telemetry_min_security.name": "Hạ dữ liệu chẩn đoán xuống mức thấp nhất",
  "tweaks.item.telemetry_min_security.desc": "Chỉ gửi dữ liệu bảo mật tối thiểu (bản Enterprise/Education).",
  "tweaks.item.telemetry_min_required.name": "Hạ dữ liệu chẩn đoán xuống mức «Bắt buộc»",
  "tweaks.item.telemetry_min_required.desc": "Bản Home/Pro chỉ hạ được xuống «Bắt buộc», không tắt hẳn được.",
  "tweaks.item.svc_diagtrack.name": "Tắt dịch vụ gửi dữ liệu chẩn đoán (DiagTrack)",
  "tweaks.item.svc_diagtrack.desc": "Dừng dịch vụ Connected User Experiences and Telemetry. Một số mục trong Cài đặt › Quyền riêng tư có thể hiện «do tổ chức quản lý».",
  "tweaks.item.search_no_bing.name": "Bỏ kết quả Bing khỏi ô tìm kiếm Start",
  "tweaks.item.search_no_bing.desc": "Tìm kiếm trong Start chỉ tìm trên máy. Explorer sẽ khởi động lại.",
  "tweaks.item.widgets_feed_win11.name": "Tắt bảng tin Widgets",
  "tweaks.item.widgets_feed_win11.desc": "Bỏ tin tức và quảng cáo trong bảng Widgets của Windows 11.",
  "tweaks.item.widgets_feed_win10.name": "Tắt Tin tức và sở thích trên thanh tác vụ",
  "tweaks.item.widgets_feed_win10.desc": "Bỏ ô thời tiết/tin tức trên thanh tác vụ Windows 10.",
  "tweaks.item.activity_history.name": "Tắt lịch sử hoạt động",
  "tweaks.item.activity_history.desc": "Windows không ghi lại và không gửi danh sách việc bạn đã làm trên máy.",
  "tweaks.item.inking_typing.name": "Tắt gửi mẫu gõ phím và viết tay",
  "tweaks.item.inking_typing.desc": "Không gửi mẫu chữ bạn gõ và viết tay để «cải thiện» nhận dạng.",
  "tweaks.item.ceip_tasks.name": "Tắt tác vụ Chương trình Cải thiện Trải nghiệm",
  "tweaks.item.ceip_tasks.desc": "Tắt hai tác vụ định kỳ gom và gửi số liệu sử dụng máy.",
  "tweaks.item.do_lan_only.name": "Chỉ chia sẻ bản cập nhật trong mạng nhà",
  "tweaks.item.do_lan_only.desc": "Máy vẫn nhận bản cập nhật như thường, nhưng không tải lên cho máy lạ trên Internet.",
  "tweaks.item.recall_off.name": "Tắt Recall",
  "tweaks.item.recall_off.desc": "Windows không chụp màn hình định kỳ để tìm lại hoạt động. Cần khởi động lại máy.",
  "tweaks.item.task_appraiser.name": "Tắt tác vụ Compatibility Appraiser",
  "tweaks.item.task_appraiser.desc": "Tắt tác vụ quét ứng dụng để gửi báo cáo tương thích. Windows Update vẫn chạy bình thường.",
  "tweaks.item.svc_dmwappush.name": "Tắt dịch vụ dmwappushservice",
  "tweaks.item.svc_dmwappush.desc": "Dịch vụ định tuyến tin nhắn đẩy WAP. Máy cơ quan quản lý bằng MDM có thể cần dịch vụ này.",
  "tweaks.item.cloud_clipboard.name": "Tắt đồng bộ bộ nhớ tạm lên đám mây",
  "tweaks.item.cloud_clipboard.desc": "Nội dung bạn sao chép không được đưa lên đám mây sang máy khác.",
  "tweaks.item.settings_sync.name": "Tắt đồng bộ cài đặt giữa các máy",
  "tweaks.item.settings_sync.desc": "Cài đặt Windows (hình nền, mật khẩu Wi-Fi…) không đồng bộ qua tài khoản Microsoft.",
  "tweaks.item.app_candy_crush.name": "Candy Crush",
  "tweaks.item.app_candy_crush.desc": "Trò chơi quảng cáo cài sẵn (Saga, Soda Saga, Friends Saga).",
  "tweaks.item.app_tiktok.name": "TikTok",
  "tweaks.item.app_tiktok.desc": "Ứng dụng ghim sẵn trong Start.",
  "tweaks.item.app_instagram.name": "Instagram",
  "tweaks.item.app_instagram.desc": "Ứng dụng ghim sẵn trong Start.",
  "tweaks.item.app_disney.name": "Disney+",
  "tweaks.item.app_disney.desc": "Ứng dụng ghim sẵn trong Start.",
  "tweaks.item.app_spotify.name": "Spotify ghim sẵn",
  "tweaks.item.app_spotify.desc": "Bản Spotify Windows tự cài. Nếu bạn dùng Spotify thì bỏ chọn mục này.",
  "tweaks.item.app_tips.name": "Mẹo",
  "tweaks.item.app_tips.desc": "Ứng dụng Mẹo (Microsoft Tips).",
  "tweaks.item.app_feedback_hub.name": "Trung tâm Phản hồi",
  "tweaks.item.app_feedback_hub.desc": "Ứng dụng gửi góp ý cho Microsoft.",
  "tweaks.item.app_news.name": "Tin tức",
  "tweaks.item.app_news.desc": "Ứng dụng Microsoft Tin tức.",
  "tweaks.item.app_office_hub.name": "Microsoft 365 (Office) — ứng dụng quảng bá",
  "tweaks.item.app_office_hub.desc": "Chỉ là trang mời mua Microsoft 365. Word, Excel đã cài không bị ảnh hưởng.",
  "tweaks.item.app_clipchamp.name": "Clipchamp",
  "tweaks.item.app_clipchamp.desc": "Trình dựng video cài sẵn.",
  "tweaks.item.app_power_automate.name": "Power Automate",
  "tweaks.item.app_power_automate.desc": "Công cụ tự động hóa dành cho doanh nghiệp.",
  "tweaks.item.app_mixed_reality.name": "Mixed Reality Portal",
  "tweaks.item.app_mixed_reality.desc": "Chỉ cần khi dùng kính thực tế ảo Windows Mixed Reality.",
  "tweaks.item.app_3d_viewer.name": "3D Viewer",
  "tweaks.item.app_3d_viewer.desc": "Trình xem mô hình 3D.",
  "tweaks.item.app_paint_3d.name": "Paint 3D",
  "tweaks.item.app_paint_3d.desc": "Paint 3D cũ. Paint thường không bị gỡ.",
  "tweaks.item.app_skype.name": "Skype",
  "tweaks.item.app_skype.desc": "Skype đã ngừng hoạt động.",
  "tweaks.item.app_people.name": "Danh bạ (People)",
  "tweaks.item.app_people.desc": "Ứng dụng Danh bạ cũ của Windows 10.",
  "tweaks.item.app_weather.name": "Thời tiết",
  "tweaks.item.app_weather.desc": "Ứng dụng MSN Thời tiết.",
  "tweaks.item.app_maps.name": "Bản đồ",
  "tweaks.item.app_maps.desc": "Ứng dụng Bản đồ Windows.",
  "tweaks.item.app_solitaire.name": "Solitaire Collection",
  "tweaks.item.app_solitaire.desc": "Trò chơi bài có quảng cáo.",
  "tweaks.item.app_copilot.name": "Copilot",
  "tweaks.item.app_copilot.desc": "Ứng dụng Microsoft Copilot.",
  "tweaks.item.app_widgets.name": "Widgets",
  "tweaks.item.app_widgets.desc": "Bảng Widgets trên thanh tác vụ Windows 11.",
  "tweaks.item.app_family.name": "Microsoft Family",
  "tweaks.item.app_family.desc": "Ứng dụng quản lý gia đình. Nếu bạn dùng để giám sát con thì bỏ chọn mục này.",
  "tweaks.item.app_cortana.name": "Cortana",
  "tweaks.item.app_cortana.desc": "Trợ lý Cortana của Windows 10 (đã ngừng hỗ trợ).",
  "tweaks.item.app_xbox.name": "Xbox",
  "tweaks.item.app_xbox.desc": "Ứng dụng Xbox. Cần nếu bạn chơi game PC Game Pass.",
  "tweaks.item.app_game_bar.name": "Xbox Game Bar",
  "tweaks.item.app_game_bar.desc": "Mất phím Win+G và tính năng quay màn hình khi chơi game.",
  "tweaks.item.app_phone_link.name": "Liên kết Điện thoại",
  "tweaks.item.app_phone_link.desc": "Nếu bạn nhận tin nhắn/cuộc gọi điện thoại trên máy tính thì bỏ chọn mục này.",
  "tweaks.item.app_outlook_new.name": "Outlook (mới)",
  "tweaks.item.app_outlook_new.desc": "Outlook bản mới cài sẵn. Outlook trong bộ Office không bị ảnh hưởng.",
  "tweaks.item.app_mail_calendar.name": "Thư và Lịch",
  "tweaks.item.app_mail_calendar.desc": "Ứng dụng Thư và Lịch cũ (Microsoft đã ngừng hỗ trợ)."
}
```

- [ ] **Step 6: Chạy test và clippy, thấy qua**

Run: `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml -- blocklist:: catalog::` rồi `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml` rồi `cargo clippy --manifest-path crates/winfreeup-tweaks/Cargo.toml --all-targets -- -D warnings`
Expected: `9 passed` (3 blocklist + 6 catalog); toàn crate `0 failed`; clippy sạch. Nếu `builtin_catalog_is_valid` đỏ, thông điệp liệt kê từng vi phạm theo dạng `<id>#<chỉ số>: …` — sửa danh mục, không nới luật.

- [ ] **Step 7: Commit**

```bash
rtk git add crates/winfreeup-tweaks/src/blocklist.rs
rtk git add crates/winfreeup-tweaks/src/catalog.rs
rtk git add crates/winfreeup-tweaks/catalog.toml
rtk git add src/features/tinh-chinh/catalog.vi.json
rtk git commit -m "feat(tweaks): danh mục v1 (48 mục), danh sách cấm gỡ/cấm đụng, test chặn danh mục sai

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Kho ảnh chụp hoàn tác `tweaks-undo.json`

**Files:**
- Modify: `crates/winfreeup-tweaks/src/undo.rs` (thay dòng khung)
- Test: `undo.rs` (module `tests`)

**Interfaces:**
- Consumes (Task 1): `RegData`, `StartType` (serde). Crate dev `tempfile = "3"` đã khai báo ở Task 1.
- Produces:
  - `enum Snapshot` — serde tag `kind`: `Registry { data: Option<RegData> }` (`None` = trước đó không có value), `Service { start: StartType }`, `Task { enabled: bool }`, `Appx { family: String }`.
  - `const FILE_NAME: &str = "tweaks-undo.json"`, `fn default_path(local_appdata: &Path) -> PathBuf` (= `…\WinFreeUp\tweaks-undo.json`).
  - `struct UndoStore` + `UndoStore::load(path: &Path) -> (UndoStore, Option<String>)` (không có file ⇒ rỗng, `None`; file hỏng/sai phiên bản ⇒ đổi tên `.bak` (`.bak1`, `.bak2`… nếu đã có), kho rỗng, `Some(chi tiết nguyên văn)`), `path(&self) -> &Path`, `get(&self, id: &str, op: usize) -> Option<&Snapshot>`, `has_any(&self, id: &str) -> bool`, `record_if_absent(&mut self, id: &str, op: usize, snap: Snapshot) -> bool` (chỉ ghi lần đầu; `true` = vừa ghi), `remove(&mut self, id: &str, op: usize)`, `save(&self) -> Result<(), String>` (tạo thư mục, ghi `*.json.tmp` rồi `rename`).
  - Hình JSON: `{"version":1,"entries":{"<id>":{"<chỉ số>":{"kind":"service","start":"auto"}}}}`.

- [ ] **Step 1: Viết test hỏng**

`crates/winfreeup-tweaks/src/undo.rs` — chỉ phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn snap(v: u32) -> Snapshot {
        Snapshot::Registry { data: Some(RegData::Dword(v)) }
    }

    #[test]
    fn missing_file_is_empty_store() {
        let d = tempfile::tempdir().unwrap();
        let (s, warn) = UndoStore::load(&d.path().join("x.json"));
        assert!(warn.is_none());
        assert!(s.get("a", 0).is_none());
    }

    #[test]
    fn snapshot_is_written_only_the_first_time_and_survives_reload() {
        let d = tempfile::tempdir().unwrap();
        let p = default_path(d.path());
        let (mut s, _) = UndoStore::load(&p);
        assert!(s.record_if_absent("ads_id", 0, snap(1)));
        assert!(!s.record_if_absent("ads_id", 0, snap(0)), "áp dụng lại không đè ảnh chụp gốc");
        s.save().unwrap();
        let (s2, warn) = UndoStore::load(&p);
        assert!(warn.is_none());
        assert_eq!(s2.get("ads_id", 0), Some(&snap(1)));
        assert!(!p.with_extension("json.tmp").exists(), "không để lại file tạm");
    }

    #[test]
    fn remove_last_op_drops_the_tweak() {
        let d = tempfile::tempdir().unwrap();
        let (mut s, _) = UndoStore::load(&d.path().join("u.json"));
        s.record_if_absent("t", 0, snap(1));
        s.record_if_absent("t", 1, Snapshot::Registry { data: None });
        s.remove("t", 0);
        assert!(s.has_any("t"));
        s.remove("t", 1);
        assert!(!s.has_any("t"));
    }

    #[test]
    fn corrupt_file_is_renamed_to_bak_not_deleted() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("tweaks-undo.json");
        fs::write(&p, "{ not json").unwrap();
        let (s, warn) = UndoStore::load(&p);
        assert!(warn.unwrap().contains("tweaks-undo.json"));
        assert!(!s.has_any("x"));
        assert!(!p.exists());
        assert_eq!(fs::read_to_string(d.path().join("tweaks-undo.json.bak")).unwrap(), "{ not json");
        // Hỏng lần hai: không đè .bak cũ.
        fs::write(&p, "[]").unwrap();
        let _ = UndoStore::load(&p);
        assert!(d.path().join("tweaks-undo.json.bak1").exists());
        assert_eq!(fs::read_to_string(d.path().join("tweaks-undo.json.bak")).unwrap(), "{ not json");
    }

    #[test]
    fn json_shape_is_stable() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("u.json");
        let (mut s, _) = UndoStore::load(&p);
        s.record_if_absent("svc_diagtrack", 0, Snapshot::Service { start: StartType::Auto });
        s.save().unwrap();
        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["version"], 1);
        assert_eq!(v["entries"]["svc_diagtrack"]["0"]["kind"], "service");
        assert_eq!(v["entries"]["svc_diagtrack"]["0"]["start"], "auto");
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml -- undo::`
Expected: FAIL biên dịch — `cannot find type UndoStore`, `cannot find function default_path`.

- [ ] **Step 3: Viết mã (TRÊN khối test)**

```rust
//! Ảnh chụp giá trị cũ để hoàn tác (spec mục 3.3): `%LOCALAPPDATA%\WinFreeUp\tweaks-undo.json`.
//! Chỉ ghi lần đầu; ghi file tạm rồi đổi tên; file hỏng ⇒ đổi thành `.bak`, không xoá.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::StartType;
use crate::ops::RegData;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Snapshot {
    /// `data = None` ⇒ trước khi áp dụng value không tồn tại.
    Registry { data: Option<RegData> },
    Service { start: StartType },
    Task { enabled: bool },
    /// Gói đã có trên máy trước khi gỡ.
    Appx { family: String },
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct UndoFile {
    version: u32,
    /// tweak_id ⇒ (chỉ số thao tác ⇒ ảnh chụp).
    entries: BTreeMap<String, BTreeMap<usize, Snapshot>>,
}

pub const FILE_NAME: &str = "tweaks-undo.json";
const VERSION: u32 = 1;

pub struct UndoStore {
    path: PathBuf,
    file: UndoFile,
}

/// `%LOCALAPPDATA%\WinFreeUp\tweaks-undo.json`.
pub fn default_path(local_appdata: &Path) -> PathBuf {
    local_appdata.join("WinFreeUp").join(FILE_NAME)
}

/// Tên `.bak` chưa dùng: `x.json.bak`, rồi `x.json.bak1`, `x.json.bak2`…
fn free_backup_name(path: &Path) -> PathBuf {
    let base = format!("{}.bak", path.display());
    let mut candidate = PathBuf::from(&base);
    let mut n = 1;
    while candidate.exists() {
        candidate = PathBuf::from(format!("{base}{n}"));
        n += 1;
    }
    candidate
}

impl UndoStore {
    /// Không có file ⇒ kho rỗng. File hỏng ⇒ đổi tên thành `.bak`, kho rỗng, và trả thông điệp
    /// nguyên văn để giao diện hiện băng hổ phách `undo_corrupt`.
    pub fn load(path: &Path) -> (UndoStore, Option<String>) {
        let empty = || UndoStore { path: path.to_path_buf(), file: UndoFile { version: VERSION, ..Default::default() } };
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (empty(), None),
            Err(e) => return (empty(), Some(format!("{}: {e}", path.display()))),
        };
        match serde_json::from_str::<UndoFile>(&text) {
            Ok(file) if file.version == VERSION => (UndoStore { path: path.to_path_buf(), file }, None),
            other => {
                let detail = match other {
                    Ok(f) => format!("unsupported version {}", f.version),
                    Err(e) => e.to_string(),
                };
                let bak = free_backup_name(path);
                let moved = fs::rename(path, &bak).map(|_| bak.display().to_string()).unwrap_or_else(|e| format!("rename failed: {e}"));
                (empty(), Some(format!("{}: {detail} ⇒ {moved}", path.display())))
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn get(&self, id: &str, op: usize) -> Option<&Snapshot> {
        self.file.entries.get(id).and_then(|m| m.get(&op))
    }

    pub fn has_any(&self, id: &str) -> bool {
        self.file.entries.get(id).is_some_and(|m| !m.is_empty())
    }

    /// Chỉ ghi khi chưa có ảnh chụp cho (id, op). Trả `true` nếu vừa ghi.
    pub fn record_if_absent(&mut self, id: &str, op: usize, snap: Snapshot) -> bool {
        let m = self.file.entries.entry(id.to_string()).or_default();
        if m.contains_key(&op) {
            return false;
        }
        m.insert(op, snap);
        true
    }

    pub fn remove(&mut self, id: &str, op: usize) {
        if let Some(m) = self.file.entries.get_mut(id) {
            m.remove(&op);
            if m.is_empty() {
                self.file.entries.remove(id);
            }
        }
    }

    /// Ghi file tạm cạnh file thật rồi đổi tên (thay thế nguyên tử trên cùng ổ).
    pub fn save(&self) -> Result<(), String> {
        let dir = self.path.parent().ok_or_else(|| format!("{}: no parent", self.path.display()))?;
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let tmp = self.path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(&self.file).map_err(|e| e.to_string())?;
        fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
        fs::rename(&tmp, &self.path).map_err(|e| format!("{}: {e}", self.path.display()))
    }
}
```

- [ ] **Step 4: Chạy test và clippy, thấy qua**

Run: `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml -- undo::` rồi `cargo clippy --manifest-path crates/winfreeup-tweaks/Cargo.toml --all-targets -- -D warnings`
Expected: `5 passed`; clippy sạch.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-tweaks/src/undo.rs
rtk git commit -m "feat(tweaks): ảnh chụp hoàn tác — chỉ ghi lần đầu, ghi tạm rồi đổi tên, file hỏng thành .bak

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Trạng thái một mục, tính từ máy thật

**Files:**
- Modify: `crates/winfreeup-tweaks/src/state.rs` (thay dòng khung)
- Test: `state.rs` (module `tests`)

**Interfaces:**
- Consumes: Task 1 — `Tweak`, `Op`, `Group`, `to_reg_data`, `parse_catalog`, `StartType`, `RegData`, `SystemInfo { build, edition, managed, .. }`, `TweakOps` (`reg_read`, `service_start`, `task_enabled`, `packages`), `fake::FakeOps` (+ `with_reg`, `with_service`, `with_task`, `with_package`, `failing`, trường `sys`); Task 3 — `UndoStore::load`, `get`, `record_if_absent`, `Snapshot::{Registry, Appx}`.
- Produces:
  - `enum TweakStatus` — serde tag `status`: `Applied`, `NotApplied`, `Partial`, `Unsupported { reason: String }` (`build_min:<n>`, `build_max:<n>`, `edition`, `missing`), `Managed`, `NotPresent` ⇒ JSON `{"status":"not_applied"}`, `{"status":"unsupported","reason":"edition"}`. `fn actionable(&self) -> bool` (Applied/NotApplied/Partial).
  - `enum OpState { Match, Differ, Missing }`.
  - `fn supported(t: &Tweak, sys: &SystemInfo) -> Result<(), String>`.
  - `fn op_state(ops: &dyn TweakOps, op: &Op, had_snapshot: bool) -> Result<OpState, String>` — dịch vụ/tác vụ không có ⇒ `Missing`; gói: có ⇒ `Differ`, không có + có ảnh chụp ⇒ `Match`, không có + không ảnh chụp ⇒ `Missing`.
  - `fn tweak_status(t: &Tweak, sys: &SystemInfo, ops: &dyn TweakOps, undo: &UndoStore) -> (TweakStatus, Vec<String>)` — lỗi đọc dạng `<id>#<i>: <thông điệp>`, thao tác lỗi tính là `Differ`.

Luật (spec 3.2): sai build/edition ⇒ `Unsupported` — **trừ** app không có trên máy ⇒ `NotPresent` (Review Focus #2). `Managed` chỉ khi `sys.managed` **và** một `registry_set` dưới `\Policies\` đang có giá trị khác đích **và** chưa có ảnh chụp của WinFreeUp. Bỏ các thao tác `Missing`; không còn gì ⇒ `NotPresent` (bloatware) hoặc `Unsupported { reason: "missing" }` (privacy). Còn lại: mọi `Match` ⇒ `Applied`; mọi `Differ` ⇒ `NotApplied`; lẫn ⇒ `Partial`.

- [ ] **Step 1: Viết test hỏng**

`crates/winfreeup-tweaks/src/state.rs` — chỉ phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeOps;
    use crate::model::{parse_catalog, StartType};
    use crate::ops::RegData;
    use crate::undo::Snapshot;

    const CAT: &str = r#"
[[tweak]]
id = "two_values"
group = "privacy"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [
  { kind = "registry_set", path = 'HKCU\A', name = "x", type = "dword", value = 0, default = 1 },
  { kind = "registry_set", path = 'HKCU\A', name = "y", type = "dword", value = 0, default = "absent" },
]

[[tweak]]
id = "policy"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 19041, editions = ["Home", "Pro"] }
ops = [ { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\P', name = "v", type = "dword", value = 1, default = "absent" } ]

[[tweak]]
id = "recall"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 26100 }
ops = [ { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\R', name = "v", type = "dword", value = 1, default = "absent" } ]

[[tweak]]
id = "tasks"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "scheduled_task_disable", task = '\T\One' }, { kind = "scheduled_task_disable", task = '\T\Two' } ]

[[tweak]]
id = "svc"
group = "privacy"
level = "recommended"
risk = "caution"
windows = { min_build = 19041 }
ops = [ { kind = "service_startup", service = "DiagTrack", start = "disabled", default = "auto" } ]

[[tweak]]
id = "app"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "A.App_1", store_product_id = "9P1J8S7CCWWT" } ]
"#;

    fn cat() -> Vec<Tweak> {
        parse_catalog(CAT).unwrap()
    }
    fn tw(id: &str) -> Tweak {
        cat().into_iter().find(|t| t.id == id).unwrap()
    }
    fn empty_undo() -> (tempfile::TempDir, UndoStore) {
        let d = tempfile::tempdir().unwrap();
        let (u, _) = UndoStore::load(&d.path().join("u.json"));
        (d, u)
    }

    #[test]
    fn applied_not_applied_partial() {
        let (_d, u) = empty_undo();
        let t = tw("two_values");
        let f = FakeOps::default();
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::NotApplied);
        let f = FakeOps::default().with_reg(r"HKCU\A", "x", RegData::Dword(0));
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::Partial);
        let f = f.with_reg(r"HKCU\A", "y", RegData::Dword(0));
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::Applied);
    }

    #[test]
    fn wrong_build_or_edition_is_unsupported() {
        let (_d, u) = empty_undo();
        let mut f = FakeOps::default();
        f.sys.build = 22631;
        assert_eq!(tweak_status(&tw("recall"), &f.sys, &f, &u).0, TweakStatus::Unsupported { reason: "build_min:26100".into() });
        f.sys.edition = "Enterprise".into();
        assert_eq!(tweak_status(&tw("policy"), &f.sys, &f, &u).0, TweakStatus::Unsupported { reason: "edition".into() });
    }

    #[test]
    fn managed_only_when_org_policy_already_differs() {
        let (_d, mut u) = empty_undo();
        let t = tw("policy");
        let mut f = FakeOps::default().with_reg(r"HKLM\SOFTWARE\Policies\P", "v", RegData::Dword(3));
        // Máy cá nhân: giá trị lạ dưới Policies không khoá.
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::NotApplied);
        f.sys.managed = true;
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::Managed);
        // Máy quản lý nhưng value chưa có ⇒ vẫn cho áp dụng.
        let mut g = FakeOps::default();
        g.sys.managed = true;
        assert_eq!(tweak_status(&t, &g.sys, &g, &u).0, TweakStatus::NotApplied);
        // Chính WinFreeUp đã ghi (có ảnh chụp) ⇒ không coi là tổ chức quản lý.
        u.record_if_absent("policy", 0, Snapshot::Registry { data: None });
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::NotApplied);
    }

    #[test]
    fn missing_tasks_are_ignored_and_all_missing_is_unsupported() {
        let (_d, u) = empty_undo();
        let t = tw("tasks");
        let f = FakeOps::default().with_task(r"\T\One", false);
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::Applied, "tác vụ Two không có trên máy ⇒ bỏ qua");
        let f = FakeOps::default();
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::Unsupported { reason: "missing".into() });
        let f = FakeOps::default().with_service("DiagTrack", StartType::Auto);
        assert_eq!(tweak_status(&tw("svc"), &f.sys, &f, &u).0, TweakStatus::NotApplied);
    }

    #[test]
    fn app_hidden_unless_installed_or_removed_by_us() {
        let (_d, mut u) = empty_undo();
        let t = tw("app");
        let f = FakeOps::default();
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::NotPresent);
        let g = FakeOps::default().with_package("A.App_1");
        assert_eq!(tweak_status(&t, &g.sys, &g, &u).0, TweakStatus::NotApplied);
        u.record_if_absent("app", 0, Snapshot::Appx { family: "A.App_1".into() });
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::Applied, "đã gỡ bởi WinFreeUp ⇒ hiện «Đã gỡ»");
    }

    #[test]
    fn absent_app_on_unsupported_build_is_hidden_not_unsupported() {
        let (_d, u) = empty_undo();
        let mut t = tw("app");
        t.windows.min_build = 22000;
        let mut f = FakeOps::default();
        f.sys.build = 19045;
        assert_eq!(tweak_status(&t, &f.sys, &f, &u).0, TweakStatus::NotPresent);
        let mut g = FakeOps::default().with_package("A.App_1");
        g.sys.build = 19045;
        assert_eq!(tweak_status(&t, &g.sys, &g, &u).0, TweakStatus::Unsupported { reason: "build_min:22000".into() });
    }

    #[test]
    fn read_error_is_reported_verbatim_and_counts_as_differ() {
        let (_d, u) = empty_undo();
        let t = tw("two_values");
        let f = FakeOps::default().with_reg(r"HKCU\A", "y", RegData::Dword(0)).failing(r"reg_read:HKCU\A|x");
        let (s, errs) = tweak_status(&t, &f.sys, &f, &u);
        assert_eq!(s, TweakStatus::Partial);
        assert_eq!(errs, vec![r"two_values#0: fake failure: reg_read:HKCU\A|x".to_string()]);
    }

    #[test]
    fn status_json_shape() {
        assert_eq!(serde_json::to_string(&TweakStatus::NotApplied).unwrap(), r#"{"status":"not_applied"}"#);
        assert_eq!(
            serde_json::to_string(&TweakStatus::Unsupported { reason: "edition".into() }).unwrap(),
            r#"{"status":"unsupported","reason":"edition"}"#
        );
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml -- state::`
Expected: FAIL biên dịch — `cannot find function tweak_status`, `cannot find type TweakStatus`.

- [ ] **Step 3: Viết mã (TRÊN khối test)**

```rust
//! Trạng thái một mục — luôn tính từ máy thật (spec mục 3.2).
use serde::Serialize;

use crate::model::{to_reg_data, Group, Op, Tweak};
use crate::ops::{SystemInfo, TweakOps};
use crate::undo::UndoStore;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TweakStatus {
    Applied,
    NotApplied,
    Partial,
    /// `reason`: `build_min:<n>`, `build_max:<n>`, `edition`, `missing` (dịch vụ/tác vụ không có trên máy).
    Unsupported { reason: String },
    /// Máy do tổ chức quản lý và chính sách đã đặt giá trị khác — không cho áp dụng.
    Managed,
    /// Gói app không có trên máy và chưa từng bị WinFreeUp gỡ — giao diện ẩn mục này.
    NotPresent,
}

impl TweakStatus {
    /// Được phép áp dụng/hoàn tác.
    pub fn actionable(&self) -> bool {
        matches!(self, TweakStatus::Applied | TweakStatus::NotApplied | TweakStatus::Partial)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpState {
    Match,
    Differ,
    /// Thành phần không có trên máy (dịch vụ, tác vụ, gói chưa từng gỡ) — bỏ qua khi tính trạng thái.
    Missing,
}

/// `Err(reason)` nếu build/edition không hợp.
pub fn supported(t: &Tweak, sys: &SystemInfo) -> Result<(), String> {
    let w = &t.windows;
    if sys.build < w.min_build {
        return Err(format!("build_min:{}", w.min_build));
    }
    if w.max_build != 0 && sys.build > w.max_build {
        return Err(format!("build_max:{}", w.max_build));
    }
    if !w.editions.iter().any(|e| e == "*" || e.eq_ignore_ascii_case(&sys.edition)) {
        return Err("edition".into());
    }
    Ok(())
}

/// Trạng thái một thao tác so với đích. `had_snapshot` chỉ dùng cho `appx_remove`.
pub fn op_state(ops: &dyn TweakOps, op: &Op, had_snapshot: bool) -> Result<OpState, String> {
    Ok(match op {
        Op::RegistrySet { path, name, value_type, value, .. } => {
            let target = to_reg_data(*value_type, value)?;
            if ops.reg_read(path, name)? == Some(target) {
                OpState::Match
            } else {
                OpState::Differ
            }
        }
        Op::RegistryDelete { path, name } => {
            if ops.reg_read(path, name)?.is_none() {
                OpState::Match
            } else {
                OpState::Differ
            }
        }
        Op::ServiceStartup { service, start, .. } => match ops.service_start(service)? {
            None => OpState::Missing,
            Some(s) if s == *start => OpState::Match,
            Some(_) => OpState::Differ,
        },
        Op::ScheduledTaskDisable { task } => match ops.task_enabled(task)? {
            None => OpState::Missing,
            Some(false) => OpState::Match,
            Some(true) => OpState::Differ,
        },
        Op::AppxRemove { package_family, .. } => {
            if !ops.packages(package_family)?.is_empty() {
                OpState::Differ
            } else if had_snapshot {
                OpState::Match
            } else {
                OpState::Missing
            }
        }
    })
}

fn is_policy_path(path: &str) -> bool {
    path.to_ascii_lowercase().contains(r"\policies\")
}

/// Trạng thái cả mục và các lỗi đọc (nguyên văn). Lỗi đọc một thao tác ⇒ tính là `Differ`.
pub fn tweak_status(t: &Tweak, sys: &SystemInfo, ops: &dyn TweakOps, undo: &UndoStore) -> (TweakStatus, Vec<String>) {
    if let Err(reason) = supported(t, sys) {
        // App không có trên máy thì ẩn luôn, kể cả khi sai build — «không hỗ trợ» chỉ hiện cho thứ đang có.
        let absent = t.group == Group::Bloatware
            && t.ops.iter().enumerate().all(|(i, op)| matches!(op_state(ops, op, undo.get(&t.id, i).is_some()), Ok(OpState::Missing)));
        let status = if absent { TweakStatus::NotPresent } else { TweakStatus::Unsupported { reason } };
        return (status, vec![]);
    }
    let mut errors = Vec::new();
    let mut states = Vec::with_capacity(t.ops.len());
    for (i, op) in t.ops.iter().enumerate() {
        let had = undo.get(&t.id, i).is_some();
        if sys.managed && !had {
            if let Op::RegistrySet { path, name, value_type, value, .. } = op {
                if is_policy_path(path) {
                    match (ops.reg_read(path, name), to_reg_data(*value_type, value)) {
                        (Ok(Some(cur)), Ok(target)) if cur != target => return (TweakStatus::Managed, vec![]),
                        _ => {}
                    }
                }
            }
        }
        match op_state(ops, op, had) {
            Ok(s) => states.push(s),
            Err(e) => {
                errors.push(format!("{}#{i}: {e}", t.id));
                states.push(OpState::Differ);
            }
        }
    }
    let present: Vec<OpState> = states.into_iter().filter(|s| *s != OpState::Missing).collect();
    let status = if present.is_empty() {
        if t.group == Group::Bloatware {
            TweakStatus::NotPresent
        } else {
            TweakStatus::Unsupported { reason: "missing".into() }
        }
    } else if present.iter().all(|s| *s == OpState::Match) {
        TweakStatus::Applied
    } else if present.iter().all(|s| *s == OpState::Differ) {
        TweakStatus::NotApplied
    } else {
        TweakStatus::Partial
    };
    (status, errors)
}
```

- [ ] **Step 4: Chạy test và clippy, thấy qua**

Run: `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml -- state::` rồi `cargo clippy --manifest-path crates/winfreeup-tweaks/Cargo.toml --all-targets -- -D warnings`
Expected: `8 passed`; clippy sạch.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-tweaks/src/state.rs
rtk git commit -m "feat(tweaks): trạng thái mục tính từ máy thật — một phần, không hỗ trợ, tổ chức quản lý, app vắng

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Bộ máy — đọc, áp dụng, hoàn tác, nhật ký

**Files:**
- Modify: `crates/winfreeup-tweaks/src/engine.rs` (thay dòng khung)
- Test: `engine.rs` (module `tests`)

**Interfaces:**
- Consumes: Task 1 — `Tweak`, `Op`, `Group`, `Level`, `Risk`, `Restart` (`Ord`, `max`), `StartType`, `to_reg_data`, `default_data`, `parse_catalog`, `RegData`, `PackageInfo`, `SystemInfo`, `TweakOps`, `fake::FakeOps`; Task 2 — `blocklist::is_blocked_package`; Task 3 — `UndoStore::{load, get, has_any, record_if_absent, remove, save}`, `Snapshot`; Task 4 — `tweak_status`, `TweakStatus::{actionable, …}`.
- Produces (Task 6, 8 và vỏ Tauri ở Task 12 dùng nguyên văn):
  - `struct TweakView { id, group, level, risk, needs_restart, status (flatten ⇒ trường "status"/"reason" nằm ngang hàng), has_undo: bool, errors: Vec<String> }`.
  - `struct ReadResult { system: SystemInfo, tweaks: Vec<TweakView>, notices: Vec<String> }` — notice `undo_corrupt:<chi tiết>`.
  - `struct TweakOutcome { id, status (flatten), errors: Vec<String>, store_opened: Vec<String> }`, `struct RunReport { outcomes, restart: Restart, notices }`.
  - `enum TweakEvent` — serde tag `kind`: `Started { id, index, total }`, `Finished { outcome }`.
  - `fn read_all(catalog: &[Tweak], ops: &dyn TweakOps, undo_path: &Path) -> Result<ReadResult, String>` — `Err` chỉ khi `system_info` hỏng; KHÔNG tạo file hoàn tác.
  - `fn apply(catalog, ops, undo_path, ids: &[String], all_users: bool, on_event: &dyn Fn(&TweakEvent)) -> RunReport`.
  - `fn revert(catalog, ops, undo_path, ids: &[String], on_event: &dyn Fn(&TweakEvent)) -> RunReport`.
  - `fn log_lines(action: &str, r: &RunReport) -> Vec<String>` — `TWEAK <ACTION> <id> OK|ERRORS <status>`, `TWEAK ERROR <lỗi>`, `TWEAK STORE <ProductId>`, `TWEAK NOTICE <mã>`.
  - Mã lỗi trong `errors`: `<id>: unknown_id`, `<id>: not_allowed` (mục Unsupported/Managed/NotPresent), `<id>#<i>: undo_save: …`, `<id>#<i>: blocked:<family>`, `<id>#<i>: no_snapshot` (chỉ `registry_delete`), còn lại là lỗi Windows nguyên văn.

Luật: ảnh chụp **lưu xuống đĩa trước** khi chạm hệ thống; lưu hỏng ⇒ bỏ thao tác đó (Review Focus #4). Thao tác đã khớp đích vẫn chụp ảnh (để hoàn tác trả đúng giá trị người dùng đang có) nhưng không ghi. `service_startup` `disabled` ⇒ đổi kiểu rồi dừng (dừng hỏng chỉ ghi lỗi); hoàn tác chỉ đổi kiểu, không khởi động. Gói `IsFramework`/ký `System`/thuộc danh sách cấm ⇒ không gỡ dù danh mục ghi. `all_users` ⇒ `remove_package(.., true)` rồi `deprovision(family)`. Hoàn tác chạy ngược thứ tự thao tác; thao tác trả xong ⇒ xoá ảnh chụp của nó; app chưa cài lại ⇒ mở trang Store và **giữ** ảnh chụp. `restart` = mức nặng nhất trong các mục đã thực sự đổi máy.

- [ ] **Step 1: Viết test hỏng**

`crates/winfreeup-tweaks/src/engine.rs` — chỉ phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeOps;
    use crate::model::parse_catalog;
    use crate::ops::{PackageInfo, RegData};
    use std::sync::Mutex;

    const CAT: &str = r#"
[[tweak]]
id = "reg"
group = "privacy"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
needs_restart = "explorer"
ops = [
  { kind = "registry_set", path = 'HKCU\A', name = "x", type = "dword", value = 0, default = 1 },
  { kind = "registry_set", path = 'HKCU\A', name = "y", type = "dword", value = 0, default = "absent" },
]

[[tweak]]
id = "svc"
group = "privacy"
level = "recommended"
risk = "caution"
windows = { min_build = 19041 }
ops = [ { kind = "service_startup", service = "DiagTrack", start = "disabled", default = "auto" } ]

[[tweak]]
id = "task"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "scheduled_task_disable", task = '\T\One' } ]

[[tweak]]
id = "recall"
group = "privacy"
level = "recommended"
risk = "safe"
windows = { min_build = 26100 }
needs_restart = "reboot"
ops = [ { kind = "registry_set", path = 'HKLM\SOFTWARE\Policies\R', name = "v", type = "dword", value = 1, default = "absent" } ]

[[tweak]]
id = "app"
group = "bloatware"
level = "basic"
risk = "safe"
windows = { min_build = 19041 }
ops = [ { kind = "appx_remove", package_family = "A.App_1", store_product_id = "9P1J8S7CCWWT" } ]
"#;

    struct Env {
        _dir: tempfile::TempDir,
        undo: std::path::PathBuf,
        cat: Vec<Tweak>,
    }

    fn env() -> Env {
        let d = tempfile::tempdir().unwrap();
        let undo = d.path().join("WinFreeUp").join("tweaks-undo.json");
        Env { _dir: d, undo, cat: parse_catalog(CAT).unwrap() }
    }

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn quiet(_: &TweakEvent) {}

    #[test]
    fn apply_then_revert_restores_snapshot_and_deletes_absent_values() {
        let e = env();
        let f = FakeOps::default().with_reg(r"HKCU\A", "x", RegData::Dword(7));
        let r = apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &quiet);
        assert_eq!(r.outcomes[0].status, TweakStatus::Applied);
        assert!(r.outcomes[0].errors.is_empty(), "{:?}", r.outcomes[0].errors);
        assert_eq!(r.restart, Restart::Explorer);
        let r = revert(&e.cat, &f, &e.undo, &ids(&["reg"]), &quiet);
        assert!(r.outcomes[0].errors.is_empty());
        assert_eq!(f.get(r"HKCU\A", "x"), Some(RegData::Dword(7)), "trả về ảnh chụp, không phải default = 1");
        assert_eq!(f.get(r"HKCU\A", "y"), None, "ảnh chụp absent ⇒ xoá value");
        let (u, _) = UndoStore::load(&e.undo);
        assert!(!u.has_any("reg"), "hoàn tác thành công ⇒ xoá ảnh chụp");
    }

    #[test]
    fn reapply_does_not_overwrite_original_snapshot() {
        let e = env();
        let f = FakeOps::default().with_reg(r"HKCU\A", "x", RegData::Dword(7));
        apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &quiet);
        f.reg_write(r"HKCU\A", "x", &RegData::Dword(9)).unwrap(); // người dùng đổi tay giữa hai lần
        apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &quiet);
        revert(&e.cat, &f, &e.undo, &ids(&["reg"]), &quiet);
        assert_eq!(f.get(r"HKCU\A", "x"), Some(RegData::Dword(7)));
    }

    #[test]
    fn value_of_unexpected_type_is_snapshotted_and_restored_as_is() {
        let e = env();
        // Công cụ khác từng ghi "0" dạng chuỗi thay vì DWORD.
        let f = FakeOps::default().with_reg(r"HKCU\A", "x", RegData::Sz("0".into()));
        let r = apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &quiet);
        assert_eq!(r.outcomes[0].status, TweakStatus::Applied);
        assert_eq!(f.get(r"HKCU\A", "x"), Some(RegData::Dword(0)));
        revert(&e.cat, &f, &e.undo, &ids(&["reg"]), &quiet);
        assert_eq!(f.get(r"HKCU\A", "x"), Some(RegData::Sz("0".into())));
    }

    #[test]
    fn revert_without_snapshot_uses_catalog_default() {
        let e = env();
        let f = FakeOps::default().with_reg(r"HKCU\A", "x", RegData::Dword(0)).with_reg(r"HKCU\A", "y", RegData::Dword(0));
        let r = revert(&e.cat, &f, &e.undo, &ids(&["reg"]), &quiet);
        assert!(r.outcomes[0].errors.is_empty());
        assert_eq!(f.get(r"HKCU\A", "x"), Some(RegData::Dword(1)));
        assert_eq!(f.get(r"HKCU\A", "y"), None);
    }

    #[test]
    fn corrupt_undo_file_becomes_bak_and_revert_uses_default() {
        let e = env();
        std::fs::create_dir_all(e.undo.parent().unwrap()).unwrap();
        std::fs::write(&e.undo, "garbage").unwrap();
        let f = FakeOps::default().with_reg(r"HKCU\A", "x", RegData::Dword(0));
        let r = revert(&e.cat, &f, &e.undo, &ids(&["reg"]), &quiet);
        assert!(r.notices[0].starts_with("undo_corrupt:"), "{:?}", r.notices);
        assert!(e.undo.with_extension("json.bak").exists());
        assert_eq!(f.get(r"HKCU\A", "x"), Some(RegData::Dword(1)));
    }

    #[test]
    fn one_failing_op_does_not_stop_the_run() {
        let e = env();
        let f = FakeOps::default().failing(r"reg_write:HKCU\A|x").with_task(r"\T\One", true);
        let r = apply(&e.cat, &f, &e.undo, &ids(&["reg", "task"]), false, &quiet);
        assert_eq!(r.outcomes[0].status, TweakStatus::Partial);
        assert_eq!(r.outcomes[0].errors, vec![r"reg#0: fake failure: reg_write:HKCU\A|x".to_string()]);
        assert_eq!(r.outcomes[1].status, TweakStatus::Applied);
        let (u, _) = UndoStore::load(&e.undo);
        assert!(u.get("reg", 0).is_some(), "thao tác hỏng vẫn có ảnh chụp đã lưu trước khi chạy");
    }

    #[test]
    fn op_is_not_run_when_snapshot_cannot_be_saved() {
        let e = env();
        // Cha của file ảnh chụp là một FILE ⇒ không tạo được thư mục ⇒ không lưu được.
        std::fs::write(e.undo.parent().unwrap(), "x").unwrap();
        let f = FakeOps::default();
        let r = apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &quiet);
        assert!(r.outcomes[0].errors.iter().all(|m| m.contains("undo_save")), "{:?}", r.outcomes[0].errors);
        assert!(f.calls().iter().all(|c| !c.starts_with("reg_write")), "không có ảnh chụp ⇒ không ghi registry");
        assert_eq!(r.outcomes[0].status, TweakStatus::NotApplied);
    }

    #[test]
    fn service_disabled_and_stopped_then_restored_without_starting() {
        let e = env();
        let f = FakeOps::default().with_service("DiagTrack", StartType::Auto);
        apply(&e.cat, &f, &e.undo, &ids(&["svc"]), false, &quiet);
        assert_eq!(f.calls(), vec!["set_service_start:DiagTrack:Disabled", "stop_service:DiagTrack"]);
        revert(&e.cat, &f, &e.undo, &ids(&["svc"]), &quiet);
        assert_eq!(f.service_start("DiagTrack").unwrap(), Some(StartType::Auto));
        assert!(!f.calls().iter().any(|c| c.starts_with("start_service")));
    }

    #[test]
    fn task_reenabled_only_if_it_was_enabled() {
        let e = env();
        let f = FakeOps::default().with_task(r"\T\One", false);
        apply(&e.cat, &f, &e.undo, &ids(&["task"]), false, &quiet);
        revert(&e.cat, &f, &e.undo, &ids(&["task"]), &quiet);
        assert_eq!(f.task_enabled(r"\T\One").unwrap(), Some(false), "ảnh chụp là tắt ⇒ không bật");
    }

    #[test]
    fn unsupported_or_unknown_ids_are_refused() {
        let e = env();
        let mut f = FakeOps::default();
        f.sys.build = 22631;
        let r = apply(&e.cat, &f, &e.undo, &ids(&["recall", "nope"]), false, &quiet);
        assert_eq!(r.outcomes[0].errors, vec!["recall: not_allowed".to_string()]);
        assert_eq!(r.outcomes[1].errors, vec!["nope: unknown_id".to_string()]);
        assert!(f.calls().is_empty());
        assert_eq!(r.restart, Restart::None);
    }

    #[test]
    fn app_removed_for_current_user_then_revert_opens_store_and_keeps_snapshot() {
        let e = env();
        let f = FakeOps::default().with_package("A.App_1");
        let r = apply(&e.cat, &f, &e.undo, &ids(&["app"]), false, &quiet);
        assert_eq!(f.calls(), vec!["remove_package:A.App_1!1:false"]);
        assert_eq!(r.outcomes[0].status, TweakStatus::Applied);
        let r = revert(&e.cat, &f, &e.undo, &ids(&["app"]), &quiet);
        assert_eq!(r.outcomes[0].store_opened, vec!["9P1J8S7CCWWT"]);
        assert!(f.calls().contains(&"open_uri:ms-windows-store://pdp/?ProductId=9P1J8S7CCWWT".to_string()));
        let (u, _) = UndoStore::load(&e.undo);
        assert!(u.has_any("app"), "chưa cài lại ⇒ giữ ảnh chụp để còn nút «Cài lại từ Store»");
    }

    #[test]
    fn all_users_mode_also_deprovisions() {
        let e = env();
        let f = FakeOps::default().with_package("A.App_1");
        apply(&e.cat, &f, &e.undo, &ids(&["app"]), true, &quiet);
        assert_eq!(f.calls(), vec!["remove_package:A.App_1!1:true", "deprovision:A.App_1"]);
    }

    #[test]
    fn framework_or_system_packages_are_never_removed_even_if_catalog_says_so() {
        let e = env();
        let f = FakeOps::default();
        f.packages.lock().unwrap().push(PackageInfo { full_name: "A.App_1!1".into(), family: "A.App_1".into(), is_framework: false, non_removable: true });
        let r = apply(&e.cat, &f, &e.undo, &ids(&["app"]), false, &quiet);
        assert_eq!(r.outcomes[0].errors, vec!["app#0: blocked:A.App_1".to_string()]);
        assert!(f.calls().is_empty());
    }

    #[test]
    fn events_bracket_each_tweak() {
        let e = env();
        let f = FakeOps::default();
        let seen = Mutex::new(Vec::new());
        apply(&e.cat, &f, &e.undo, &ids(&["reg", "task"]), false, &|ev| {
            seen.lock().unwrap().push(match ev {
                TweakEvent::Started { id, index, total } => format!("start {id} {index}/{total}"),
                TweakEvent::Finished { outcome } => format!("end {}", outcome.id),
            })
        });
        assert_eq!(*seen.lock().unwrap(), vec!["start reg 0/2", "end reg", "start task 1/2", "end task"]);
    }

    #[test]
    fn read_all_marks_undo_and_serializes_flat_status() {
        let e = env();
        let f = FakeOps::default().with_package("A.App_1");
        apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &quiet);
        let r = read_all(&e.cat, &f, &e.undo).unwrap();
        let reg = r.tweaks.iter().find(|t| t.id == "reg").unwrap();
        assert!(reg.has_undo);
        let v = serde_json::to_value(reg).unwrap();
        assert_eq!(v["status"], "applied");
        assert_eq!(v["needs_restart"], "explorer");
        let recall = serde_json::to_value(r.tweaks.iter().find(|t| t.id == "recall").unwrap()).unwrap();
        assert_eq!(recall["status"], "not_applied", "build 26100 của FakeOps ⇒ hỗ trợ");
    }

    #[test]
    fn log_lines_are_greppable() {
        let e = env();
        let f = FakeOps::default().failing(r"reg_write:HKCU\A|x");
        let r = apply(&e.cat, &f, &e.undo, &ids(&["reg"]), false, &quiet);
        let lines = log_lines("APPLY", &r);
        assert_eq!(lines[0], "TWEAK APPLY reg ERRORS partial");
        assert_eq!(lines[1], r"TWEAK ERROR reg#0: fake failure: reg_write:HKCU\A|x");
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml -- engine::`
Expected: FAIL biên dịch — `cannot find function apply`, `cannot find type TweakEvent`.

- [ ] **Step 3: Viết mã (TRÊN khối test)**

```rust
//! Đọc trạng thái, áp dụng, hoàn tác (spec mục 3.3–3.4). Mỗi thao tác hỏng độc lập; mọi thao tác
//! có ảnh chụp được lưu xuống đĩa TRƯỚC khi chạm hệ thống.
use std::path::Path;

use serde::Serialize;

use crate::blocklist::is_blocked_package;
use crate::model::{default_data, to_reg_data, Group, Level, Op, Restart, Risk, StartType, Tweak};
use crate::ops::{SystemInfo, TweakOps};
use crate::state::{tweak_status, TweakStatus};
use crate::undo::{Snapshot, UndoStore};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TweakView {
    pub id: String,
    pub group: Group,
    pub level: Level,
    pub risk: Risk,
    pub needs_restart: Restart,
    #[serde(flatten)]
    pub status: TweakStatus,
    pub has_undo: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadResult {
    pub system: SystemInfo,
    pub tweaks: Vec<TweakView>,
    /// Mã thông báo cho giao diện: `undo_corrupt:<chi tiết>`.
    pub notices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TweakOutcome {
    pub id: String,
    /// Trạng thái đọc lại từ máy sau khi chạy.
    #[serde(flatten)]
    pub status: TweakStatus,
    /// Lỗi nguyên văn, dạng `<id>#<chỉ số thao tác>: <thông điệp>`.
    pub errors: Vec<String>,
    /// ProductId đã mở trang Store (hoàn tác app đã gỡ).
    pub store_opened: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunReport {
    pub outcomes: Vec<TweakOutcome>,
    /// Mức khởi động lại nặng nhất trong các mục đã thay đổi máy.
    pub restart: Restart,
    pub notices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TweakEvent {
    Started { id: String, index: usize, total: usize },
    Finished { outcome: TweakOutcome },
}

fn load_undo(path: &Path, notices: &mut Vec<String>) -> UndoStore {
    let (u, warn) = UndoStore::load(path);
    if let Some(w) = warn {
        notices.push(format!("undo_corrupt:{w}"));
    }
    u
}

pub fn read_all(catalog: &[Tweak], ops: &dyn TweakOps, undo_path: &Path) -> Result<ReadResult, String> {
    let system = ops.system_info()?;
    let mut notices = Vec::new();
    let undo = load_undo(undo_path, &mut notices);
    let tweaks = catalog
        .iter()
        .map(|t| {
            let (status, errors) = tweak_status(t, &system, ops, &undo);
            TweakView {
                id: t.id.clone(),
                group: t.group,
                level: t.level,
                risk: t.risk,
                needs_restart: t.needs_restart,
                status,
                has_undo: undo.has_any(&t.id),
                errors,
            }
        })
        .collect();
    Ok(ReadResult { system, tweaks, notices })
}

/// Ghi ảnh chụp (nếu chưa có) xuống đĩa. Không lưu được ⇒ `Err` và thao tác KHÔNG được chạy.
fn snapshot(undo: &mut UndoStore, id: &str, i: usize, snap: Snapshot) -> Result<(), String> {
    if undo.record_if_absent(id, i, snap) {
        if let Err(e) = undo.save() {
            undo.remove(id, i);
            return Err(format!("undo_save: {e}"));
        }
    }
    Ok(())
}

/// Chạy một thao tác áp dụng. `Ok(true)` = đã thay đổi máy.
fn apply_op(ops: &dyn TweakOps, undo: &mut UndoStore, id: &str, i: usize, op: &Op, all_users: bool, errors: &mut Vec<String>) -> Result<bool, String> {
    match op {
        Op::RegistrySet { path, name, value_type, value, .. } => {
            let target = to_reg_data(*value_type, value)?;
            let cur = ops.reg_read(path, name)?;
            snapshot(undo, id, i, Snapshot::Registry { data: cur.clone() })?;
            if cur.as_ref() == Some(&target) {
                return Ok(false);
            }
            ops.reg_write(path, name, &target)?;
            Ok(true)
        }
        Op::RegistryDelete { path, name } => {
            let cur = ops.reg_read(path, name)?;
            if cur.is_none() {
                return Ok(false);
            }
            snapshot(undo, id, i, Snapshot::Registry { data: cur })?;
            ops.reg_delete(path, name)?;
            Ok(true)
        }
        Op::ServiceStartup { service, start, .. } => {
            let Some(cur) = ops.service_start(service)? else { return Ok(false) };
            snapshot(undo, id, i, Snapshot::Service { start: cur })?;
            let changed = cur != *start;
            if changed {
                ops.set_service_start(service, *start)?;
            }
            if *start == StartType::Disabled {
                if let Err(e) = ops.stop_service(service) {
                    errors.push(format!("{id}#{i}: {e}"));
                }
            }
            Ok(changed)
        }
        Op::ScheduledTaskDisable { task } => {
            let Some(enabled) = ops.task_enabled(task)? else { return Ok(false) };
            snapshot(undo, id, i, Snapshot::Task { enabled })?;
            if !enabled {
                return Ok(false);
            }
            ops.set_task_enabled(task, false)?;
            Ok(true)
        }
        Op::AppxRemove { package_family, .. } => {
            let mut changed = false;
            for p in ops.packages(package_family)? {
                if is_blocked_package(&p.family) || p.is_framework || p.non_removable {
                    errors.push(format!("{id}#{i}: blocked:{}", p.family));
                    continue;
                }
                snapshot(undo, id, i, Snapshot::Appx { family: p.family.clone() })?;
                match ops.remove_package(&p.full_name, all_users) {
                    Ok(()) => changed = true,
                    Err(e) => errors.push(format!("{id}#{i}: {e}")),
                }
            }
            if all_users {
                if let Err(e) = ops.deprovision(package_family) {
                    errors.push(format!("{id}#{i}: {e}"));
                }
            }
            Ok(changed)
        }
    }
}

enum Reverted {
    /// Đã trả về như trước ⇒ xoá ảnh chụp của thao tác.
    Done,
    /// Không làm gì (không có gì để trả).
    Nothing,
    /// Đã mở trang Store; giữ ảnh chụp cho tới khi app được cài lại.
    StoreOpened(String),
}

fn revert_op(ops: &dyn TweakOps, snap: Option<&Snapshot>, op: &Op) -> Result<Reverted, String> {
    match op {
        Op::RegistrySet { path, name, value_type, default, .. } => {
            let want = match snap {
                Some(Snapshot::Registry { data }) => data.clone(),
                _ => default_data(*value_type, default)?,
            };
            match want {
                Some(d) => ops.reg_write(path, name, &d)?,
                None => ops.reg_delete(path, name)?,
            }
            Ok(Reverted::Done)
        }
        Op::RegistryDelete { path, name } => match snap {
            Some(Snapshot::Registry { data: Some(d) }) => ops.reg_write(path, name, d).map(|_| Reverted::Done),
            Some(_) => Ok(Reverted::Done),
            None => Err("no_snapshot".into()),
        },
        Op::ServiceStartup { service, default, .. } => {
            if ops.service_start(service)?.is_none() {
                return Ok(Reverted::Nothing);
            }
            let want = match snap {
                Some(Snapshot::Service { start }) => *start,
                _ => *default,
            };
            ops.set_service_start(service, want).map(|_| Reverted::Done)
        }
        Op::ScheduledTaskDisable { task } => {
            if ops.task_enabled(task)?.is_none() {
                return Ok(Reverted::Nothing);
            }
            let want = match snap {
                Some(Snapshot::Task { enabled }) => *enabled,
                _ => true,
            };
            if want {
                ops.set_task_enabled(task, true)?;
            }
            Ok(Reverted::Done)
        }
        Op::AppxRemove { package_family, store_product_id } => {
            if !ops.packages(package_family)?.is_empty() {
                return Ok(Reverted::Done);
            }
            if snap.is_none() {
                return Ok(Reverted::Nothing);
            }
            ops.open_uri(&format!("ms-windows-store://pdp/?ProductId={store_product_id}"))?;
            Ok(Reverted::StoreOpened(store_product_id.clone()))
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Apply { all_users: bool },
    Revert,
}

fn run(catalog: &[Tweak], ops: &dyn TweakOps, undo_path: &Path, ids: &[String], mode: Mode, on_event: &dyn Fn(&TweakEvent)) -> RunReport {
    let mut notices = Vec::new();
    let mut undo = load_undo(undo_path, &mut notices);
    let mut outcomes = Vec::new();
    let mut restart = Restart::None;
    let sys = ops.system_info();
    for (index, id) in ids.iter().enumerate() {
        on_event(&TweakEvent::Started { id: id.clone(), index, total: ids.len() });
        let mut errors = Vec::new();
        let mut store_opened = Vec::new();
        let outcome = match (catalog.iter().find(|t| &t.id == id), &sys) {
            (None, _) => TweakOutcome { id: id.clone(), status: TweakStatus::NotApplied, errors: vec![format!("{id}: unknown_id")], store_opened },
            (_, Err(e)) => TweakOutcome { id: id.clone(), status: TweakStatus::NotApplied, errors: vec![format!("{id}: {e}")], store_opened },
            (Some(t), Ok(sys)) => {
                let (before, _) = tweak_status(t, sys, ops, &undo);
                if !before.actionable() {
                    errors.push(format!("{id}: not_allowed"));
                } else {
                    let mut changed = false;
                    match mode {
                        Mode::Apply { all_users } => {
                            for (i, op) in t.ops.iter().enumerate() {
                                match apply_op(ops, &mut undo, id, i, op, all_users, &mut errors) {
                                    Ok(c) => changed |= c,
                                    Err(e) => errors.push(format!("{id}#{i}: {e}")),
                                }
                            }
                        }
                        Mode::Revert => {
                            for (i, op) in t.ops.iter().enumerate().rev() {
                                match revert_op(ops, undo.get(id, i), op) {
                                    Ok(Reverted::Done) => {
                                        changed = true;
                                        undo.remove(id, i);
                                    }
                                    Ok(Reverted::Nothing) => {}
                                    Ok(Reverted::StoreOpened(pid)) => store_opened.push(pid),
                                    Err(e) => errors.push(format!("{id}#{i}: {e}")),
                                }
                            }
                            if let Err(e) = undo.save() {
                                errors.push(format!("{id}: undo_save: {e}"));
                            }
                        }
                    }
                    if changed {
                        restart = restart.max(t.needs_restart);
                    }
                }
                let (after, read_errors) = tweak_status(t, sys, ops, &undo);
                errors.extend(read_errors);
                TweakOutcome { id: id.clone(), status: after, errors, store_opened }
            }
        };
        on_event(&TweakEvent::Finished { outcome: outcome.clone() });
        outcomes.push(outcome);
    }
    RunReport { outcomes, restart, notices }
}

pub fn apply(catalog: &[Tweak], ops: &dyn TweakOps, undo_path: &Path, ids: &[String], all_users: bool, on_event: &dyn Fn(&TweakEvent)) -> RunReport {
    run(catalog, ops, undo_path, ids, Mode::Apply { all_users }, on_event)
}

pub fn revert(catalog: &[Tweak], ops: &dyn TweakOps, undo_path: &Path, ids: &[String], on_event: &dyn Fn(&TweakEvent)) -> RunReport {
    run(catalog, ops, undo_path, ids, Mode::Revert, on_event)
}

/// Dòng nhật ký cho `log::CleanLog` của v0.1 (Task Tích hợp ghi ra file).
pub fn log_lines(action: &str, r: &RunReport) -> Vec<String> {
    let mut out: Vec<String> = r.notices.iter().map(|n| format!("TWEAK NOTICE {n}")).collect();
    for o in &r.outcomes {
        let status = serde_json::to_value(&o.status).ok().and_then(|v| v["status"].as_str().map(str::to_string)).unwrap_or_default();
        out.push(format!("TWEAK {action} {} {} {status}", o.id, if o.errors.is_empty() { "OK" } else { "ERRORS" }));
        out.extend(o.errors.iter().map(|e| format!("TWEAK ERROR {e}")));
        out.extend(o.store_opened.iter().map(|p| format!("TWEAK STORE {p}")));
    }
    out
}
```

- [ ] **Step 4: Chạy test và clippy, thấy qua**

Run: `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml -- engine::` rồi `cargo clippy --manifest-path crates/winfreeup-tweaks/Cargo.toml --all-targets -- -D warnings`
Expected: `16 passed`; clippy sạch.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-tweaks/src/engine.rs
rtk git commit -m "feat(tweaks): bộ máy áp dụng/hoàn tác — ảnh chụp trước khi chạm máy, lỗi từng thao tác độc lập, nhật ký

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Windows thật — registry, thông tin máy, Store/Explorer

**Files:**
- Modify: `crates/winfreeup-tweaks/src/sys_windows/registry.rs` (thay dòng khung)
- Modify: `crates/winfreeup-tweaks/src/sys_windows/info.rs` (thay dòng khung)
- Modify: `crates/winfreeup-tweaks/src/sys_windows/shell.rs` (thay dòng khung)
- Test: ba file trên (module `tests`) — registry test ghi/xoá trên **khoá thật riêng** `HKCU\Software\WinFreeUpTest\<pid>-<nano giây>` và xoá cả cây khi xong (kể cả khi test hỏng, nhờ `Drop`); mọi phần khác chỉ **đọc**.

**Interfaces:**
- Consumes: Task 1 — `RegData`, `SystemInfo`, `edition_of`, `StartType`, `PackageInfo`, `TweakOps`, `fake::FakeOps`, `parse_catalog`; Task 3 — `UndoStore::load`, `Snapshot`; Task 4 — `TweakStatus`; Task 5 — `read_all`, `apply`, `revert`; Task 2 — `catalog::is_product_id`. Crate `windows` 0.62 (feature `Win32_System_Registry`, `Win32_Security`, `Win32_NetworkManagement_NetManagement`, `Win32_System_RemoteDesktop`, `Win32_System_WindowsProgramming`, `Win32_UI_Shell`, `Win32_UI_WindowsAndMessaging` — đã khai báo ở Task 1).
- Produces (Task 8 dùng nguyên văn):
  - `registry::{read(path: &str, name: &str) -> Result<Option<RegData>, String>, write(path, name, &RegData) -> Result<(), String>, delete(path, name) -> Result<(), String>, subkeys(path) -> Result<Vec<String>, String>}` — `path` dạng `HKCU\…`/`HKLM\…` (chấp nhận cả `HKEY_CURRENT_USER\…`); key/value vắng ⇒ `Ok(None)`/`Ok(())`/`Ok(vec![])`; REG_EXPAND_SZ đọc như chuỗi không bung biến.
  - `info::{system_info() -> Result<SystemInfo, String>, is_mdm_provider(provider_id: &str) -> bool, other_user() -> bool}`.
  - `shell::{STORE_PREFIX, check_store_uri(uri: &str) -> Result<(), String>, open_store_uri(uri: &str) -> Result<(), String>, restart_explorer() -> Result<(), String>}` — chỉ mở đúng dạng `ms-windows-store://pdp/?ProductId=<id>` với `catalog::is_product_id(id)`.

Ghi chú đo trên máy dev (2026-09-25): `system_info()` trả `build 26200, edition "Pro", managed false, other_user false`; `HKLM\SOFTWARE\Microsoft\Enrollments` có 35 key, ProviderID chỉ gồm rỗng / `Local Authority` / `Cloud Authority` / `Deploy Authority` ⇒ phải so đúng `MS DM Server`. `ProductName` của máy Win 11 vẫn ghi «Windows 10 Pro» — không dùng `ProductName`, chỉ dùng `CurrentBuildNumber` + `EditionID`. `restart_explorer` **không có test tự động** (tắt Explorer của máy đang chạy test) — thử tay ở Task 12.

- [ ] **Step 1: Viết test hỏng**

`sys_windows/registry.rs` — chỉ phần test (lớp `RealReg`: registry thật, phần còn lại qua `FakeOps` đúng như spec mục 7):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{apply, read_all, revert};
    use crate::fake::FakeOps;
    use crate::model::{parse_catalog, StartType};
    use crate::ops::{PackageInfo, SystemInfo, TweakOps};
    use crate::state::TweakStatus;
    use crate::undo::{Snapshot, UndoStore};
    use windows::Win32::System::Registry::RegDeleteTreeW;

    /// Registry THẬT (khoá test riêng); dịch vụ, tác vụ, gói app, thông tin máy qua lớp giả (spec mục 7).
    struct RealReg(FakeOps);

    impl TweakOps for RealReg {
        fn reg_read(&self, path: &str, name: &str) -> Result<Option<RegData>, String> {
            read(path, name)
        }
        fn reg_write(&self, path: &str, name: &str, data: &RegData) -> Result<(), String> {
            write(path, name, data)
        }
        fn reg_delete(&self, path: &str, name: &str) -> Result<(), String> {
            delete(path, name)
        }
        fn service_start(&self, name: &str) -> Result<Option<StartType>, String> {
            self.0.service_start(name)
        }
        fn set_service_start(&self, name: &str, start: StartType) -> Result<(), String> {
            self.0.set_service_start(name, start)
        }
        fn stop_service(&self, name: &str) -> Result<(), String> {
            self.0.stop_service(name)
        }
        fn task_enabled(&self, path: &str) -> Result<Option<bool>, String> {
            self.0.task_enabled(path)
        }
        fn set_task_enabled(&self, path: &str, enabled: bool) -> Result<(), String> {
            self.0.set_task_enabled(path, enabled)
        }
        fn packages(&self, family: &str) -> Result<Vec<PackageInfo>, String> {
            self.0.packages(family)
        }
        fn remove_package(&self, full_name: &str, all_users: bool) -> Result<(), String> {
            self.0.remove_package(full_name, all_users)
        }
        fn deprovision(&self, family: &str) -> Result<(), String> {
            self.0.deprovision(family)
        }
        fn open_uri(&self, uri: &str) -> Result<(), String> {
            self.0.open_uri(uri)
        }
        fn system_info(&self) -> Result<SystemInfo, String> {
            self.0.system_info()
        }
        fn restart_explorer(&self) -> Result<(), String> {
            self.0.restart_explorer()
        }
    }

    /// Khoá riêng `HKCU\Software\WinFreeUpTest\<duy nhất>`; xoá cả cây khi rơi khỏi phạm vi (kể cả khi test hỏng).
    struct TestKey(String);

    impl TestKey {
        fn new() -> TestKey {
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            TestKey(format!(r"Software\WinFreeUpTest\{}-{nanos}", std::process::id()))
        }
        fn path(&self) -> String {
            format!(r"HKCU\{}", self.0)
        }
    }

    impl Drop for TestKey {
        fn drop(&mut self) {
            unsafe {
                let _ = RegDeleteTreeW(HKEY_CURRENT_USER, &HSTRING::from(self.0.as_str()));
            }
        }
    }

    #[test]
    fn read_write_delete_roundtrip_on_real_registry() {
        let k = TestKey::new();
        let p = k.path();
        assert_eq!(read(&p, "d").unwrap(), None, "key chưa tồn tại ⇒ None, không lỗi");
        write(&p, "d", &RegData::Dword(0)).unwrap();
        write(&p, "q", &RegData::Qword(1 << 40)).unwrap();
        write(&p, "s", &RegData::Sz("Tiếng Việt".into())).unwrap();
        assert_eq!(read(&p, "d").unwrap(), Some(RegData::Dword(0)));
        assert_eq!(read(&p, "q").unwrap(), Some(RegData::Qword(1 << 40)));
        assert_eq!(read(&p, "s").unwrap(), Some(RegData::Sz("Tiếng Việt".into())));
        delete(&p, "d").unwrap();
        delete(&p, "d").unwrap();
        assert_eq!(read(&p, "d").unwrap(), None);
        let leaf = k.0.rsplit('\\').next().unwrap().to_string();
        assert!(subkeys(r"HKCU\Software\WinFreeUpTest").unwrap().contains(&leaf));
    }

    #[test]
    fn test_key_is_removed_on_drop() {
        let p;
        {
            let k = TestKey::new();
            p = k.path();
            write(&p, "x", &RegData::Dword(1)).unwrap();
        }
        assert_eq!(read(&p, "x").unwrap(), None);
        let leaf = p.rsplit('\\').next().unwrap().to_string();
        assert!(!subkeys(r"HKCU\Software\WinFreeUpTest").unwrap().contains(&leaf));
    }

    #[test]
    fn bad_root_is_an_error() {
        assert!(read(r"HKCR\x", "y").is_err());
        assert!(write("nope", "y", &RegData::Dword(1)).is_err());
    }

    fn catalog(p: &str) -> String {
        format!(
            r#"
[[tweak]]
id = "real"
group = "privacy"
level = "basic"
risk = "safe"
windows = {{ min_build = 0 }}
ops = [
  {{ kind = "registry_set", path = '{p}', name = "a", type = "dword", value = 0, default = 1 }},
  {{ kind = "registry_set", path = '{p}', name = "b", type = "dword", value = 0, default = "absent" }},
]
"#
        )
    }

    #[test]
    fn engine_on_real_registry_partial_snapshot_once_and_absent_revert() {
        let k = TestKey::new();
        let p = k.path();
        let cat = parse_catalog(&catalog(&p)).unwrap();
        let d = tempfile::tempdir().unwrap();
        let undo = d.path().join("tweaks-undo.json");
        let ids = vec!["real".to_string()];
        let ops = RealReg(FakeOps::default());
        write(&p, "a", &RegData::Dword(5)).unwrap();
        assert_eq!(read_all(&cat, &ops, &undo).unwrap().tweaks[0].status, TweakStatus::NotApplied);
        write(&p, "b", &RegData::Dword(0)).unwrap();
        assert_eq!(read_all(&cat, &ops, &undo).unwrap().tweaks[0].status, TweakStatus::Partial);
        delete(&p, "b").unwrap();
        let rep = apply(&cat, &ops, &undo, &ids, false, &|_| {});
        assert_eq!(rep.outcomes[0].status, TweakStatus::Applied, "{:?}", rep.outcomes[0].errors);
        write(&p, "a", &RegData::Dword(9)).unwrap();
        apply(&cat, &ops, &undo, &ids, false, &|_| {});
        let (u, _) = UndoStore::load(&undo);
        assert_eq!(u.get("real", 0), Some(&Snapshot::Registry { data: Some(RegData::Dword(5)) }), "ảnh chụp chỉ ghi lần đầu");
        let rep = revert(&cat, &ops, &undo, &ids, &|_| {});
        assert!(rep.outcomes[0].errors.is_empty(), "{:?}", rep.outcomes[0].errors);
        assert_eq!(read(&p, "a").unwrap(), Some(RegData::Dword(5)));
        assert_eq!(read(&p, "b").unwrap(), None, "ảnh chụp absent ⇒ xoá value");
    }

    #[test]
    fn engine_on_real_registry_uses_default_when_undo_file_is_corrupt() {
        let k = TestKey::new();
        let p = k.path();
        let cat = parse_catalog(&catalog(&p)).unwrap();
        let d = tempfile::tempdir().unwrap();
        let undo = d.path().join("tweaks-undo.json");
        std::fs::write(&undo, "{ hỏng").unwrap();
        let ops = RealReg(FakeOps::default());
        write(&p, "a", &RegData::Dword(0)).unwrap();
        write(&p, "b", &RegData::Dword(0)).unwrap();
        let rep = revert(&cat, &ops, &undo, &["real".to_string()], &|_| {});
        assert!(rep.notices[0].starts_with("undo_corrupt:"), "{:?}", rep.notices);
        assert!(d.path().join("tweaks-undo.json.bak").exists());
        assert_eq!(read(&p, "a").unwrap(), Some(RegData::Dword(1)), "không có ảnh chụp ⇒ default = 1");
        assert_eq!(read(&p, "b").unwrap(), None, "default absent ⇒ xoá value");
    }
}
```

`sys_windows/info.rs` — chỉ phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_mdm_counts_as_managed() {
        for p in ["", "Local Authority", "Cloud Authority", "Deploy Authority", "WMI_Bridge_Server"] {
            assert!(!is_mdm_provider(p), "{p}");
        }
        assert!(is_mdm_provider("MS DM Server"));
        assert!(is_mdm_provider("ms dm server "));
    }

    #[test]
    fn reads_this_machine() {
        let s = system_info().unwrap();
        assert!(s.build >= 17763, "build {}", s.build);
        assert!(["Home", "Pro", "Enterprise", "Education", "Other"].contains(&s.edition.as_str()));
        // Máy dev và runner CI chạy test bằng chính người đăng nhập.
        assert!(!s.other_user);
    }
}
```

`sys_windows/shell.rs` — chỉ phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_store_product_pages_may_be_opened() {
        assert!(check_store_uri("ms-windows-store://pdp/?ProductId=9P1J8S7CCWWT").is_ok());
        assert!(check_store_uri("ms-windows-store://pdp/?ProductId=XP8BT8DW290MPQ").is_ok());
        assert!(check_store_uri("https://evil.example/").is_err());
        assert!(check_store_uri("ms-windows-store://pdp/?ProductId=9P1J8S7CCWWT&x=1").is_err());
        assert!(open_store_uri("file:///C:/Windows/System32/cmd.exe").is_err());
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml -- sys_windows::registry sys_windows::info sys_windows::shell`
Expected: FAIL biên dịch — `cannot find function read`, `cannot find function system_info`, `cannot find function check_store_uri`.

- [ ] **Step 3: Viết mã (TRÊN khối test của từng file)**

`registry.rs`:

```rust
//! Registry thật: đọc/ghi/xoá value dưới `HKCU\` hoặc `HKLM\` (view 64-bit — exe là 64-bit).
use windows::core::{HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_ITEMS, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteKeyValueW, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE, REG_DWORD, REG_EXPAND_SZ, REG_OPTION_NON_VOLATILE,
    REG_QWORD, REG_SZ, REG_VALUE_TYPE, RRF_NOEXPAND, RRF_RT_ANY,
};

use crate::ops::RegData;

fn split(path: &str) -> Result<(HKEY, &str), String> {
    let (root, sub) = path.split_once('\\').ok_or_else(|| format!("bad registry path: {path}"))?;
    let hkey = match root.to_ascii_uppercase().as_str() {
        "HKCU" | "HKEY_CURRENT_USER" => HKEY_CURRENT_USER,
        "HKLM" | "HKEY_LOCAL_MACHINE" => HKEY_LOCAL_MACHINE,
        _ => return Err(format!("unsupported registry root: {path}")),
    };
    Ok((hkey, sub))
}

fn win_err(what: &str, code: WIN32_ERROR) -> String {
    format!("{what}: {}", windows::core::Error::from(code.to_hresult()).message())
}

fn missing(code: WIN32_ERROR) -> bool {
    code == ERROR_FILE_NOT_FOUND || code == ERROR_PATH_NOT_FOUND
}

/// Key hoặc value không tồn tại ⇒ `Ok(None)`.
pub fn read(path: &str, name: &str) -> Result<Option<RegData>, String> {
    let (root, sub) = split(path)?;
    let (s, n) = (HSTRING::from(sub), HSTRING::from(name));
    let flags = RRF_RT_ANY | RRF_NOEXPAND;
    let mut ty = REG_VALUE_TYPE::default();
    let mut len = 0u32;
    let rc = unsafe { RegGetValueW(root, &s, &n, flags, Some(&mut ty), None, Some(&mut len)) };
    if missing(rc) {
        return Ok(None);
    }
    if rc != ERROR_SUCCESS {
        return Err(win_err(&format!("{path}\\{name}"), rc));
    }
    let mut buf = vec![0u8; len as usize];
    let rc = unsafe { RegGetValueW(root, &s, &n, flags, Some(&mut ty), Some(buf.as_mut_ptr().cast()), Some(&mut len)) };
    if missing(rc) {
        return Ok(None);
    }
    if rc != ERROR_SUCCESS {
        return Err(win_err(&format!("{path}\\{name}"), rc));
    }
    buf.truncate(len as usize);
    match ty {
        REG_DWORD if buf.len() >= 4 => Ok(Some(RegData::Dword(u32::from_le_bytes(buf[..4].try_into().unwrap())))),
        REG_QWORD if buf.len() >= 8 => Ok(Some(RegData::Qword(u64::from_le_bytes(buf[..8].try_into().unwrap())))),
        REG_SZ | REG_EXPAND_SZ => {
            let wide: Vec<u16> = buf.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
            Ok(Some(RegData::Sz(String::from_utf16_lossy(&wide[..end]))))
        }
        other => Err(format!("{path}\\{name}: unsupported value type {}", other.0)),
    }
}

/// Tạo key nếu chưa có rồi ghi value.
pub fn write(path: &str, name: &str, data: &RegData) -> Result<(), String> {
    let (root, sub) = split(path)?;
    let (ty, bytes) = match data {
        RegData::Dword(v) => (REG_DWORD, v.to_le_bytes().to_vec()),
        RegData::Qword(v) => (REG_QWORD, v.to_le_bytes().to_vec()),
        RegData::Sz(s) => (REG_SZ, s.encode_utf16().chain(std::iter::once(0)).flat_map(|c| c.to_le_bytes()).collect()),
    };
    let mut key = HKEY::default();
    let rc = unsafe {
        RegCreateKeyExW(root, &HSTRING::from(sub), None, PCWSTR::null(), REG_OPTION_NON_VOLATILE, KEY_SET_VALUE, None, &mut key, None)
    };
    if rc != ERROR_SUCCESS {
        return Err(win_err(path, rc));
    }
    let rc = unsafe { RegSetValueExW(key, &HSTRING::from(name), None, ty, Some(&bytes)) };
    unsafe {
        let _ = RegCloseKey(key);
    }
    if rc != ERROR_SUCCESS {
        return Err(win_err(&format!("{path}\\{name}"), rc));
    }
    Ok(())
}

/// Value hoặc key không tồn tại ⇒ `Ok(())`. Không bao giờ xoá key.
pub fn delete(path: &str, name: &str) -> Result<(), String> {
    let (root, sub) = split(path)?;
    let rc = unsafe { RegDeleteKeyValueW(root, &HSTRING::from(sub), &HSTRING::from(name)) };
    if rc == ERROR_SUCCESS || missing(rc) {
        Ok(())
    } else {
        Err(win_err(&format!("{path}\\{name}"), rc))
    }
}

/// Tên các key con trực tiếp. Key không tồn tại ⇒ rỗng.
pub fn subkeys(path: &str) -> Result<Vec<String>, String> {
    let (root, sub) = split(path)?;
    let mut key = HKEY::default();
    let rc = unsafe { RegOpenKeyExW(root, &HSTRING::from(sub), None, KEY_READ, &mut key) };
    if missing(rc) {
        return Ok(vec![]);
    }
    if rc != ERROR_SUCCESS {
        return Err(win_err(path, rc));
    }
    let mut out = Vec::new();
    let mut result = Ok(());
    for i in 0.. {
        let mut name = [0u16; 256];
        let mut len = name.len() as u32;
        let rc = unsafe { RegEnumKeyExW(key, i, Some(PWSTR(name.as_mut_ptr())), &mut len, None, None, None, None) };
        if rc == ERROR_NO_MORE_ITEMS {
            break;
        }
        if rc != ERROR_SUCCESS {
            result = Err(win_err(path, rc));
            break;
        }
        out.push(String::from_utf16_lossy(&name[..len as usize]));
    }
    unsafe {
        let _ = RegCloseKey(key);
    }
    result.map(|_| out)
}
```

`info.rs`:

```rust
//! Build, edition, máy có bị tổ chức quản lý không, và app có đang chạy bằng tài khoản khác không.
use windows::core::PWSTR;
use windows::Win32::NetworkManagement::NetManagement::{NetApiBufferFree, NetGetJoinInformation, NetSetupDomainName, NETSETUP_JOIN_STATUS};
use windows::Win32::System::RemoteDesktop::{WTSFreeMemory, WTSQuerySessionInformationW, WTSUserName, WTS_CURRENT_SESSION};
use windows::Win32::System::WindowsProgramming::GetUserNameW;

use super::registry;
use crate::ops::{edition_of, RegData, SystemInfo};

const CURRENT_VERSION: &str = r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion";
const ENROLLMENTS: &str = r"HKLM\SOFTWARE\Microsoft\Enrollments";

fn read_sz(path: &str, name: &str) -> Result<String, String> {
    match registry::read(path, name)? {
        Some(RegData::Sz(s)) => Ok(s),
        other => Err(format!("{path}\\{name}: unexpected {other:?}")),
    }
}

pub fn system_info() -> Result<SystemInfo, String> {
    let build_text = read_sz(CURRENT_VERSION, "CurrentBuildNumber")?;
    let build = build_text.trim().parse::<u32>().map_err(|e| format!("CurrentBuildNumber {build_text:?}: {e}"))?;
    let edition = edition_of(&read_sz(CURRENT_VERSION, "EditionID").unwrap_or_default()).to_string();
    Ok(SystemInfo { build, edition, managed: domain_joined() || mdm_enrolled(), other_user: other_user() })
}

fn domain_joined() -> bool {
    let mut name = PWSTR::null();
    let mut status = NETSETUP_JOIN_STATUS::default();
    let rc = unsafe { NetGetJoinInformation(None, &mut name, &mut status) };
    if !name.is_null() {
        unsafe {
            NetApiBufferFree(Some(name.0 as *const _));
        }
    }
    rc == 0 && status == NetSetupDomainName
}

/// Chỉ đăng ký MDM thật (Intune và các MDM khác dùng ProviderID `MS DM Server`). Máy cá nhân cũng có
/// hàng chục key dưới `Enrollments` (ProviderID rỗng hoặc Local/Cloud/Deploy Authority) — không tính
/// (đo trên máy dev 2026-09-25: 35 key, không key nào là `MS DM Server`).
pub fn is_mdm_provider(provider_id: &str) -> bool {
    provider_id.trim().eq_ignore_ascii_case("MS DM Server")
}

fn mdm_enrolled() -> bool {
    registry::subkeys(ENROLLMENTS).unwrap_or_default().iter().any(|k| {
        matches!(registry::read(&format!(r"{ENROLLMENTS}\{k}"), "ProviderID"), Ok(Some(RegData::Sz(p))) if is_mdm_provider(&p))
    })
}

fn wide_to_string(p: PWSTR) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { p.to_string().unwrap_or_default() }
    }
}

/// true khi người đăng nhập phiên này khác tài khoản của tiến trình (UAC nhập mật khẩu admin khác).
/// Khi đó `HKCU` và gói app là của tài khoản admin, không phải của người đang ngồi máy.
pub fn other_user() -> bool {
    let mut buf = PWSTR::null();
    let mut bytes = 0u32;
    let ok = unsafe { WTSQuerySessionInformationW(None, WTS_CURRENT_SESSION, WTSUserName, &mut buf, &mut bytes) }.is_ok();
    let session_user = wide_to_string(buf);
    if !buf.is_null() {
        unsafe { WTSFreeMemory(buf.0.cast()) };
    }
    let mut name = [0u16; 257];
    let mut len = name.len() as u32;
    let me = match unsafe { GetUserNameW(Some(PWSTR(name.as_mut_ptr())), &mut len) } {
        Ok(()) => String::from_utf16_lossy(&name[..len.saturating_sub(1) as usize]),
        Err(_) => return false,
    };
    ok && !session_user.is_empty() && !session_user.eq_ignore_ascii_case(&me)
}
```

`shell.rs`:

```rust
//! Mở trang Store và khởi động lại Explorer.
use std::process::Command;
use std::time::{Duration, Instant};

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub const STORE_PREFIX: &str = "ms-windows-store://pdp/?ProductId=";

/// Chỉ mở đúng dạng `ms-windows-store://pdp/?ProductId=<ProductId hợp lệ>` — không mở URI tuỳ ý.
pub fn check_store_uri(uri: &str) -> Result<(), String> {
    match uri.strip_prefix(STORE_PREFIX) {
        Some(id) if crate::catalog::is_product_id(id) => Ok(()),
        _ => Err(format!("refused uri: {uri}")),
    }
}

pub fn open_store_uri(uri: &str) -> Result<(), String> {
    check_store_uri(uri)?;
    let h = unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(uri), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
    // ShellExecuteW trả giá trị > 32 khi thành công.
    if h.0 as isize > 32 {
        Ok(())
    } else {
        Err(format!("{uri}: ShellExecute error {}", h.0 as isize))
    }
}

fn explorer_running() -> bool {
    Command::new("tasklist")
        .args(["/fi", "imagename eq explorer.exe", "/fo", "csv", "/nh"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_ascii_lowercase().contains("\"explorer.exe\""))
        .unwrap_or(false)
}

/// Tắt Explorer; Windows tự bật lại vỏ (`AutoRestartShell`). Sau 8 giây chưa thấy thì tự khởi động.
pub fn restart_explorer() -> Result<(), String> {
    let out = Command::new("taskkill").args(["/f", "/im", "explorer.exe"]).output().map_err(|e| format!("taskkill: {e}"))?;
    if !out.status.success() && explorer_running() {
        return Err(format!("taskkill: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        std::thread::sleep(Duration::from_millis(500));
        if explorer_running() {
            return Ok(());
        }
    }
    Command::new("explorer.exe").spawn().map(|_| ()).map_err(|e| format!("explorer.exe: {e}"))
}
```

- [ ] **Step 4: Chạy test và clippy, thấy qua; kiểm khoá test đã dọn**

Run (terminal thường, KHÔNG cần Admin):

```powershell
cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml -- sys_windows::registry sys_windows::info sys_windows::shell
cargo clippy --manifest-path crates/winfreeup-tweaks/Cargo.toml --all-targets -- -D warnings
reg query HKCU\Software\WinFreeUpTest
```

Expected: `8 passed` (5 registry + 2 info + 1 shell); clippy sạch; `reg query` không liệt kê key con nào (chỉ còn key cha rỗng `WinFreeUpTest`, hoặc báo không tìm thấy nếu máy chưa từng chạy test).

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-tweaks/src/sys_windows/registry.rs
rtk git add crates/winfreeup-tweaks/src/sys_windows/info.rs
rtk git add crates/winfreeup-tweaks/src/sys_windows/shell.rs
rtk git commit -m "feat(tweaks): registry thật (test trên HKCU\\Software\\WinFreeUpTest), build/edition/domain/MDM, mở trang Store

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Windows thật — dịch vụ (SCM) và tác vụ theo lịch (COM)

**Files:**
- Modify: `crates/winfreeup-tweaks/src/sys_windows/services.rs` (thay dòng khung)
- Modify: `crates/winfreeup-tweaks/src/sys_windows/tasks.rs` (thay dòng khung)
- Test: hai file trên (module `tests`) — chỉ **đọc**; đổi kiểu khởi động/tắt tác vụ thật được thử tay ở Task 12.

**Interfaces:**
- Consumes: Task 1 — `StartType`. Crate `windows` 0.62 (feature `Win32_System_Services`, `Win32_System_Com`, `Win32_System_Ole`, `Win32_System_Variant`, `Win32_System_TaskScheduler` — đã khai báo ở Task 1).
- Produces (Task 8 dùng nguyên văn):
  - `services::{start_type(name: &str) -> Result<Option<StartType>, String>, set_start_type(name: &str, start: StartType) -> Result<(), String>, stop(name: &str) -> Result<(), String>}` — dịch vụ không có ⇒ `Ok(None)` / `stop` trả `Ok(())` / `set_start_type` trả `Err`; `stop` không chờ dịch vụ dừng hẳn; đọc không cần Admin.
  - `tasks::{split_task_path(path: &str) -> Result<(String, String), String>, enabled(path: &str) -> Result<Option<bool>, String>, set_enabled(path: &str, on: bool) -> Result<(), String>}` — thư mục/tác vụ không có ⇒ `Ok(None)`.

Vì sao COM chứ không `schtasks`: chữ trạng thái của `schtasks /Query` đổi theo ngôn ngữ Windows (máy tiếng Việt in «Sẵn sàng»/«Đã tắt»). Đo trên máy dev: `ITaskService` đọc được `\Microsoft\Windows\…` bằng tài khoản thường.

- [ ] **Step 1: Viết test hỏng**

`sys_windows/services.rs` — chỉ phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_start_type_without_changing_anything() {
        // Winmgmt (WMI) có trên mọi bản Windows, kể cả runner CI; đọc cấu hình không cần Admin.
        assert!(start_type("Winmgmt").unwrap().is_some());
        assert_eq!(start_type("WinFreeUpNoSuchService").unwrap(), None);
        assert!(stop("WinFreeUpNoSuchService").is_ok());
        assert!(set_start_type("WinFreeUpNoSuchService", StartType::Manual).is_err());
    }
}
```

`sys_windows/tasks.rs` — chỉ phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_paths() {
        assert_eq!(split_task_path(r"\Microsoft\Windows\A B\C").unwrap(), (r"\Microsoft\Windows\A B".into(), "C".into()));
        assert_eq!(split_task_path(r"\Top").unwrap(), (r"\".into(), "Top".into()));
        assert!(split_task_path(r"\Folder\").is_err());
    }

    #[test]
    fn reads_task_state_without_changing_anything() {
        assert_eq!(enabled(r"\WinFreeUpNoSuchFolder\X").unwrap(), None);
        assert_eq!(enabled(r"\Microsoft\Windows\WinFreeUpNoSuchTask").unwrap(), None);
        assert!(set_enabled(r"\WinFreeUpNoSuchFolder\X", true).is_err());
        // Tác vụ chống phân mảnh có trên mọi bản Windows client và Server có Desktop Experience.
        assert!(enabled(r"\Microsoft\Windows\Defrag\ScheduledDefrag").unwrap().is_some());
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml -- sys_windows::services sys_windows::tasks`
Expected: FAIL biên dịch — `cannot find function start_type`, `cannot find function split_task_path`.

- [ ] **Step 3: Viết mã (TRÊN khối test của từng file)**

`services.rs`:

```rust
//! Kiểu khởi động dịch vụ qua Service Control Manager. Hoàn tác KHÔNG tự khởi động dịch vụ (spec 3.1).
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{ERROR_SERVICE_DOES_NOT_EXIST, ERROR_SERVICE_NOT_ACTIVE};
use windows::Win32::System::Services::{
    ChangeServiceConfigW, CloseServiceHandle, ControlService, OpenSCManagerW, OpenServiceW, QueryServiceConfigW, ENUM_SERVICE_TYPE,
    QUERY_SERVICE_CONFIGW, SC_HANDLE, SC_MANAGER_CONNECT, SERVICE_AUTO_START, SERVICE_CHANGE_CONFIG, SERVICE_CONTROL_STOP,
    SERVICE_DEMAND_START, SERVICE_DISABLED, SERVICE_ERROR, SERVICE_NO_CHANGE, SERVICE_QUERY_CONFIG, SERVICE_STATUS, SERVICE_STOP,
};

use crate::model::StartType;

struct Handle(SC_HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseServiceHandle(self.0);
        }
    }
}

/// `Ok(None)` = dịch vụ không tồn tại.
fn open(name: &str, access: u32) -> Result<Option<(Handle, Handle)>, String> {
    let scm = Handle(unsafe { OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT) }.map_err(|e| format!("OpenSCManager: {}", e.message()))?);
    match unsafe { OpenServiceW(scm.0, &HSTRING::from(name), access) } {
        Ok(h) => Ok(Some((scm, Handle(h)))),
        Err(e) if e.code() == ERROR_SERVICE_DOES_NOT_EXIST.to_hresult() => Ok(None),
        Err(e) => Err(format!("{name}: {}", e.message())),
    }
}

pub fn start_type(name: &str) -> Result<Option<StartType>, String> {
    let Some((_scm, svc)) = open(name, SERVICE_QUERY_CONFIG)? else { return Ok(None) };
    let mut needed = 0u32;
    let _ = unsafe { QueryServiceConfigW(svc.0, None, 0, &mut needed) };
    // u64 để vùng đệm canh lề 8 byte cho QUERY_SERVICE_CONFIGW.
    let mut buf = vec![0u64; (needed as usize).div_ceil(8).max(1)];
    let cfg = buf.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW;
    unsafe { QueryServiceConfigW(svc.0, Some(cfg), (buf.len() * 8) as u32, &mut needed) }.map_err(|e| format!("{name}: {}", e.message()))?;
    let st = unsafe { (*cfg).dwStartType };
    match st {
        SERVICE_AUTO_START => Ok(Some(StartType::Auto)),
        SERVICE_DEMAND_START => Ok(Some(StartType::Manual)),
        SERVICE_DISABLED => Ok(Some(StartType::Disabled)),
        other => Err(format!("{name}: unsupported start type {}", other.0)),
    }
}

pub fn set_start_type(name: &str, start: StartType) -> Result<(), String> {
    let Some((_scm, svc)) = open(name, SERVICE_CHANGE_CONFIG)? else { return Err(format!("{name}: service not found")) };
    let st = match start {
        StartType::Auto => SERVICE_AUTO_START,
        StartType::Manual => SERVICE_DEMAND_START,
        StartType::Disabled => SERVICE_DISABLED,
    };
    unsafe {
        ChangeServiceConfigW(
            svc.0,
            ENUM_SERVICE_TYPE(SERVICE_NO_CHANGE),
            st,
            SERVICE_ERROR(SERVICE_NO_CHANGE),
            PCWSTR::null(),
            PCWSTR::null(),
            None,
            PCWSTR::null(),
            PCWSTR::null(),
            PCWSTR::null(),
            PCWSTR::null(),
        )
    }
    .map_err(|e| format!("{name}: {}", e.message()))
}

/// Dịch vụ vốn đã dừng hoặc không tồn tại ⇒ `Ok(())`. Không chờ dừng hẳn.
pub fn stop(name: &str) -> Result<(), String> {
    let Some((_scm, svc)) = open(name, SERVICE_STOP)? else { return Ok(()) };
    let mut status = SERVICE_STATUS::default();
    match unsafe { ControlService(svc.0, SERVICE_CONTROL_STOP, &mut status) } {
        Ok(()) => Ok(()),
        Err(e) if e.code() == ERROR_SERVICE_NOT_ACTIVE.to_hresult() => Ok(()),
        Err(e) => Err(format!("{name}: {}", e.message())),
    }
}
```

`tasks.rs`:

```rust
//! Bật/tắt tác vụ theo lịch qua COM `ITaskService` (không phân tích chữ của `schtasks`, vốn đổi theo ngôn ngữ Windows).
use windows::core::BSTR;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, VARIANT_BOOL};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::System::TaskScheduler::{IRegisteredTask, ITaskService, TaskScheduler};
use windows::Win32::System::Variant::VARIANT;

/// Khởi tạo COM cho luồng hiện tại; chỉ gỡ nếu chính mình đã khởi tạo thành công.
struct Com(bool);

impl Com {
    fn init() -> Com {
        Com(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

fn not_found(e: &windows::core::Error) -> bool {
    e.code() == ERROR_FILE_NOT_FOUND.to_hresult() || e.code() == ERROR_PATH_NOT_FOUND.to_hresult()
}

/// `\A\B\Tên` ⇒ (`\A\B`, `Tên`); `\Tên` ⇒ (`\`, `Tên`).
pub fn split_task_path(path: &str) -> Result<(String, String), String> {
    match path.rsplit_once('\\') {
        Some((folder, name)) if !name.is_empty() => Ok((if folder.is_empty() { "\\".into() } else { folder.into() }, name.into())),
        _ => Err(format!("bad task path: {path}")),
    }
}

fn with_task<T>(path: &str, f: impl FnOnce(&IRegisteredTask) -> Result<T, String>) -> Result<Option<T>, String> {
    let (folder, name) = split_task_path(path)?;
    let _com = Com::init();
    let svc: ITaskService = unsafe { CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER) }.map_err(|e| format!("TaskScheduler: {}", e.message()))?;
    let empty = VARIANT::default();
    unsafe { svc.Connect(&empty, &empty, &empty, &empty) }.map_err(|e| format!("TaskScheduler: {}", e.message()))?;
    let folder = match unsafe { svc.GetFolder(&BSTR::from(folder.as_str())) } {
        Ok(f) => f,
        Err(e) if not_found(&e) => return Ok(None),
        Err(e) => return Err(format!("{path}: {}", e.message())),
    };
    let task = match unsafe { folder.GetTask(&BSTR::from(name.as_str())) } {
        Ok(t) => t,
        Err(e) if not_found(&e) => return Ok(None),
        Err(e) => return Err(format!("{path}: {}", e.message())),
    };
    f(&task).map(Some)
}

/// `Ok(None)` = tác vụ không có trên máy.
pub fn enabled(path: &str) -> Result<Option<bool>, String> {
    with_task(path, |t| unsafe { t.Enabled() }.map(|b| b.as_bool()).map_err(|e| format!("{path}: {}", e.message())))
}

pub fn set_enabled(path: &str, on: bool) -> Result<(), String> {
    match with_task(path, |t| unsafe { t.SetEnabled(VARIANT_BOOL::from(on)) }.map_err(|e| format!("{path}: {}", e.message())))? {
        Some(()) => Ok(()),
        None => Err(format!("{path}: task not found")),
    }
}
```

- [ ] **Step 4: Chạy test và clippy, thấy qua**

Run: `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml -- sys_windows::services sys_windows::tasks` rồi `cargo clippy --manifest-path crates/winfreeup-tweaks/Cargo.toml --all-targets -- -D warnings`
Expected: `3 passed`; clippy sạch.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-tweaks/src/sys_windows/services.rs
rtk git add crates/winfreeup-tweaks/src/sys_windows/tasks.rs
rtk git commit -m "feat(tweaks): kiểu khởi động dịch vụ qua SCM, bật/tắt tác vụ theo lịch qua ITaskService

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Windows thật — gói app (`PackageManager`) và `RealTweakOps`

**Files:**
- Modify: `crates/winfreeup-tweaks/src/sys_windows/appx.rs` (thay dòng khung)
- Modify: `crates/winfreeup-tweaks/src/sys_windows/real.rs` (thay dòng khung)
- Test: hai file trên (module `tests`) — chỉ **đọc**; gỡ app thật được thử tay ở Task 12.

**Interfaces:**
- Consumes: Task 1 — `PackageInfo`, `RegData`, `StartType`, `SystemInfo`, `TweakOps` (14 hàm); Task 2 — `catalog::builtin()`; Task 5 — `engine::read_all`; Task 6 — `registry::{read, write, delete}`, `info::system_info`, `shell::{open_store_uri, restart_explorer}`; Task 7 — `services::{start_type, set_start_type, stop}`, `tasks::{enabled, set_enabled}`. Crate `windows` 0.62 (`ApplicationModel`, `Management_Deployment`, `Foundation`, `Foundation_Collections`) + `windows-future` 0.3 (`IAsyncOperationWithProgress::join`).
- Produces (vỏ Tauri ở Task 12 dùng nguyên văn):
  - `appx::{packages(family: &str) -> Result<Vec<PackageInfo>, String>, remove(full_name: &str, all_users: bool) -> Result<(), String>, deprovision(family: &str) -> Result<(), String>}` — `packages` chỉ trả gói của người dùng đang chạy tiến trình (`FindPackagesByUserSecurityIdPackageFamilyName` với SID rỗng), không cần Admin; `non_removable` = `SignatureKind == System`.
  - `real::RealTweakOps` (struct đơn vị) — `impl TweakOps`, chỉ chuyển tiếp tới các hàm trên.

Ghi chú đo trên máy dev (185 gói): mọi gói ký `System` (49) đều `NonRemovable` trong `Get-AppxPackage`; chỉ hai gói `NonRemovable` không ký `System` là `Microsoft.SecHealthUI` và `Microsoft.DesktopAppInstaller` — cả hai đã nằm trong `BLOCKED_PACKAGES` (Task 2). API WinRT công khai không có `IsNonRemovable`, nên dùng `SignatureKind`.

- [ ] **Step 1: Viết test hỏng**

`sys_windows/appx.rs` — chỉ phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_packages_without_changing_anything() {
        assert!(packages("WinFreeUp.NoSuchPackage_0000000000000").unwrap().is_empty());
        // Vỏ Start/Taskbar có trên mọi bản Windows có giao diện; ký System ⇒ không gỡ được.
        let shell = packages("Microsoft.Windows.ShellExperienceHost_cw5n1h2txyewy").unwrap();
        assert_eq!(shell.len(), 1, "{shell:?}");
        assert!(shell[0].non_removable);
        assert!(!shell[0].is_framework);
        assert!(shell[0].full_name.starts_with("Microsoft.Windows.ShellExperienceHost_"));
    }
}
```

`sys_windows/real.rs` — chỉ phần test (đọc toàn bộ danh mục thật trên máy đang chạy test, không đổi gì):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::builtin;
    use crate::engine::read_all;

    /// Đọc trạng thái toàn bộ danh mục thật trên máy này — chỉ ĐỌC, không đổi gì.
    #[test]
    fn reads_whole_builtin_catalog_on_this_machine() {
        let d = tempfile::tempdir().unwrap();
        let r = read_all(&builtin().unwrap(), &RealTweakOps, &d.path().join("u.json")).unwrap();
        assert_eq!(r.tweaks.len(), builtin().unwrap().len());
        let errors: Vec<&String> = r.tweaks.iter().flat_map(|t| t.errors.iter()).collect();
        assert!(errors.is_empty(), "{errors:?}");
        assert!(!d.path().join("u.json").exists(), "đọc không tạo file hoàn tác");
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml -- sys_windows::appx sys_windows::real`
Expected: FAIL biên dịch — `cannot find function packages`, `cannot find value RealTweakOps`.

- [ ] **Step 3: Viết mã (TRÊN khối test của từng file)**

`appx.rs`:

```rust
//! Gói app (MSIX/Appx) qua WinRT `PackageManager` — phạm vi tài khoản hiện tại, hoặc mọi tài khoản.
use windows::core::HSTRING;
use windows::ApplicationModel::PackageSignatureKind;
use windows::Management::Deployment::{DeploymentProgress, DeploymentResult, PackageManager, RemovalOptions};
use windows_future::IAsyncOperationWithProgress;

use crate::ops::PackageInfo;

fn msg(what: &str, e: windows::core::Error) -> String {
    format!("{what}: {}", e.message())
}

/// Gói của người dùng đang chạy tiến trình thuộc `family`. Không cần Admin.
pub fn packages(family: &str) -> Result<Vec<PackageInfo>, String> {
    let pm = PackageManager::new().map_err(|e| msg("PackageManager", e))?;
    let found = pm.FindPackagesByUserSecurityIdPackageFamilyName(&HSTRING::new(), &HSTRING::from(family)).map_err(|e| msg(family, e))?;
    let mut out = Vec::new();
    for p in found {
        let id = p.Id().map_err(|e| msg(family, e))?;
        out.push(PackageInfo {
            full_name: id.FullName().map_err(|e| msg(family, e))?.to_string(),
            family: id.FamilyName().map_err(|e| msg(family, e))?.to_string(),
            is_framework: p.IsFramework().map_err(|e| msg(family, e))?,
            // Đo trên máy dev: mọi gói ký System đều NonRemovable; hai gói Store còn lại
            // (SecHealthUI, DesktopAppInstaller) đã nằm trong danh sách cấm gỡ.
            non_removable: p.SignatureKind().map_err(|e| msg(family, e))? == PackageSignatureKind::System,
        });
    }
    Ok(out)
}

/// Chờ thao tác triển khai xong; lỗi ⇒ `ErrorText` nguyên văn của Windows nếu có.
fn finish(what: &str, op: IAsyncOperationWithProgress<DeploymentResult, DeploymentProgress>) -> Result<(), String> {
    match op.join() {
        Ok(_) => Ok(()),
        Err(e) => {
            let text = op.GetResults().and_then(|r| r.ErrorText()).map(|t| t.to_string()).unwrap_or_default();
            Err(if text.trim().is_empty() { msg(what, e) } else { format!("{what}: {}", text.trim()) })
        }
    }
}

pub fn remove(full_name: &str, all_users: bool) -> Result<(), String> {
    let pm = PackageManager::new().map_err(|e| msg("PackageManager", e))?;
    let opts = if all_users { RemovalOptions::RemoveForAllUsers } else { RemovalOptions::None };
    let op = pm.RemovePackageWithOptionsAsync(&HSTRING::from(full_name), opts).map_err(|e| msg(full_name, e))?;
    finish(full_name, op)
}

/// Chặn cài lại cho tài khoản mới (cần Admin).
pub fn deprovision(family: &str) -> Result<(), String> {
    let pm = PackageManager::new().map_err(|e| msg("PackageManager", e))?;
    let op = pm.DeprovisionPackageForAllUsersAsync(&HSTRING::from(family)).map_err(|e| msg(family, e))?;
    finish(family, op)
}
```

`real.rs`:

```rust
//! `RealTweakOps`: nối trait `TweakOps` với các hàm Windows thật của từng file trong `sys_windows`.
use super::{appx, info, registry, services, shell, tasks};
use crate::model::StartType;
use crate::ops::{PackageInfo, RegData, SystemInfo, TweakOps};

pub struct RealTweakOps;

impl TweakOps for RealTweakOps {
    fn reg_read(&self, path: &str, name: &str) -> Result<Option<RegData>, String> {
        registry::read(path, name)
    }
    fn reg_write(&self, path: &str, name: &str, data: &RegData) -> Result<(), String> {
        registry::write(path, name, data)
    }
    fn reg_delete(&self, path: &str, name: &str) -> Result<(), String> {
        registry::delete(path, name)
    }
    fn service_start(&self, name: &str) -> Result<Option<StartType>, String> {
        services::start_type(name)
    }
    fn set_service_start(&self, name: &str, start: StartType) -> Result<(), String> {
        services::set_start_type(name, start)
    }
    fn stop_service(&self, name: &str) -> Result<(), String> {
        services::stop(name)
    }
    fn task_enabled(&self, path: &str) -> Result<Option<bool>, String> {
        tasks::enabled(path)
    }
    fn set_task_enabled(&self, path: &str, enabled: bool) -> Result<(), String> {
        tasks::set_enabled(path, enabled)
    }
    fn packages(&self, family: &str) -> Result<Vec<PackageInfo>, String> {
        appx::packages(family)
    }
    fn remove_package(&self, full_name: &str, all_users: bool) -> Result<(), String> {
        appx::remove(full_name, all_users)
    }
    fn deprovision(&self, family: &str) -> Result<(), String> {
        appx::deprovision(family)
    }
    fn open_uri(&self, uri: &str) -> Result<(), String> {
        shell::open_store_uri(uri)
    }
    fn system_info(&self) -> Result<SystemInfo, String> {
        info::system_info()
    }
    fn restart_explorer(&self) -> Result<(), String> {
        shell::restart_explorer()
    }
}
```

- [ ] **Step 4: Chạy toàn bộ test và clippy, thấy qua**

Run: `cargo test --manifest-path crates/winfreeup-tweaks/Cargo.toml` rồi `cargo clippy --manifest-path crates/winfreeup-tweaks/Cargo.toml --all-targets -- -D warnings`
Expected: `test result: ok. 58 passed; 0 failed` (toàn crate — 2 test mới của task này); clippy sạch. Nếu `reads_whole_builtin_catalog_on_this_machine` đỏ, thông điệp liệt kê lỗi đọc nguyên văn theo từng mục — đó là lỗi thật trên máy, không phải test sai.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-tweaks/src/sys_windows/appx.rs
rtk git add crates/winfreeup-tweaks/src/sys_windows/real.rs
rtk git commit -m "feat(tweaks): gỡ app qua PackageManager (tài khoản hiện tại / mọi tài khoản + deprovision), RealTweakOps

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Giao diện — kiểu dữ liệu, chuỗi, mức sẵn, nhãn

**Files:**
- Create: `src/features/tinh-chinh/types.ts`
- Create: `src/features/tinh-chinh/vi.json`
- Create: `src/features/tinh-chinh/strings.ts`
- Create: `src/features/tinh-chinh/testdata.ts`
- Create: `src/features/tinh-chinh/presets.ts`
- Create: `src/features/tinh-chinh/labels.ts`
- Test: `src/features/tinh-chinh/presets.test.ts`, `src/features/tinh-chinh/labels.test.ts`

**Interfaces:**
- Consumes: Task 2 — `src/features/tinh-chinh/catalog.vi.json` (khoá `tweaks.item.<id>.name/.desc`); v0.1 Task 10 — kiểu `RestorePointStatus` ở `src/api/types.ts`; hình JSON của Task 4–5 (`TweakView` phẳng có `status`/`reason`, `RunReport`, `TweakEvent`).
- Produces (Task 10, 11, 12 dùng nguyên văn):
  - `types.ts`: `TweakGroup`, `TweakLevel` (`'basic' | 'recommended' | 'aggressive'`), `TweakRisk`, `Restart`, `TweakStatus`, `TweakView`, `SystemInfo`, `ReadResult`, `TweakOutcome`, `RunReport`, `TweakEvent`, `interface TweakApi { read(); prepareRestorePoint(); apply(ids, allUsers, onEvent); revert(ids, onEvent); restartExplorer() }`.
  - `strings.ts`: `tt(key: string, params?: Record<string, string | number>): string` (thiếu khoá ⇒ trả khoá + `console.warn`), `hasKey(key)`, `type Params`.
  - `vi.json`: mọi khoá giao diện `tweaks.*` (không trùng khoá của `catalog.vi.json`).
  - `testdata.ts`: `tw(id, over?) => TweakView`, `sample(): TweakView[]` (8 mục: `ads_id`, `app_clipchamp` đã gỡ + có ảnh chụp, `svc_diagtrack` caution, `app_weather`, `recall_off` không hỗ trợ `build_min:26100`, `cloud_clipboard` một phần, `app_game_bar` caution, `app_tiktok` không có trên máy), `readResult(tweaks?, over?)`, `report(over?)`.
  - `presets.ts`: `LEVELS`, `type Preset = TweakLevel | 'custom'`, `visible(t)`, `selectable(t)`, `presetSelection(tweaks, level): string[]`, `currentPreset(tweaks, selected): Preset`, `pendingChanges(tweaks, selected): string[]`, `revertable(tweaks, selected): string[]`, `needsConfirm(tweaks, ids, allUsers): TweakView[]`.
  - `labels.ts`: `itemName(id)`, `itemDesc(id)`, `levelText(level)`, `reasonText(reason)`, `statusText(t)`, `outcomeText(o, kind: 'apply' | 'revert')`.

- [ ] **Step 1: Viết kiểu, chuỗi và dữ liệu mẫu (không có logic để test riêng)**

`src/features/tinh-chinh/types.ts`:

```ts
// Khớp serde của crate winfreeup-tweaks (tên trường snake_case) — đừng đổi tên.
import type { RestorePointStatus } from '../../api/types';

export type TweakGroup = 'privacy' | 'bloatware';
export type TweakLevel = 'basic' | 'recommended' | 'aggressive';
export type TweakRisk = 'safe' | 'caution';
export type Restart = 'none' | 'explorer' | 'logoff' | 'reboot';

export type TweakStatus =
  | { status: 'applied' }
  | { status: 'not_applied' }
  | { status: 'partial' }
  | { status: 'unsupported'; reason: string }
  | { status: 'managed' }
  | { status: 'not_present' };

export type TweakView = {
  id: string;
  group: TweakGroup;
  level: TweakLevel;
  risk: TweakRisk;
  needs_restart: Restart;
  has_undo: boolean;
  errors: string[];
} & TweakStatus;

export interface SystemInfo {
  build: number;
  edition: string;
  managed: boolean;
  other_user: boolean;
}

export interface ReadResult {
  system: SystemInfo;
  tweaks: TweakView[];
  /** Mã thông báo, vd `undo_corrupt:<chi tiết>`. */
  notices: string[];
}

export type TweakOutcome = { id: string; errors: string[]; store_opened: string[] } & TweakStatus;

export interface RunReport {
  outcomes: TweakOutcome[];
  restart: Restart;
  notices: string[];
}

export type TweakEvent =
  | { kind: 'started'; id: string; index: number; total: number }
  | { kind: 'finished'; outcome: TweakOutcome };

/** Cầu nối tới vỏ Tauri cho tab Tinh chỉnh. Bản thật ở `api.ts`; test dùng bản giả. */
export interface TweakApi {
  read(): Promise<ReadResult>;
  prepareRestorePoint(): Promise<RestorePointStatus>;
  apply(ids: string[], allUsers: boolean, onEvent: (e: TweakEvent) => void): Promise<RunReport>;
  revert(ids: string[], onEvent: (e: TweakEvent) => void): Promise<RunReport>;
  restartExplorer(): Promise<void>;
}
```

`src/features/tinh-chinh/vi.json`:

```json
{
  "tweaks.tab": "Tinh chỉnh",
  "tweaks.loading": "Đang đọc trạng thái máy… (liệt kê app mất vài giây)",
  "tweaks.reloading": "Đang đọc lại trạng thái…",
  "tweaks.retry": "Đọc lại",
  "tweaks.preset.label": "Mức sẵn",
  "tweaks.preset.basic": "Cơ bản",
  "tweaks.preset.recommended": "Khuyến nghị",
  "tweaks.preset.aggressive": "Triệt để",
  "tweaks.preset.custom": "Tùy chỉnh",
  "tweaks.group.bloatware": "Gỡ app ({present} đang có · {removed} đã gỡ)",
  "tweaks.group.privacy": "Quyền riêng tư & quảng cáo",
  "tweaks.status.applied": "Đã áp dụng",
  "tweaks.status.removed": "Đã gỡ",
  "tweaks.status.notApplied": "Chưa áp dụng",
  "tweaks.status.notRemoved": "Chưa gỡ",
  "tweaks.status.partial": "Một phần",
  "tweaks.status.managed": "Do tổ chức quản lý",
  "tweaks.status.unsupported": "Không hỗ trợ trên máy này — {reason}",
  "tweaks.reason.buildMin": "cần Windows build {build} trở lên",
  "tweaks.reason.win11": "cần Windows 11",
  "tweaks.reason.win11_24h2": "cần Windows 11 24H2 trở lên",
  "tweaks.reason.win10Only": "chỉ dành cho Windows 10",
  "tweaks.reason.edition": "không áp dụng cho bản Windows này",
  "tweaks.reason.missing": "thành phần này không có trên máy",
  "tweaks.caution": "Cân nhắc",
  "tweaks.readError": "Không đọc được: {message}",
  "tweaks.reinstall": "Cài lại từ Store",
  "tweaks.allUsers": "Nâng cao: gỡ cho mọi tài khoản và chặn cài lại",
  "tweaks.allUsersWarn": "Khó hoàn tác; bản cập nhật Windows lớn có thể vẫn cài lại một số app.",
  "tweaks.apply": "Áp dụng {count} thay đổi",
  "tweaks.revert": "Hoàn tác đã chọn ({count})",
  "tweaks.dryRun": "Chạy thử: tab này chỉ xem trạng thái, không áp dụng hay hoàn tác.",
  "tweaks.otherUser": "WinFreeUp đang chạy bằng một tài khoản quản trị khác với người đang đăng nhập. Các mục theo tài khoản (quảng cáo, gỡ app) sẽ áp cho tài khoản quản trị đó, không phải cho bạn.",
  "tweaks.managedMachine": "Máy này do tổ chức quản lý; mục nào chính sách của tổ chức đã đặt thì bị khóa.",
  "tweaks.confirm.title": "Xác nhận trước khi áp dụng",
  "tweaks.confirm.caution": "Các mục sau cần bạn cân nhắc:",
  "tweaks.confirm.allUsers": "Gỡ cho mọi tài khoản và chặn cài lại — khó hoàn tác; bản cập nhật Windows lớn có thể vẫn cài lại một số app.",
  "tweaks.confirm.accept": "Áp dụng",
  "tweaks.confirm.cancel": "Quay lại",
  "tweaks.restore.creating": "Đang tạo điểm khôi phục hệ thống…",
  "tweaks.restore.failedTitle": "Không tạo được điểm khôi phục",
  "tweaks.restore.failedBody": "Nếu tiếp tục, bạn sẽ không dùng System Restore để quay lại được; mọi mục vẫn hoàn tác được ngay trong WinFreeUp. Chi tiết: {message}",
  "tweaks.restore.continue": "Vẫn áp dụng",
  "tweaks.restore.abort": "Dừng lại",
  "tweaks.run.apply": "Đang áp dụng {done}/{total}…",
  "tweaks.run.revert": "Đang hoàn tác {done}/{total}…",
  "tweaks.result.title": "Kết quả",
  "tweaks.result.ok": "✓ Xong",
  "tweaks.result.partial": "⚠ Một phần",
  "tweaks.result.failed": "✗ Lỗi",
  "tweaks.result.storeOpened": "Đã mở trang Store để bạn cài lại.",
  "tweaks.result.explorer": "Khởi động lại Explorer",
  "tweaks.result.explorerBusy": "Đang khởi động lại Explorer…",
  "tweaks.result.logoff": "Đăng xuất rồi đăng nhập lại để hoàn tất.",
  "tweaks.result.reboot": "Khởi động lại máy để hoàn tất.",
  "tweaks.result.close": "Đóng",
  "tweaks.errors.busy": "Đang có thao tác khác, chờ xong rồi thử lại.",
  "tweaks.errors.dryRun": "Đang chạy thử — không thay đổi máy.",
  "tweaks.errors.loadFailed": "Không đọc được trạng thái máy: {message}",
  "tweaks.errors.applyFailed": "Áp dụng không xong: {message}",
  "tweaks.errors.revertFailed": "Hoàn tác không xong: {message}",
  "tweaks.errors.explorerFailed": "Không khởi động lại được Explorer: {message}",
  "tweaks.notice.undoCorrupt": "Không đọc được dữ liệu hoàn tác; hoàn tác sẽ dùng giá trị mặc định của Windows. Chi tiết: {detail}"
}
```

`src/features/tinh-chinh/strings.ts`:

```ts
// Chuỗi của tab Tinh chỉnh. Trước Task Tích hợp: đọc hai file JSON riêng của tính năng
// (không đụng src/i18n/vi.json đang do v0.1 sở hữu). Task Tích hợp thay cả file bằng một dòng re-export `t`.
import ui from './vi.json';
import items from './catalog.vi.json';

const dict: Record<string, string> = { ...ui, ...items };

export type Params = Record<string, string | number>;

export function hasKey(key: string): boolean {
  return Object.prototype.hasOwnProperty.call(dict, key);
}

export function tt(key: string, params?: Params): string {
  if (!hasKey(key)) {
    console.warn(`[i18n] thiếu khoá: ${key}`);
    return key;
  }
  return dict[key].replace(/\{(\w+)\}/g, (whole, name: string) => (params && name in params ? String(params[name]) : whole));
}
```

`src/features/tinh-chinh/testdata.ts`:

```ts
// Dữ liệu mẫu cho test của tab Tinh chỉnh.
import type { ReadResult, RunReport, TweakView } from './types';

export function tw(id: string, over: Partial<TweakView> = {}): TweakView {
  return {
    id,
    group: 'privacy',
    level: 'basic',
    risk: 'safe',
    needs_restart: 'none',
    has_undo: false,
    errors: [],
    status: 'not_applied',
    ...over,
  } as TweakView;
}

/** Hai mục mỗi mức + một mục không hỗ trợ + một app không có trên máy. */
export function sample(): TweakView[] {
  return [
    tw('ads_id'),
    tw('app_clipchamp', { group: 'bloatware', status: 'applied', has_undo: true }),
    tw('svc_diagtrack', { level: 'recommended', risk: 'caution' }),
    tw('app_weather', { group: 'bloatware', level: 'recommended' }),
    tw('recall_off', { level: 'recommended', status: 'unsupported', reason: 'build_min:26100' } as Partial<TweakView>),
    tw('cloud_clipboard', { level: 'aggressive', status: 'partial' }),
    tw('app_game_bar', { group: 'bloatware', level: 'aggressive', risk: 'caution' }),
    tw('app_tiktok', { group: 'bloatware', status: 'not_present' }),
  ];
}

export function readResult(tweaks: TweakView[] = sample(), over: Partial<ReadResult> = {}): ReadResult {
  return { system: { build: 26200, edition: 'Pro', managed: false, other_user: false }, tweaks, notices: [], ...over };
}

export function report(over: Partial<RunReport> = {}): RunReport {
  return { outcomes: [], restart: 'none', notices: [], ...over };
}
```

- [ ] **Step 2: Viết test hỏng**

`src/features/tinh-chinh/presets.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { currentPreset, needsConfirm, pendingChanges, presetSelection, revertable, selectable, visible } from './presets';
import { sample, tw } from './testdata';

const all = sample();

describe('mức sẵn', () => {
  it('Cơ bản ⊂ Khuyến nghị ⊂ Triệt để, bỏ mục không hỗ trợ và app không có', () => {
    expect(presetSelection(all, 'basic')).toEqual(['ads_id', 'app_clipchamp']);
    expect(presetSelection(all, 'recommended')).toEqual(['ads_id', 'app_clipchamp', 'svc_diagtrack', 'app_weather']);
    expect(presetSelection(all, 'aggressive')).toEqual([
      'ads_id',
      'app_clipchamp',
      'svc_diagtrack',
      'app_weather',
      'cloud_clipboard',
      'app_game_bar',
    ]);
  });

  it('mục bị tổ chức quản lý không bao giờ được tích', () => {
    const list = [tw('a'), tw('b', { status: 'managed' })];
    expect(presetSelection(list, 'aggressive')).toEqual(['a']);
    expect(selectable(list[1])).toBe(false);
  });

  it('nhận diện mức đang khớp, không phụ thuộc thứ tự tích', () => {
    expect(currentPreset(all, ['app_clipchamp', 'ads_id'])).toBe('basic');
    expect(currentPreset(all, presetSelection(all, 'aggressive'))).toBe('aggressive');
    expect(currentPreset(all, ['ads_id'])).toBe('custom');
    expect(currentPreset(all, [])).toBe('custom');
  });

  it('danh mục chỉ có mục Cơ bản ⇒ ba mức trùng nhau ⇒ báo mức thấp nhất', () => {
    const list = [tw('a'), tw('b')];
    expect(currentPreset(list, ['a', 'b'])).toBe('basic');
  });
});

describe('đếm thay đổi', () => {
  it('bỏ qua mục đã ở trạng thái đích', () => {
    expect(pendingChanges(all, ['ads_id', 'app_clipchamp', 'cloud_clipboard'])).toEqual(['ads_id', 'cloud_clipboard']);
  });

  it('hoàn tác chỉ tính mục có gì để trả', () => {
    expect(revertable(all, ['ads_id', 'app_clipchamp', 'cloud_clipboard', 'recall_off'])).toEqual(['app_clipchamp', 'cloud_clipboard']);
  });

  it('app không có trên máy bị ẩn', () => {
    expect(all.filter(visible).map((t) => t.id)).not.toContain('app_tiktok');
  });
});

describe('hộp xác nhận', () => {
  it('liệt kê mục caution; bật «mọi tài khoản» thì thêm mọi app sẽ gỡ', () => {
    expect(needsConfirm(all, ['ads_id', 'svc_diagtrack', 'app_weather'], false).map((t) => t.id)).toEqual(['svc_diagtrack']);
    expect(needsConfirm(all, ['ads_id', 'svc_diagtrack', 'app_weather'], true).map((t) => t.id)).toEqual(['svc_diagtrack', 'app_weather']);
    expect(needsConfirm(all, ['ads_id'], false)).toEqual([]);
  });
});
```

`src/features/tinh-chinh/labels.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import catalogVi from './catalog.vi.json';
import ui from './vi.json';
import { itemName, outcomeText, reasonText, statusText } from './labels';
import { tw } from './testdata';

describe('nhãn', () => {
  it('trạng thái app nói «gỡ», quyền riêng tư nói «áp dụng»', () => {
    expect(statusText(tw('a', { group: 'bloatware', status: 'applied' }))).toBe('Đã gỡ');
    expect(statusText(tw('a', { group: 'bloatware' }))).toBe('Chưa gỡ');
    expect(statusText(tw('a'))).toBe('Chưa áp dụng');
    expect(statusText(tw('a', { status: 'partial' }))).toBe('Một phần');
  });

  it('lý do không hỗ trợ đọc được', () => {
    expect(reasonText('build_min:26100')).toBe('cần Windows 11 24H2 trở lên');
    expect(reasonText('build_min:22000')).toBe('cần Windows 11');
    expect(reasonText('build_min:19041')).toBe('cần Windows build 19041 trở lên');
    expect(reasonText('build_max:19045')).toBe('chỉ dành cho Windows 10');
    expect(reasonText('edition')).toBe('không áp dụng cho bản Windows này');
    expect(reasonText('la_chua_biet')).toBe('la_chua_biet');
    expect(statusText(tw('r', { status: 'unsupported', reason: 'build_min:26100' } as never))).toBe(
      'Không hỗ trợ trên máy này — cần Windows 11 24H2 trở lên',
    );
  });

  it('tên mục lấy từ danh mục tiếng Việt', () => {
    expect(itemName('app_clipchamp')).toBe('Clipchamp');
  });

  it('kết quả từng mục', () => {
    const o = (status: 'applied' | 'partial' | 'not_applied', errors: string[] = []) => ({ id: 'x', status, errors, store_opened: [] }) as never;
    expect(outcomeText(o('applied'), 'apply')).toBe('✓ Xong');
    expect(outcomeText(o('partial', ['x#0: Access is denied.']), 'apply')).toBe('⚠ Một phần');
    expect(outcomeText(o('not_applied', ['x#0: Access is denied.']), 'apply')).toBe('✗ Lỗi');
    expect(outcomeText(o('not_applied'), 'revert')).toBe('✓ Xong');
  });

  it('không có chuỗi rỗng, và khoá UI không trùng khoá danh mục', () => {
    for (const [k, v] of Object.entries({ ...ui, ...catalogVi })) expect(v.trim(), k).not.toBe('');
    for (const k of Object.keys(ui)) expect(k in catalogVi, k).toBe(false);
  });
});
```

- [ ] **Step 3: Chạy test, thấy hỏng**

Run: `npx vitest run src/features/tinh-chinh/presets.test.ts src/features/tinh-chinh/labels.test.ts`
Expected: FAIL — `Failed to resolve import "./presets"`, `"./labels"`.

- [ ] **Step 4: Viết mã**

`src/features/tinh-chinh/presets.ts`:

```ts
import type { TweakLevel, TweakView } from './types';

export const LEVELS: TweakLevel[] = ['basic', 'recommended', 'aggressive'];
export type Preset = TweakLevel | 'custom';

const rank = (l: TweakLevel) => LEVELS.indexOf(l);

/** App không có trên máy và chưa từng bị gỡ ⇒ không hiện (spec 4.1). */
export function visible(t: TweakView): boolean {
  return t.status !== 'not_present';
}

/** Chỉ mục đọc được trạng thái bình thường mới được tích (không hỗ trợ / bị quản lý ⇒ không bao giờ). */
export function selectable(t: TweakView): boolean {
  return t.status === 'applied' || t.status === 'not_applied' || t.status === 'partial';
}

/** Bấm mức sẵn ⇒ tích mọi mục có `level` ≤ mức đó (spec mục 5), theo thứ tự danh mục. */
export function presetSelection(tweaks: TweakView[], level: TweakLevel): string[] {
  return tweaks.filter((t) => selectable(t) && rank(t.level) <= rank(level)).map((t) => t.id);
}

function sameSet(a: string[], b: string[]): boolean {
  if (a.length !== b.length) return false;
  const s = new Set(a);
  return b.every((x) => s.has(x));
}

/** Mức đang khớp đúng tập đã tích; không khớp mức nào ⇒ `custom` («Tùy chỉnh»). Trùng nhiều mức ⇒ mức thấp nhất. */
export function currentPreset(tweaks: TweakView[], selected: string[]): Preset {
  if (selected.length === 0) return 'custom';
  return LEVELS.find((l) => sameSet(presetSelection(tweaks, l), selected)) ?? 'custom';
}

/** Số thay đổi thật: mục đã tích mà chưa ở trạng thái đích. */
export function pendingChanges(tweaks: TweakView[], selected: string[]): string[] {
  const s = new Set(selected);
  return tweaks.filter((t) => s.has(t.id) && selectable(t) && t.status !== 'applied').map((t) => t.id);
}

/** Mục đã tích có gì để hoàn tác: có ảnh chụp, hoặc đang ở trạng thái đã áp dụng/một phần. */
export function revertable(tweaks: TweakView[], selected: string[]): string[] {
  const s = new Set(selected);
  return tweaks
    .filter((t) => s.has(t.id) && selectable(t) && (t.has_undo || t.status === 'applied' || t.status === 'partial'))
    .map((t) => t.id);
}

/** Mục cần hộp xác nhận: `caution`, và mọi app sẽ gỡ khi bật «mọi tài khoản» (spec mục 5). */
export function needsConfirm(tweaks: TweakView[], ids: string[], allUsers: boolean): TweakView[] {
  const s = new Set(ids);
  return tweaks.filter((t) => s.has(t.id) && (t.risk === 'caution' || (allUsers && t.group === 'bloatware')));
}
```

`src/features/tinh-chinh/labels.ts`:

```ts
import { tt } from './strings';
import type { TweakLevel, TweakOutcome, TweakView } from './types';

export function itemName(id: string): string {
  return tt(`tweaks.item.${id}.name`);
}

export function itemDesc(id: string): string {
  return tt(`tweaks.item.${id}.desc`);
}

export function levelText(l: TweakLevel): string {
  return tt(`tweaks.preset.${l}`);
}

/** Mã lý do của lõi (`build_min:26100`, `build_max:19045`, `edition`, `missing`) ⇒ câu cho người đọc. */
export function reasonText(reason: string): string {
  const [kind, arg] = reason.split(':');
  if (kind === 'build_min') {
    const build = Number(arg);
    if (build === 22000) return tt('tweaks.reason.win11');
    if (build === 26100) return tt('tweaks.reason.win11_24h2');
    return tt('tweaks.reason.buildMin', { build: arg ?? '?' });
  }
  if (kind === 'build_max') return tt('tweaks.reason.win10Only');
  if (kind === 'edition') return tt('tweaks.reason.edition');
  if (kind === 'missing') return tt('tweaks.reason.missing');
  return reason;
}

export function statusText(t: TweakView): string {
  const app = t.group === 'bloatware';
  switch (t.status) {
    case 'applied':
      return tt(app ? 'tweaks.status.removed' : 'tweaks.status.applied');
    case 'not_applied':
      return tt(app ? 'tweaks.status.notRemoved' : 'tweaks.status.notApplied');
    case 'partial':
      return tt('tweaks.status.partial');
    case 'managed':
      return tt('tweaks.status.managed');
    case 'unsupported':
      return tt('tweaks.status.unsupported', { reason: reasonText(t.reason) });
    case 'not_present':
      return '';
  }
}

/** ✓ khi không lỗi và (áp dụng xong hoặc hoàn tác xong); ⚠ khi còn một phần; ✗ khi có lỗi. */
export function outcomeText(o: TweakOutcome, kind: 'apply' | 'revert'): string {
  if (o.errors.length > 0) return o.status === 'partial' ? tt('tweaks.result.partial') : tt('tweaks.result.failed');
  if (kind === 'apply' && o.status === 'partial') return tt('tweaks.result.partial');
  return tt('tweaks.result.ok');
}
```

- [ ] **Step 5: Chạy test và typecheck, thấy qua**

Run: `npx vitest run src/features/tinh-chinh/presets.test.ts src/features/tinh-chinh/labels.test.ts` rồi `npm run typecheck`
Expected: `Tests  13 passed (13)` (8 presets + 5 labels); typecheck không lỗi.

- [ ] **Step 6: Commit**

```bash
rtk git add src/features/tinh-chinh/types.ts
rtk git add src/features/tinh-chinh/vi.json
rtk git add src/features/tinh-chinh/strings.ts
rtk git add src/features/tinh-chinh/testdata.ts
rtk git add src/features/tinh-chinh/presets.ts
rtk git add src/features/tinh-chinh/presets.test.ts
rtk git add src/features/tinh-chinh/labels.ts
rtk git add src/features/tinh-chinh/labels.test.ts
rtk git commit -m "feat(ui-tinh-chinh): kiểu dữ liệu, chuỗi tiếng Việt, mức sẵn Cơ bản/Khuyến nghị/Triệt để, nhãn trạng thái

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Giao diện — máy trạng thái, cầu nối Tauri, bộ điều khiển

**Files:**
- Create: `src/features/tinh-chinh/reducer.ts`
- Create: `src/features/tinh-chinh/api.ts`
- Create: `src/features/tinh-chinh/controller.ts`
- Test: `src/features/tinh-chinh/reducer.test.ts`, `src/features/tinh-chinh/api.test.ts`, `src/features/tinh-chinh/controller.test.ts`

**Interfaces:**
- Consumes:
  - Task 9: `TweakApi`, `ReadResult`, `RunReport`, `TweakEvent`, `TweakLevel` (`types.ts`); `needsConfirm`, `pendingChanges`, `presetSelection`, `selectable`, `revertable` (`presets.ts`); `tt` (`strings.ts`); `readResult`, `report`, `sample` (`testdata.ts`).
  - v0.1 Task 10: `RestorePointStatus` (`src/api/types.ts`). v0.1 Task 12: `messageOf`, `type Severity` (`src/errors/errors.ts`). v0.1 Task 13: `createStore`, `type Store` (`src/state/store.ts`) — **phải gộp `feat/v0.1-don-o-dia` vào `feat/tinh-chinh` trước đợt này** (mục «Đồng bộ với v0.1»).
  - Vỏ Tauri (Task 12) sẽ cung cấp đúng: lệnh `tweaks_read`, `tweaks_prepare_restore_point`, `tweaks_apply {ids, allUsers}`, `tweaks_revert {ids}`, `tweaks_restart_explorer`; sự kiện `tweak-progress` (payload `TweakEvent`). Lỗi lệnh là **chuỗi**: `"busy"` (đang có thao tác khác), `"dry_run"` (chế độ chạy thử), còn lại nguyên văn.
- Produces (Task 11, 12 dùng nguyên văn):
  - `reducer.ts`: `type Phase = 'loading' | 'ready' | 'confirm' | 'restorePoint' | 'running' | 'done'`, `type RunKind = 'apply' | 'revert'`, `interface RunState { kind; ids; current: string | null; finished: string[] }`, `interface TState { phase; data: ReadResult | null; reloading; loadError: string | null; selected: string[]; touched; allUsers; restore: RestorePointStatus | null; run: RunState | null; report: RunReport | null }`, `initialTState`, `DEFAULT_LEVEL = 'basic'`, `type TAction` (17 loại — xem mã), `reducer(s, a)` — hành động không hợp phase ⇒ trả nguyên `s` (cùng tham chiếu).
  - `api.ts`: `tauriTweakApi: TweakApi`.
  - `controller.ts`: `type Notify = (severity: Severity, message: string) => void`, `interface TDeps { api; store; notify }`, `createTDeps(api, store, notify)`, `friendlyT(e)`, `noticeText(code)`, `load(d)`, `toggle(d, id)`, `preset(d, level)`, `setAllUsers(d, on)`, `dismissResult(d)`, `requestApply(d)`, `acceptConfirm(d)`, `cancelConfirm(d)`, `continueAfterRestoreFailure(d)`, `abortAfterRestoreFailure(d)`, `revertSelected(d)`, `reinstall(d, id)`, `restartExplorer(d)`.
  - Luồng áp dụng: `REQUEST_APPLY` ⇒ có mục caution / «mọi tài khoản» ⇒ `confirm` ⇒ `CONFIRM_ACCEPTED`; hoặc thẳng ⇒ `restorePoint` (restore = null, đang tạo) ⇒ `RESTORE_RESULT` (failed ⇒ đứng chờ `RESTORE_CONTINUE`/`RESTORE_ABORT`) ⇒ `RUN_STARTED` ⇒ `RUN_EVENT`… ⇒ `RUN_DONE` ⇒ `done` ⇒ đọc lại (phủ mờ, `reloading`). Hoàn tác đi thẳng `ready` ⇒ `running` (không điểm khôi phục, không hộp xác nhận).

- [ ] **Step 1: Viết test hỏng**

`src/features/tinh-chinh/reducer.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { initialTState, reducer, type TAction, type TState } from './reducer';
import { readResult, report, sample } from './testdata';

const run = (actions: TAction[], from: TState = initialTState) => actions.reduce(reducer, from);
const loaded = () => run([{ type: 'LOAD_STARTED' }, { type: 'LOADED', data: readResult() }]);

describe('đọc trạng thái', () => {
  it('lần đầu tích sẵn mức Cơ bản', () => {
    const s = loaded();
    expect(s.phase).toBe('ready');
    expect(s.selected).toEqual(['ads_id', 'app_clipchamp']);
  });

  it('đọc lại giữ lựa chọn người dùng, bỏ mục không còn chọn được, phủ mờ chứ không xoá dữ liệu', () => {
    let s = run([{ type: 'TOGGLE', id: 'svc_diagtrack' }], loaded());
    s = reducer(s, { type: 'LOAD_STARTED' });
    expect(s.reloading).toBe(true);
    expect(s.data).not.toBeNull();
    const again = sample().map((t) => (t.id === 'svc_diagtrack' ? { ...t, status: 'managed' as const } : t));
    s = reducer(s, { type: 'LOADED', data: readResult(again) });
    expect(s.reloading).toBe(false);
    expect(s.selected).toEqual(['ads_id', 'app_clipchamp']);
  });

  it('đọc hỏng ⇒ dọn số cũ và giữ thông điệp', () => {
    const s = reducer(loaded(), { type: 'LOAD_FAILED', message: 'Access is denied.' });
    expect(s.data).toBeNull();
    expect(s.loadError).toBe('Access is denied.');
  });
});

describe('chọn mục', () => {
  it('không tích được mục không hỗ trợ', () => {
    const s = loaded();
    expect(reducer(s, { type: 'TOGGLE', id: 'recall_off' })).toBe(s);
  });

  it('bấm mức sẵn thay toàn bộ lựa chọn', () => {
    const s = run([{ type: 'TOGGLE', id: 'ads_id' }, { type: 'PRESET', level: 'recommended' }], loaded());
    expect(s.selected).toEqual(['ads_id', 'app_clipchamp', 'svc_diagtrack', 'app_weather']);
  });
});

describe('luồng áp dụng', () => {
  it('chỉ mục an toàn ⇒ thẳng tới điểm khôi phục, rồi chạy', () => {
    let s = reducer(loaded(), { type: 'REQUEST_APPLY' });
    expect(s.phase).toBe('restorePoint');
    expect(s.restore).toBeNull();
    s = run([{ type: 'RESTORE_RESULT', status: { status: 'created' } }, { type: 'RUN_STARTED', kind: 'apply', ids: ['ads_id'] }], s);
    expect(s.phase).toBe('running');
    s = run(
      [
        { type: 'RUN_EVENT', event: { kind: 'started', id: 'ads_id', index: 0, total: 1 } },
        { type: 'RUN_EVENT', event: { kind: 'finished', outcome: { id: 'ads_id', status: 'applied', errors: [], store_opened: [] } } },
        { type: 'RUN_DONE', report: report({ restart: 'explorer' }) },
      ],
      s,
    );
    expect(s.phase).toBe('done');
    expect(s.run?.finished).toEqual(['ads_id']);
    expect(s.report?.restart).toBe('explorer');
    expect(reducer(s, { type: 'DISMISS_RESULT' }).phase).toBe('ready');
  });

  it('có mục caution ⇒ hỏi trước; huỷ thì quay lại', () => {
    const s = run([{ type: 'PRESET', level: 'recommended' }, { type: 'REQUEST_APPLY' }], loaded());
    expect(s.phase).toBe('confirm');
    expect(reducer(s, { type: 'CONFIRM_CANCELLED' }).phase).toBe('ready');
    expect(reducer(s, { type: 'CONFIRM_ACCEPTED' }).phase).toBe('restorePoint');
  });

  it('bật «mọi tài khoản» ⇒ hỏi trước cả khi toàn mục an toàn', () => {
    const s = run([{ type: 'SET_ALL_USERS', on: true }, { type: 'TOGGLE', id: 'app_weather' }, { type: 'REQUEST_APPLY' }], loaded());
    expect(s.phase).toBe('confirm');
  });

  it('không tạo được điểm khôi phục ⇒ chờ người dùng chọn tiếp hay dừng', () => {
    let s = run([{ type: 'REQUEST_APPLY' }, { type: 'RESTORE_RESULT', status: { status: 'failed', message: 'System Protection is off' } }], loaded());
    expect(s.phase).toBe('restorePoint');
    expect(reducer(s, { type: 'RUN_STARTED', kind: 'apply', ids: ['ads_id'] })).toBe(s);
    expect(reducer(s, { type: 'RESTORE_ABORT' }).phase).toBe('ready');
    s = run([{ type: 'RESTORE_CONTINUE' }, { type: 'RUN_STARTED', kind: 'apply', ids: ['ads_id'] }], s);
    expect(s.phase).toBe('running');
  });

  it('không có gì để đổi ⇒ nút Áp dụng không làm gì', () => {
    const s = run([{ type: 'TOGGLE', id: 'ads_id' }], loaded()); // còn app_clipchamp — đã gỡ
    expect(reducer(s, { type: 'REQUEST_APPLY' })).toBe(s);
  });

  it('đang chạy thì mọi thao tác chọn bị bỏ qua', () => {
    const s = run([{ type: 'RUN_STARTED', kind: 'revert', ids: ['app_clipchamp'] }], loaded());
    expect(s.phase).toBe('running');
    expect(reducer(s, { type: 'TOGGLE', id: 'ads_id' })).toBe(s);
    expect(reducer(s, { type: 'PRESET', level: 'basic' })).toBe(s);
    expect(reducer(s, { type: 'REQUEST_APPLY' })).toBe(s);
    expect(reducer(s, { type: 'RUN_FAILED' }).phase).toBe('ready');
  });
});
```

`src/features/tinh-chinh/api.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invoke, listen, unlisten } = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn(), unlisten: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

import { tauriTweakApi } from './api';

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  unlisten.mockReset();
});

describe('tauriTweakApi', () => {
  it('apply gửi allUsers, chuyển tiếp tweak-progress và luôn gỡ lắng nghe kể cả khi lỗi', async () => {
    let handler: (e: { payload: unknown }) => void = () => {};
    listen.mockImplementation(async (_n: string, h: typeof handler) => {
      handler = h;
      return unlisten;
    });
    invoke.mockImplementation(async () => {
      handler({ payload: { kind: 'started', id: 'ads_id', index: 0, total: 1 } });
      throw 'busy';
    });
    const events: unknown[] = [];
    await expect(tauriTweakApi.apply(['ads_id'], true, (e) => events.push(e))).rejects.toBe('busy');
    expect(listen).toHaveBeenCalledWith('tweak-progress', expect.any(Function));
    expect(invoke).toHaveBeenCalledWith('tweaks_apply', { ids: ['ads_id'], allUsers: true });
    expect(events).toEqual([{ kind: 'started', id: 'ads_id', index: 0, total: 1 }]);
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('revert và các lệnh đơn giản', async () => {
    listen.mockResolvedValue(unlisten);
    invoke.mockResolvedValue({ outcomes: [], restart: 'none', notices: [] });
    await tauriTweakApi.revert(['app_clipchamp'], () => {});
    await tauriTweakApi.read();
    await tauriTweakApi.prepareRestorePoint();
    await tauriTweakApi.restartExplorer();
    expect(invoke.mock.calls).toEqual([
      ['tweaks_revert', { ids: ['app_clipchamp'] }],
      ['tweaks_read'],
      ['tweaks_prepare_restore_point'],
      ['tweaks_restart_explorer'],
    ]);
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
```

`src/features/tinh-chinh/controller.test.ts`:

```ts
import { describe, expect, it, vi } from 'vitest';
import { createStore } from '../../state/store';
import * as c from './controller';
import { initialTState, reducer } from './reducer';
import { readResult, report } from './testdata';
import type { RunReport, TweakApi } from './types';

function fakeApi(over: Partial<TweakApi> = {}): TweakApi {
  return {
    read: vi.fn(async () => readResult()),
    prepareRestorePoint: vi.fn(async () => ({ status: 'created' as const })),
    apply: vi.fn(async (ids: string[], _all: boolean, onEvent) => {
      ids.forEach((id, index) => {
        onEvent({ kind: 'started', id, index, total: ids.length });
        onEvent({ kind: 'finished', outcome: { id, status: 'applied', errors: [], store_opened: [] } });
      });
      return report({ outcomes: ids.map((id) => ({ id, status: 'applied' as const, errors: [], store_opened: [] })) });
    }),
    revert: vi.fn(async () => report()),
    restartExplorer: vi.fn(async () => {}),
    ...over,
  };
}

function setup(api = fakeApi()) {
  const store = createStore(reducer, initialTState);
  const notify = vi.fn();
  return { d: c.createTDeps(api, store, notify), store, notify, api };
}

describe('bộ điều khiển Tinh chỉnh', () => {
  it('áp dụng: điểm khôi phục ⇒ chỉ gửi mục chưa ở trạng thái đích ⇒ đọc lại', async () => {
    const { d, store, api } = setup();
    await c.load(d);
    await c.requestApply(d);
    expect(api.prepareRestorePoint).toHaveBeenCalledTimes(1);
    expect(api.apply).toHaveBeenCalledWith(['ads_id'], false, expect.any(Function));
    expect(store.getState().phase).toBe('done');
    expect(store.getState().run?.finished).toEqual(['ads_id']);
    expect(api.read).toHaveBeenCalledTimes(2);
  });

  it('điểm khôi phục hỏng ⇒ không áp dụng cho tới khi người dùng chọn tiếp', async () => {
    const { d, store, api } = setup(fakeApi({ prepareRestorePoint: vi.fn(async () => ({ status: 'failed' as const, message: 'System Protection is off' })) }));
    await c.load(d);
    await c.requestApply(d);
    expect(api.apply).not.toHaveBeenCalled();
    expect(store.getState().restore).toEqual({ status: 'failed', message: 'System Protection is off' });
    await c.continueAfterRestoreFailure(d);
    expect(api.apply).toHaveBeenCalledTimes(1);
  });

  it('lệnh điểm khôi phục ném lỗi ⇒ coi như hỏng, giữ nguyên văn', async () => {
    const { d, store } = setup(fakeApi({ prepareRestorePoint: vi.fn(async () => Promise.reject('rpc down')) }));
    await c.load(d);
    await c.requestApply(d);
    expect(store.getState().restore).toEqual({ status: 'failed', message: 'rpc down' });
  });

  it('đọc hỏng ⇒ băng đỏ nguyên văn', async () => {
    const { d, notify } = setup(fakeApi({ read: vi.fn(async () => Promise.reject(new Error('Access is denied.'))) }));
    await c.load(d);
    expect(notify).toHaveBeenCalledWith('error', 'Không đọc được trạng thái máy: Access is denied.');
  });

  it('file hoàn tác hỏng ⇒ băng hổ phách', async () => {
    const { d, notify } = setup(fakeApi({ read: vi.fn(async () => readResult(undefined, { notices: ['undo_corrupt:x.json: EOF'] })) }));
    await c.load(d);
    expect(notify).toHaveBeenCalledWith('warning', expect.stringContaining('dùng giá trị mặc định của Windows'));
    expect(notify).toHaveBeenCalledWith('warning', expect.stringContaining('x.json: EOF'));
  });

  it('áp dụng ném lỗi ⇒ băng đỏ, về danh sách, vẫn đọc lại trạng thái thật', async () => {
    const { d, store, notify, api } = setup(fakeApi({ apply: vi.fn(async () => Promise.reject('busy')) }));
    await c.load(d);
    await c.requestApply(d);
    expect(notify).toHaveBeenCalledWith('error', expect.stringContaining('Đang có thao tác khác'));
    expect(store.getState().phase).toBe('ready');
    expect(api.read).toHaveBeenCalledTimes(2);
  });

  it('hoàn tác chỉ gửi mục có gì để trả; «Cài lại từ Store» hoàn tác đúng một app', async () => {
    const { d, api } = setup();
    await c.load(d);
    await c.revertSelected(d);
    expect(api.revert).toHaveBeenCalledWith(['app_clipchamp'], expect.any(Function));
    c.dismissResult(d);
    await c.reinstall(d, 'app_clipchamp');
    expect(api.revert).toHaveBeenLastCalledWith(['app_clipchamp'], expect.any(Function));
  });

  it('không chạy chồng: đang chạy thì bấm tiếp không gọi lệnh', async () => {
    let release: () => void = () => {};
    const api = fakeApi({ revert: vi.fn(() => new Promise<RunReport>((r) => (release = () => r(report())))) });
    const { d } = setup(api);
    await c.load(d);
    const first = c.reinstall(d, 'app_clipchamp');
    await c.reinstall(d, 'app_clipchamp');
    expect(api.revert).toHaveBeenCalledTimes(1);
    release();
    await first;
  });

  it('khởi động lại Explorer hỏng ⇒ băng đỏ nguyên văn', async () => {
    const { d, notify } = setup(fakeApi({ restartExplorer: vi.fn(async () => Promise.reject('taskkill: Access is denied.')) }));
    await c.restartExplorer(d);
    expect(notify).toHaveBeenCalledWith('error', 'Không khởi động lại được Explorer: taskkill: Access is denied.');
  });
});
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `npx vitest run src/features/tinh-chinh/reducer.test.ts src/features/tinh-chinh/api.test.ts src/features/tinh-chinh/controller.test.ts`
Expected: FAIL — `Failed to resolve import "./reducer"`, `"./api"`, `"./controller"`.

- [ ] **Step 3: Viết mã**

`src/features/tinh-chinh/reducer.ts`:

```ts
import type { RestorePointStatus } from '../../api/types';
import { needsConfirm, pendingChanges, presetSelection, selectable } from './presets';
import type { ReadResult, RunReport, TweakEvent, TweakLevel } from './types';

/**
 * loading      — lần đọc đầu (chưa có dữ liệu)
 * ready        — danh sách, chọn mục
 * confirm      — hộp xác nhận mục caution / «mọi tài khoản»
 * restorePoint — đang tạo điểm khôi phục (restore = null) hoặc hỏng chờ chọn tiếp/dừng
 * running      — đang áp dụng/hoàn tác (khoá mọi nút)
 * done         — kết quả từng mục + nút khởi động lại Explorer / nhắc khởi động lại
 */
export type Phase = 'loading' | 'ready' | 'confirm' | 'restorePoint' | 'running' | 'done';
export type RunKind = 'apply' | 'revert';

export interface RunState {
  kind: RunKind;
  ids: string[];
  current: string | null;
  finished: string[];
}

export interface TState {
  phase: Phase;
  data: ReadResult | null;
  /** Đang đọc lại sau khi đã có dữ liệu — phủ mờ, không xoá trắng. */
  reloading: boolean;
  loadError: string | null;
  selected: string[];
  /** Người dùng đã tự tích/bỏ — đọc lại không tự đổi lựa chọn nữa. */
  touched: boolean;
  allUsers: boolean;
  restore: RestorePointStatus | null;
  run: RunState | null;
  report: RunReport | null;
}

export const initialTState: TState = {
  phase: 'loading',
  data: null,
  reloading: false,
  loadError: null,
  selected: [],
  touched: false,
  allUsers: false,
  restore: null,
  run: null,
  report: null,
};

export type TAction =
  | { type: 'LOAD_STARTED' }
  | { type: 'LOADED'; data: ReadResult }
  | { type: 'LOAD_FAILED'; message: string }
  | { type: 'TOGGLE'; id: string }
  | { type: 'PRESET'; level: TweakLevel }
  | { type: 'SET_ALL_USERS'; on: boolean }
  | { type: 'REQUEST_APPLY' }
  | { type: 'CONFIRM_ACCEPTED' }
  | { type: 'CONFIRM_CANCELLED' }
  | { type: 'RESTORE_RESULT'; status: RestorePointStatus }
  | { type: 'RESTORE_CONTINUE' }
  | { type: 'RESTORE_ABORT' }
  | { type: 'RUN_STARTED'; kind: RunKind; ids: string[] }
  | { type: 'RUN_EVENT'; event: TweakEvent }
  | { type: 'RUN_DONE'; report: RunReport }
  | { type: 'RUN_FAILED' }
  | { type: 'DISMISS_RESULT' };

/** Mức mặc định khi mở tab: Cơ bản (toàn mục an toàn). */
export const DEFAULT_LEVEL: TweakLevel = 'basic';

function startRun(s: TState, kind: RunKind, ids: string[]): TState {
  return { ...s, phase: 'running', restore: null, run: { kind, ids, current: null, finished: [] }, report: null };
}

export function reducer(s: TState, a: TAction): TState {
  switch (a.type) {
    case 'LOAD_STARTED':
      return s.data ? { ...s, reloading: true, loadError: null } : { ...s, phase: 'loading', loadError: null };
    case 'LOADED': {
      const tweaks = a.data.tweaks;
      const allowed = new Set(tweaks.filter(selectable).map((t) => t.id));
      const selected = s.touched ? s.selected.filter((id) => allowed.has(id)) : presetSelection(tweaks, DEFAULT_LEVEL);
      const phase = s.phase === 'loading' ? 'ready' : s.phase;
      return { ...s, phase, data: a.data, reloading: false, loadError: null, selected };
    }
    case 'LOAD_FAILED':
      // Lỗi đọc làm mất sạch số liệu ⇒ dọn số cũ (luật báo lỗi: không để người đọc tin số cũ).
      return { ...s, phase: s.phase === 'loading' || s.phase === 'ready' ? 'loading' : s.phase, data: null, reloading: false, loadError: a.message };
    case 'TOGGLE': {
      if (s.phase !== 'ready' || !s.data) return s;
      const t = s.data.tweaks.find((x) => x.id === a.id);
      if (!t || !selectable(t)) return s;
      const selected = s.selected.includes(a.id) ? s.selected.filter((x) => x !== a.id) : [...s.selected, a.id];
      return { ...s, selected, touched: true };
    }
    case 'PRESET':
      if (s.phase !== 'ready' || !s.data) return s;
      return { ...s, selected: presetSelection(s.data.tweaks, a.level), touched: true };
    case 'SET_ALL_USERS':
      if (s.phase !== 'ready') return s;
      return { ...s, allUsers: a.on };
    case 'REQUEST_APPLY': {
      if (s.phase !== 'ready' || !s.data || s.reloading) return s;
      const ids = pendingChanges(s.data.tweaks, s.selected);
      if (ids.length === 0) return s;
      return needsConfirm(s.data.tweaks, ids, s.allUsers).length > 0 ? { ...s, phase: 'confirm' } : { ...s, phase: 'restorePoint', restore: null };
    }
    case 'CONFIRM_ACCEPTED':
      return s.phase === 'confirm' ? { ...s, phase: 'restorePoint', restore: null } : s;
    case 'CONFIRM_CANCELLED':
      return s.phase === 'confirm' ? { ...s, phase: 'ready' } : s;
    case 'RESTORE_RESULT':
      // failed ⇒ đứng lại chờ RESTORE_CONTINUE / RESTORE_ABORT; khác ⇒ bộ điều khiển gửi RUN_STARTED.
      return s.phase === 'restorePoint' && s.restore === null ? { ...s, restore: a.status } : s;
    case 'RESTORE_CONTINUE':
      return s.phase === 'restorePoint' && s.restore?.status === 'failed' ? { ...s, restore: { status: 'skipped' } } : s;
    case 'RESTORE_ABORT':
      return s.phase === 'restorePoint' && s.restore?.status === 'failed' ? { ...s, phase: 'ready', restore: null } : s;
    case 'RUN_STARTED':
      if (a.kind === 'apply' && !(s.phase === 'restorePoint' && s.restore !== null && s.restore.status !== 'failed')) return s;
      if (a.kind === 'revert' && s.phase !== 'ready') return s;
      return startRun(s, a.kind, a.ids);
    case 'RUN_EVENT': {
      if (s.phase !== 'running' || !s.run) return s;
      const e = a.event;
      if (e.kind === 'started') return { ...s, run: { ...s.run, current: e.id } };
      return { ...s, run: { ...s.run, current: null, finished: [...s.run.finished, e.outcome.id] } };
    }
    case 'RUN_DONE':
      return s.phase === 'running' ? { ...s, phase: 'done', report: a.report, run: s.run ? { ...s.run, current: null } : null } : s;
    case 'RUN_FAILED':
      return s.phase === 'running' ? { ...s, phase: 'ready', run: null } : s;
    case 'DISMISS_RESULT':
      return s.phase === 'done' ? { ...s, phase: 'ready', run: null, report: null } : s;
  }
}
```

`src/features/tinh-chinh/api.ts`:

```ts
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { RestorePointStatus } from '../../api/types';
import type { ReadResult, RunReport, TweakApi, TweakEvent } from './types';

// Lệnh do Task Tích hợp đăng ký trong src-tauri. Tauri đổi camelCase ⇒ snake_case (allUsers → all_users).
async function withProgress(run: () => Promise<RunReport>, onEvent: (e: TweakEvent) => void): Promise<RunReport> {
  const unlisten = await listen<TweakEvent>('tweak-progress', (e) => onEvent(e.payload));
  try {
    return await run();
  } finally {
    unlisten();
  }
}

export const tauriTweakApi: TweakApi = {
  read: () => invoke<ReadResult>('tweaks_read'),
  prepareRestorePoint: () => invoke<RestorePointStatus>('tweaks_prepare_restore_point'),
  apply: (ids, allUsers, onEvent) => withProgress(() => invoke<RunReport>('tweaks_apply', { ids, allUsers }), onEvent),
  revert: (ids, onEvent) => withProgress(() => invoke<RunReport>('tweaks_revert', { ids }), onEvent),
  restartExplorer: () => invoke<void>('tweaks_restart_explorer'),
};
```

`src/features/tinh-chinh/controller.ts`:

```ts
import type { RestorePointStatus } from '../../api/types';
import { messageOf, type Severity } from '../../errors/errors';
import type { Store } from '../../state/store';
import { pendingChanges, revertable } from './presets';
import type { RunKind, TAction, TState } from './reducer';
import { tt } from './strings';
import type { TweakApi, TweakEvent, TweakLevel } from './types';

export type Notify = (severity: Severity, message: string) => void;

export interface TDeps {
  api: TweakApi;
  store: Store<TState, TAction>;
  notify: Notify;
}

export function createTDeps(api: TweakApi, store: Store<TState, TAction>, notify: Notify): TDeps {
  return { api, store, notify };
}

/** Lỗi của lệnh Tauri là chuỗi; hai mã riêng được dịch, còn lại giữ nguyên văn. */
export function friendlyT(e: unknown): string {
  const m = messageOf(e);
  if (m === 'busy') return tt('tweaks.errors.busy');
  if (m === 'dry_run') return tt('tweaks.errors.dryRun');
  return m;
}

export function noticeText(code: string): string {
  const i = code.indexOf(':');
  const [kind, detail] = i < 0 ? [code, ''] : [code.slice(0, i), code.slice(i + 1)];
  return kind === 'undo_corrupt' ? tt('tweaks.notice.undoCorrupt', { detail }) : code;
}

export async function load(d: TDeps): Promise<void> {
  d.store.dispatch({ type: 'LOAD_STARTED' });
  try {
    const data = await d.api.read();
    d.store.dispatch({ type: 'LOADED', data });
    data.notices.forEach((n) => d.notify('warning', noticeText(n)));
  } catch (e) {
    const message = friendlyT(e);
    d.store.dispatch({ type: 'LOAD_FAILED', message });
    d.notify('error', tt('tweaks.errors.loadFailed', { message }));
  }
}

export function toggle(d: TDeps, id: string): void {
  d.store.dispatch({ type: 'TOGGLE', id });
}

export function preset(d: TDeps, level: TweakLevel): void {
  d.store.dispatch({ type: 'PRESET', level });
}

export function setAllUsers(d: TDeps, on: boolean): void {
  d.store.dispatch({ type: 'SET_ALL_USERS', on });
}

export function dismissResult(d: TDeps): void {
  d.store.dispatch({ type: 'DISMISS_RESULT' });
}

async function execute(d: TDeps, kind: RunKind, ids: string[]): Promise<void> {
  const before = d.store.getState();
  d.store.dispatch({ type: 'RUN_STARTED', kind, ids });
  // Reducer từ chối (đang chạy lượt khác, hoặc chưa qua điểm khôi phục) ⇒ trạng thái giữ nguyên tham chiếu ⇒ không gọi lệnh.
  if (d.store.getState() === before) return;
  const allUsers = d.store.getState().allUsers;
  const onEvent = (event: TweakEvent) => d.store.dispatch({ type: 'RUN_EVENT', event });
  try {
    const report = kind === 'apply' ? await d.api.apply(ids, allUsers, onEvent) : await d.api.revert(ids, onEvent);
    d.store.dispatch({ type: 'RUN_DONE', report });
    report.notices.forEach((n) => d.notify('warning', noticeText(n)));
  } catch (e) {
    d.store.dispatch({ type: 'RUN_FAILED' });
    d.notify('error', tt(kind === 'apply' ? 'tweaks.errors.applyFailed' : 'tweaks.errors.revertFailed', { message: friendlyT(e) }));
  }
  await load(d);
}

async function createRestorePoint(d: TDeps): Promise<void> {
  let status: RestorePointStatus;
  try {
    status = await d.api.prepareRestorePoint();
  } catch (e) {
    status = { status: 'failed', message: friendlyT(e) };
  }
  d.store.dispatch({ type: 'RESTORE_RESULT', status });
  if (status.status !== 'failed') await runApply(d);
}

async function runApply(d: TDeps): Promise<void> {
  const s = d.store.getState();
  if (!s.data) return;
  await execute(d, 'apply', pendingChanges(s.data.tweaks, s.selected));
}

export async function requestApply(d: TDeps): Promise<void> {
  d.store.dispatch({ type: 'REQUEST_APPLY' });
  if (d.store.getState().phase === 'restorePoint') await createRestorePoint(d);
}

export async function acceptConfirm(d: TDeps): Promise<void> {
  d.store.dispatch({ type: 'CONFIRM_ACCEPTED' });
  if (d.store.getState().phase === 'restorePoint') await createRestorePoint(d);
}

export function cancelConfirm(d: TDeps): void {
  d.store.dispatch({ type: 'CONFIRM_CANCELLED' });
}

export async function continueAfterRestoreFailure(d: TDeps): Promise<void> {
  d.store.dispatch({ type: 'RESTORE_CONTINUE' });
  await runApply(d);
}

export function abortAfterRestoreFailure(d: TDeps): void {
  d.store.dispatch({ type: 'RESTORE_ABORT' });
}

export async function revertSelected(d: TDeps): Promise<void> {
  const s = d.store.getState();
  if (!s.data || s.phase !== 'ready') return;
  const ids = revertable(s.data.tweaks, s.selected);
  if (ids.length > 0) await execute(d, 'revert', ids);
}

/** Nút «Cài lại từ Store» của một app đã gỡ = hoàn tác riêng mục đó. */
export async function reinstall(d: TDeps, id: string): Promise<void> {
  await execute(d, 'revert', [id]);
}

export async function restartExplorer(d: TDeps): Promise<void> {
  try {
    await d.api.restartExplorer();
  } catch (e) {
    d.notify('error', tt('tweaks.errors.explorerFailed', { message: friendlyT(e) }));
  }
}
```

- [ ] **Step 4: Chạy test và typecheck, thấy qua**

Run: `npx vitest run src/features/tinh-chinh/reducer.test.ts src/features/tinh-chinh/api.test.ts src/features/tinh-chinh/controller.test.ts` rồi `npm run typecheck`
Expected: `Tests  22 passed (22)` (11 reducer + 2 api + 9 controller); typecheck không lỗi.

- [ ] **Step 5: Commit**

```bash
rtk git add src/features/tinh-chinh/reducer.ts
rtk git add src/features/tinh-chinh/reducer.test.ts
rtk git add src/features/tinh-chinh/api.ts
rtk git add src/features/tinh-chinh/api.test.ts
rtk git add src/features/tinh-chinh/controller.ts
rtk git add src/features/tinh-chinh/controller.test.ts
rtk git commit -m "feat(ui-tinh-chinh): máy trạng thái tab, cầu nối tweaks_*, bộ điều khiển — điểm khôi phục, không chạy chồng

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Giao diện — danh sách, hộp xác nhận, tiến độ/kết quả, màn tab

**Files:**
- Create: `src/features/tinh-chinh/TweakList.tsx`
- Create: `src/features/tinh-chinh/Dialogs.tsx`
- Create: `src/features/tinh-chinh/RunPanel.tsx`
- Create: `src/features/tinh-chinh/TinhChinhView.tsx`
- Create: `src/features/tinh-chinh/tinh-chinh.css`
- Test: `src/features/tinh-chinh/views.test.tsx`

**Interfaces:**
- Consumes:
  - Task 9: `itemName`, `itemDesc`, `levelText`, `statusText`, `outcomeText` (`labels.ts`); `LEVELS`, `currentPreset`, `pendingChanges`, `revertable`, `needsConfirm`, `selectable`, `visible` (`presets.ts`); `tt`; `TweakApi`, `TweakView`, `TweakGroup`; `readResult`, `report`, `sample` (`testdata.ts`).
  - Task 10: `TState`, `initialTState`, `reducer`; `c.createTDeps`, `c.load`, `c.toggle`, `c.preset`, `c.setAllUsers`, `c.requestApply`, `c.acceptConfirm`, `c.cancelConfirm`, `c.continueAfterRestoreFailure`, `c.abortAfterRestoreFailure`, `c.revertSelected`, `c.reinstall`, `c.restartExplorer`, `c.dismissResult`, `c.friendlyT`, `type Notify`.
  - v0.1 Task 12: `Busy({ label?, size? })` (`src/components/Busy.tsx`); lớp CSS `wfu-muted`, `wfu-actions`, `wfu-canhbao` (`src/styles.css`, quy tắc `prefers-reduced-motion` cho `.wfu-busy` đã có). v0.1 Task 13: `createStore`.
- Produces (App ở Task 12 dùng nguyên văn): `TinhChinhView({ api: TweakApi; notify: Notify; dryRun: boolean; onBusyChange?: (busy: boolean) => void })` — tự tạo store riêng, đọc trạng thái khi dựng; `onBusyChange(true)` khi đang xác nhận / tạo điểm khôi phục / chạy.

Chỉ báo động (luật người dùng): lần đọc đầu ⇒ `Busy` cỡ `medium`; đọc lại ⇒ lớp phủ mờ + `Busy`, giữ danh sách cũ; tạo điểm khôi phục, áp dụng/hoàn tác (kèm tên mục đang chạy), khởi động lại Explorer ⇒ `Busy`. Đang chạy ⇒ mọi ô và nút bị khoá.

- [ ] **Step 1: Viết test hỏng**

`src/features/tinh-chinh/views.test.tsx`:

```tsx
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import type { ReactNode } from 'react';
import { TinhChinhView } from './TinhChinhView';
import { readResult, report, sample } from './testdata';
import type { RunReport, TweakApi } from './types';

const wrap = (ui: ReactNode) => render(<FluentProvider theme={webLightTheme}>{ui}</FluentProvider>);

function fakeApi(over: Partial<TweakApi> = {}): TweakApi {
  return {
    read: vi.fn(async () => readResult()),
    prepareRestorePoint: vi.fn(async () => ({ status: 'created' as const })),
    apply: vi.fn(async (ids: string[]) =>
      report({ outcomes: ids.map((id) => ({ id, status: 'applied' as const, errors: [], store_opened: [] })), restart: 'explorer' }),
    ),
    revert: vi.fn(async () => report()),
    restartExplorer: vi.fn(async () => {}),
    ...over,
  };
}

describe('TinhChinhView', () => {
  it('lần đầu có vòng quay, rồi danh sách tích sẵn Cơ bản, app không có trên máy bị ẩn', async () => {
    wrap(<TinhChinhView api={fakeApi()} notify={vi.fn()} dryRun={false} />);
    expect(screen.getByRole('progressbar')).toBeTruthy();
    expect(await screen.findByText('Gỡ app (2 đang có · 1 đã gỡ)')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Cơ bản' }).getAttribute('aria-pressed')).toBe('true');
    expect(screen.queryByText('TikTok')).toBeNull();
    expect(screen.getByRole('button', { name: 'Áp dụng 1 thay đổi' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Hoàn tác đã chọn (1)' })).toBeTruthy();
  });

  it('mục không hỗ trợ hiện lý do và không tích được', async () => {
    wrap(<TinhChinhView api={fakeApi()} notify={vi.fn()} dryRun={false} />);
    const row = await screen.findByTestId('tweak-recall_off');
    expect(within(row).getByText('Không hỗ trợ trên máy này — cần Windows 11 24H2 trở lên')).toBeTruthy();
    expect((within(row).getByRole('checkbox') as HTMLInputElement).disabled).toBe(true);
  });

  it('bấm Khuyến nghị ⇒ đếm lại; tự bỏ một mục ⇒ «Tùy chỉnh»', async () => {
    wrap(<TinhChinhView api={fakeApi()} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Khuyến nghị' }));
    expect(screen.getByRole('button', { name: 'Áp dụng 3 thay đổi' })).toBeTruthy();
    fireEvent.click(within(screen.getByTestId('tweak-ads_id')).getByRole('checkbox'));
    expect(screen.getByText('Tùy chỉnh')).toBeTruthy();
  });

  it('có mục Cân nhắc ⇒ hộp xác nhận liệt kê; đồng ý ⇒ áp dụng, hiện kết quả và nút khởi động lại Explorer', async () => {
    const api = fakeApi();
    wrap(<TinhChinhView api={api} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Khuyến nghị' }));
    fireEvent.click(screen.getByRole('button', { name: 'Áp dụng 3 thay đổi' }));
    const dialog = await screen.findByRole('alertdialog');
    expect(within(dialog).getByText('Tắt dịch vụ gửi dữ liệu chẩn đoán (DiagTrack)')).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Áp dụng' }));
    expect(await screen.findByText('Kết quả')).toBeTruthy();
    expect(api.apply).toHaveBeenCalledWith(['ads_id', 'svc_diagtrack', 'app_weather'], false, expect.any(Function));
    fireEvent.click(screen.getByRole('button', { name: 'Khởi động lại Explorer' }));
    await waitFor(() => expect(api.restartExplorer).toHaveBeenCalled());
  });

  it('lỗi từng mục hiện nguyên văn trong kết quả', async () => {
    const api = fakeApi({
      apply: vi.fn(async () => report({ outcomes: [{ id: 'ads_id', status: 'not_applied', errors: ['ads_id#0: Access is denied.'], store_opened: [] }] })),
    });
    wrap(<TinhChinhView api={api} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    expect(await screen.findByText('ads_id#0: Access is denied.')).toBeTruthy();
    expect(screen.getByText('✗ Lỗi')).toBeTruthy();
  });

  it('đang áp dụng ⇒ khoá mọi nút, có vòng quay, báo App đang bận', async () => {
    let finish: () => void = () => {};
    const api = fakeApi({ apply: vi.fn(() => new Promise<RunReport>((r) => (finish = () => r(report())))) });
    const onBusyChange = vi.fn();
    wrap(<TinhChinhView api={api} notify={vi.fn()} dryRun={false} onBusyChange={onBusyChange} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    expect(await screen.findByText(/Đang áp dụng 0\/1/)).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Khuyến nghị' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: /Hoàn tác đã chọn/ }) as HTMLButtonElement).disabled).toBe(true);
    expect(onBusyChange).toHaveBeenLastCalledWith(true);
    finish();
    expect(await screen.findByText('Kết quả')).toBeTruthy();
    expect(onBusyChange).toHaveBeenLastCalledWith(false);
  });

  it('app đã gỡ có nút «Cài lại từ Store»', async () => {
    const api = fakeApi();
    wrap(<TinhChinhView api={api} notify={vi.fn()} dryRun={false} />);
    const row = await screen.findByTestId('tweak-app_clipchamp');
    fireEvent.click(within(row).getByRole('button', { name: 'Cài lại từ Store' }));
    await waitFor(() => expect(api.revert).toHaveBeenCalledWith(['app_clipchamp'], expect.any(Function)));
  });

  it('chạy thử ⇒ chỉ xem, nút Áp dụng/Hoàn tác bị khoá', async () => {
    wrap(<TinhChinhView api={fakeApi()} notify={vi.fn()} dryRun />);
    expect(await screen.findByText('Chạy thử: tab này chỉ xem trạng thái, không áp dụng hay hoàn tác.')).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Áp dụng 1 thay đổi' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('chạy bằng tài khoản admin khác ⇒ băng hổ phách cảnh báo', async () => {
    const data = readResult(sample(), { system: { build: 26200, edition: 'Pro', managed: false, other_user: true } });
    wrap(<TinhChinhView api={fakeApi({ read: vi.fn(async () => data) })} notify={vi.fn()} dryRun={false} />);
    expect(await screen.findByText(/tài khoản quản trị khác/)).toBeTruthy();
  });

  it('đọc hỏng ⇒ báo đỏ qua notify và có nút Đọc lại', async () => {
    const notify = vi.fn();
    const read = vi.fn().mockRejectedValueOnce('Access is denied.').mockResolvedValue(readResult());
    wrap(<TinhChinhView api={fakeApi({ read })} notify={notify} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Đọc lại' }));
    expect(notify).toHaveBeenCalledWith('error', 'Không đọc được trạng thái máy: Access is denied.');
    expect(await screen.findByText('Quyền riêng tư & quảng cáo')).toBeTruthy();
  });
});
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `npx vitest run src/features/tinh-chinh/views.test.tsx`
Expected: FAIL — `Failed to resolve import "./TinhChinhView"`.

- [ ] **Step 3: Viết mã**

`src/features/tinh-chinh/TweakList.tsx`:

```tsx
import { Badge, Button, Checkbox } from '@fluentui/react-components';
import { itemDesc, itemName, levelText, statusText } from './labels';
import { selectable, visible } from './presets';
import { tt } from './strings';
import type { TweakGroup, TweakView } from './types';

interface ListProps {
  tweaks: TweakView[];
  selected: string[];
  /** Đang chạy/đọc lại ⇒ khoá mọi ô và nút. */
  locked: boolean;
  onToggle: (id: string) => void;
  onReinstall: (id: string) => void;
}

function statusColor(t: TweakView): 'success' | 'warning' | 'informative' | 'subtle' {
  if (t.status === 'applied') return 'success';
  if (t.status === 'partial') return 'warning';
  if (t.status === 'managed' || t.status === 'unsupported') return 'subtle';
  return 'informative';
}

function Row({ t, checked, locked, onToggle, onReinstall }: { t: TweakView; checked: boolean; locked: boolean; onToggle: () => void; onReinstall: () => void }) {
  const canPick = selectable(t);
  return (
    <li className={canPick ? 'tc-row' : 'tc-row tc-row-off'} data-testid={`tweak-${t.id}`}>
      <Checkbox checked={checked} disabled={!canPick || locked} onChange={onToggle} aria-label={itemName(t.id)} />
      <div className="tc-row-main">
        <div className="tc-row-title">
          <span>{itemName(t.id)}</span>
          {t.risk === 'caution' && (
            <Badge appearance="tint" color="warning" size="small">
              ⚠ {tt('tweaks.caution')}
            </Badge>
          )}
        </div>
        <div className="wfu-muted">{itemDesc(t.id)}</div>
        {t.errors.map((e) => (
          <div key={e} className="tc-row-error">
            {tt('tweaks.readError', { message: e })}
          </div>
        ))}
      </div>
      <span className="tc-level">{levelText(t.level)}</span>
      <Badge appearance="outline" color={statusColor(t)} className="tc-status">
        {statusText(t)}
      </Badge>
      {t.group === 'bloatware' && t.status === 'applied' && t.has_undo ? (
        <Button size="small" disabled={locked} onClick={onReinstall}>
          {tt('tweaks.reinstall')}
        </Button>
      ) : (
        <span />
      )}
    </li>
  );
}

function Section({ group, title, ...p }: ListProps & { group: TweakGroup; title: string }) {
  const rows = p.tweaks.filter((t) => t.group === group && visible(t));
  if (rows.length === 0) return null;
  const chosen = new Set(p.selected);
  return (
    <section className="tc-section">
      <h3 className="tc-section-title">{title}</h3>
      <ul className="tc-rows">
        {rows.map((t) => (
          <Row key={t.id} t={t} checked={chosen.has(t.id)} locked={p.locked} onToggle={() => p.onToggle(t.id)} onReinstall={() => p.onReinstall(t.id)} />
        ))}
      </ul>
    </section>
  );
}

export function TweakList(p: ListProps) {
  const apps = p.tweaks.filter((t) => t.group === 'bloatware' && visible(t));
  const removed = apps.filter((t) => t.status === 'applied').length;
  const present = apps.filter((t) => t.status === 'not_applied' || t.status === 'partial').length;
  return (
    <>
      <Section {...p} group="bloatware" title={tt('tweaks.group.bloatware', { present, removed })} />
      <Section {...p} group="privacy" title={tt('tweaks.group.privacy')} />
    </>
  );
}
```

`src/features/tinh-chinh/Dialogs.tsx`:

```tsx
import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  MessageBar,
  MessageBarActions,
  MessageBarBody,
  MessageBarTitle,
} from '@fluentui/react-components';
import { Busy } from '../../components/Busy';
import { itemName } from './labels';
import { needsConfirm, pendingChanges } from './presets';
import type { TState } from './reducer';
import { tt } from './strings';

/** Chỉ vẽ khi `phase === 'confirm'`. Liệt kê mục caution, và nếu bật «mọi tài khoản» thì cả các app sẽ gỡ. */
export function ConfirmTweaks({ state, onAccept, onCancel }: { state: TState; onAccept: () => void; onCancel: () => void }) {
  const tweaks = state.data?.tweaks ?? [];
  const list = needsConfirm(tweaks, pendingChanges(tweaks, state.selected), state.allUsers);
  const caution = list.filter((t) => t.risk === 'caution');
  const apps = state.allUsers ? list.filter((t) => t.group === 'bloatware') : [];
  return (
    <Dialog open modalType="alert" onOpenChange={(_, d) => !d.open && onCancel()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>{tt('tweaks.confirm.title')}</DialogTitle>
          <DialogContent>
            {caution.length > 0 && (
              <>
                <p>{tt('tweaks.confirm.caution')}</p>
                <ul>
                  {caution.map((t) => (
                    <li key={t.id}>{itemName(t.id)}</li>
                  ))}
                </ul>
              </>
            )}
            {apps.length > 0 && (
              <>
                <p className="tc-warn">⚠ {tt('tweaks.confirm.allUsers')}</p>
                <ul>
                  {apps.map((t) => (
                    <li key={t.id}>{itemName(t.id)}</li>
                  ))}
                </ul>
              </>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onCancel}>
              {tt('tweaks.confirm.cancel')}
            </Button>
            <Button appearance="primary" onClick={onAccept}>
              {tt('tweaks.confirm.accept')}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

/** Đang tạo điểm khôi phục ⇒ vòng quay; hỏng ⇒ băng hổ phách, người dùng chọn tiếp/dừng. */
export function RestorePrompt({ state, onContinue, onAbort }: { state: TState; onContinue: () => void; onAbort: () => void }) {
  if (state.phase !== 'restorePoint') return null;
  if (state.restore?.status !== 'failed') return <Busy label={tt('tweaks.restore.creating')} size="small" />;
  return (
    <MessageBar intent="warning" className="wfu-canhbao">
      <MessageBarBody>
        <MessageBarTitle>{tt('tweaks.restore.failedTitle')}</MessageBarTitle>
        {tt('tweaks.restore.failedBody', { message: state.restore.message })}
      </MessageBarBody>
      <MessageBarActions>
        <Button onClick={onAbort}>{tt('tweaks.restore.abort')}</Button>
        <Button appearance="primary" onClick={onContinue}>
          {tt('tweaks.restore.continue')}
        </Button>
      </MessageBarActions>
    </MessageBar>
  );
}
```

`src/features/tinh-chinh/RunPanel.tsx`:

```tsx
import { useState } from 'react';
import { Button, MessageBar, MessageBarBody } from '@fluentui/react-components';
import { Busy } from '../../components/Busy';
import { itemName, outcomeText } from './labels';
import type { TState } from './reducer';
import { tt } from './strings';

/** Tiến độ khi đang chạy; kết quả từng mục (lỗi nguyên văn) và việc cần khởi động lại khi xong. */
export function RunPanel({ state, onRestartExplorer, onClose }: { state: TState; onRestartExplorer: () => Promise<void>; onClose: () => void }) {
  const [restarting, setRestarting] = useState(false);
  const run = state.run;
  if (!run) return null;
  if (state.phase === 'running') {
    const label = tt(run.kind === 'apply' ? 'tweaks.run.apply' : 'tweaks.run.revert', { done: run.finished.length, total: run.ids.length });
    return (
      <div className="tc-panel" aria-live="polite">
        <Busy label={run.current ? `${label} ${itemName(run.current)}` : label} size="small" />
      </div>
    );
  }
  const report = state.report;
  if (state.phase !== 'done' || !report) return null;
  const restartExplorer = async () => {
    setRestarting(true);
    try {
      await onRestartExplorer();
    } finally {
      setRestarting(false);
    }
  };
  return (
    <div className="tc-panel" aria-live="polite">
      <h3 className="tc-section-title">{tt('tweaks.result.title')}</h3>
      <ul className="tc-results">
        {report.outcomes.map((o) => (
          <li key={o.id}>
            <span className="tc-result-mark">{outcomeText(o, run.kind)}</span> {itemName(o.id)}
            {o.store_opened.length > 0 && <div className="wfu-muted">{tt('tweaks.result.storeOpened')}</div>}
            {o.errors.map((e) => (
              <div key={e} className="tc-row-error">
                {e}
              </div>
            ))}
          </li>
        ))}
      </ul>
      {report.restart === 'logoff' && (
        <MessageBar intent="info">
          <MessageBarBody>{tt('tweaks.result.logoff')}</MessageBarBody>
        </MessageBar>
      )}
      {report.restart === 'reboot' && (
        <MessageBar intent="info">
          <MessageBarBody>{tt('tweaks.result.reboot')}</MessageBarBody>
        </MessageBar>
      )}
      <div className="wfu-actions">
        {report.restart === 'explorer' &&
          (restarting ? (
            <Busy label={tt('tweaks.result.explorerBusy')} />
          ) : (
            <Button appearance="primary" onClick={() => void restartExplorer()}>
              {tt('tweaks.result.explorer')}
            </Button>
          ))}
        <Button disabled={restarting} onClick={onClose}>
          {tt('tweaks.result.close')}
        </Button>
      </div>
    </div>
  );
}
```

`src/features/tinh-chinh/TinhChinhView.tsx`:

```tsx
import { useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import { Button, Checkbox, MessageBar, MessageBarBody } from '@fluentui/react-components';
import { Busy } from '../../components/Busy';
import { createStore } from '../../state/store';
import * as c from './controller';
import { ConfirmTweaks, RestorePrompt } from './Dialogs';
import { currentPreset, LEVELS, pendingChanges, revertable } from './presets';
import { initialTState, reducer } from './reducer';
import { RunPanel } from './RunPanel';
import { tt } from './strings';
import { TweakList } from './TweakList';
import type { TweakApi } from './types';
import './tinh-chinh.css';

export interface TinhChinhProps {
  api: TweakApi;
  notify: c.Notify;
  /** Cờ `--dry-run` của app: chỉ xem, không cho áp dụng/hoàn tác. */
  dryRun: boolean;
  /** Báo cho App khoá tab khác khi đang xác nhận / tạo điểm khôi phục / chạy. */
  onBusyChange?: (busy: boolean) => void;
}

export function TinhChinhView({ api, notify, dryRun, onBusyChange }: TinhChinhProps) {
  const [store] = useState(() => createStore(reducer, initialTState));
  const s = useSyncExternalStore(store.subscribe, store.getState);
  const d = useMemo(() => c.createTDeps(api, store, notify), [api, store, notify]);
  useEffect(() => {
    void c.load(d);
  }, [d]);
  const working = s.phase === 'confirm' || s.phase === 'restorePoint' || s.phase === 'running';
  useEffect(() => {
    onBusyChange?.(working);
  }, [working, onBusyChange]);
  const run = (p: Promise<void>) => {
    p.catch((e) => notify('error', c.friendlyT(e)));
  };

  if (!s.data) {
    if (!s.loadError) return <Busy label={tt('tweaks.loading')} size="medium" />;
    // Nội dung lỗi đã lên băng đỏ qua notify; ở đây chỉ cho đọc lại.
    return (
      <div className="wfu-actions">
        <Button onClick={() => run(c.load(d))}>{tt('tweaks.retry')}</Button>
      </div>
    );
  }

  const tweaks = s.data.tweaks;
  const busy = s.phase !== 'ready' || s.reloading;
  const locked = busy || dryRun;
  const preset = currentPreset(tweaks, s.selected);
  const toApply = pendingChanges(tweaks, s.selected).length;
  const toRevert = revertable(tweaks, s.selected).length;

  return (
    <div className="tc-root">
      {dryRun && (
        <MessageBar intent="info">
          <MessageBarBody>{tt('tweaks.dryRun')}</MessageBarBody>
        </MessageBar>
      )}
      {s.data.system.other_user && (
        <MessageBar intent="warning" className="wfu-canhbao">
          <MessageBarBody>{tt('tweaks.otherUser')}</MessageBarBody>
        </MessageBar>
      )}
      {s.data.system.managed && (
        <MessageBar intent="info">
          <MessageBarBody>{tt('tweaks.managedMachine')}</MessageBarBody>
        </MessageBar>
      )}

      <div className="tc-presets" role="group" aria-label={tt('tweaks.preset.label')}>
        {LEVELS.map((l) => (
          <Button key={l} appearance={preset === l ? 'primary' : 'secondary'} aria-pressed={preset === l} disabled={busy} onClick={() => c.preset(d, l)}>
            {tt(`tweaks.preset.${l}`)}
          </Button>
        ))}
        {preset === 'custom' && <span className="tc-custom">{tt('tweaks.preset.custom')}</span>}
      </div>

      <RestorePrompt state={s} onContinue={() => run(c.continueAfterRestoreFailure(d))} onAbort={() => c.abortAfterRestoreFailure(d)} />
      <RunPanel state={s} onRestartExplorer={() => c.restartExplorer(d)} onClose={() => c.dismissResult(d)} />

      <div className={s.reloading ? 'tc-list tc-dim' : 'tc-list'} aria-busy={s.reloading}>
        {s.reloading && (
          <div className="tc-overlay">
            <Busy label={tt('tweaks.reloading')} size="small" />
          </div>
        )}
        <TweakList tweaks={tweaks} selected={s.selected} locked={busy} onToggle={(id) => c.toggle(d, id)} onReinstall={(id) => run(c.reinstall(d, id))} />
      </div>

      <div className="tc-footer">
        <Checkbox
          checked={s.allUsers}
          disabled={locked}
          onChange={(_, v) => c.setAllUsers(d, v.checked === true)}
          label={
            <>
              {tt('tweaks.allUsers')} <span className="tc-warn">⚠ {tt('tweaks.allUsersWarn')}</span>
            </>
          }
        />
        <div className="wfu-actions">
          <Button disabled={locked || toRevert === 0} onClick={() => run(c.revertSelected(d))}>
            {tt('tweaks.revert', { count: toRevert })}
          </Button>
          <Button appearance="primary" disabled={locked || toApply === 0} onClick={() => run(c.requestApply(d))}>
            {tt('tweaks.apply', { count: toApply })}
          </Button>
        </div>
      </div>

      {s.phase === 'confirm' && <ConfirmTweaks state={s} onAccept={() => run(c.acceptConfirm(d))} onCancel={() => c.cancelConfirm(d)} />}
    </div>
  );
}
```

`src/features/tinh-chinh/tinh-chinh.css`:

```css
/* Tab Tinh chỉnh. Dùng lại .wfu-muted, .wfu-actions, .wfu-canhbao, .wfu-busy của src/styles.css. */
.tc-root {
  display: grid;
  gap: 16px;
}

.tc-presets {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.tc-custom {
  font-weight: 600;
  padding: 0 8px;
}

.tc-section {
  display: grid;
  gap: 8px;
}

.tc-section-title {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}

.tc-rows,
.tc-results {
  display: grid;
  gap: 6px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.tc-row {
  display: grid;
  grid-template-columns: auto 1fr auto auto auto;
  align-items: center;
  gap: 4px 12px;
  padding: 8px 12px;
  border-radius: 6px;
  border: 1px solid color-mix(in srgb, currentColor 14%, transparent);
}

.tc-row-off {
  opacity: 0.55;
}

.tc-row-main {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.tc-row-title {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  font-weight: 600;
}

.tc-row-error {
  font-size: 12px;
  color: #c50f1f;
  white-space: pre-wrap;
}

.tc-level {
  font-size: 12px;
  opacity: 0.75;
  white-space: nowrap;
}

.tc-status {
  white-space: nowrap;
}

.tc-warn {
  color: #bc4b09;
}

.tc-list {
  position: relative;
  display: grid;
  gap: 16px;
}

/* Đọc lại: giữ nội dung cũ mờ bên dưới, không xoá trắng. */
.tc-dim > .tc-section {
  opacity: 0.45;
  pointer-events: none;
}

.tc-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 24px;
  z-index: 1;
}

.tc-panel {
  display: grid;
  gap: 8px;
  padding: 12px;
  border-radius: 6px;
  border: 1px solid color-mix(in srgb, currentColor 20%, transparent);
}

.tc-result-mark {
  font-weight: 600;
}

.tc-footer {
  display: grid;
  gap: 8px;
  position: sticky;
  bottom: 0;
  padding: 8px 0;
  background: inherit;
}

@media (prefers-color-scheme: dark) {
  .tc-row-error {
    color: #f1707b;
  }
  .tc-warn {
    color: #fdb983;
  }
}
```

- [ ] **Step 4: Chạy test, typecheck và toàn bộ Vitest, thấy qua**

Run: `npx vitest run src/features/tinh-chinh/views.test.tsx` rồi `npm run typecheck` rồi `npm test`
Expected: `Tests  10 passed (10)`; typecheck không lỗi; `npm test` `0 failed` (45 test của tính năng + test của v0.1 đang có trên nhánh). Cảnh báo `Keyborg instance … disposed incorrectly` trên stderr là của Fluent UI trong jsdom — không phải lỗi.

- [ ] **Step 5: Commit**

```bash
rtk git add src/features/tinh-chinh/TweakList.tsx
rtk git add src/features/tinh-chinh/Dialogs.tsx
rtk git add src/features/tinh-chinh/RunPanel.tsx
rtk git add src/features/tinh-chinh/TinhChinhView.tsx
rtk git add src/features/tinh-chinh/tinh-chinh.css
rtk git add src/features/tinh-chinh/views.test.tsx
rtk git commit -m "feat(ui-tinh-chinh): màn Tinh chỉnh — mức sẵn, danh sách, xác nhận, điểm khôi phục, tiến độ, kết quả

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Tích hợp — workspace, vỏ Tauri, chuỗi chung, tab trong `App`, CI, thử tay

Task **duy nhất** sửa file của v0.1. Chạy khi `feat/v0.1-don-o-dia` đã gộp đủ Task 16 (App), 17 (vỏ Tauri) và 18 (CI). Đầu task: gộp `feat/v0.1-don-o-dia` vào `feat/tinh-chinh` (mục «Đồng bộ với v0.1»), rồi tách worktree `task/tc-12-tich-hop`.

> Nếu `feat/kham-may` đã gộp trước và `App.tsx`/`main.rs`/`ci.yml`/`vi.json` đã khác bản của kế hoạch v0.1: giữ mọi thứ nhánh đó thêm, chỉ **cộng thêm** đúng các thay đổi dưới (một `Tab` + một khối nội dung trong `App.tsx`, 5 lệnh + một `.manage` trong `main.rs`, 2 bước CI, khoá chuỗi mới). Nếu hai nhánh cùng thêm `TabList` thì gộp làm một `TabList` ba tab.

**Files:**
- Modify: `Cargo.toml` (gốc) — thêm member
- Modify: `Cargo.lock` (gốc) — cargo cập nhật
- Modify: `crates/winfreeup-tweaks/Cargo.toml` — xoá bảng `[workspace]`
- Delete: `crates/winfreeup-tweaks/Cargo.lock`
- Modify: `crates/winfreeup-tweaks/src/catalog.rs` — đường dẫn chuỗi tiếng Việt
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/tweak_commands.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/i18n/vi.json` — trộn chuỗi, `app.title`, `tabs.clean`
- Delete: `src/features/tinh-chinh/vi.json`, `src/features/tinh-chinh/catalog.vi.json`
- Modify: `src/features/tinh-chinh/strings.ts`, `src/features/tinh-chinh/labels.test.ts`
- Modify: `src/App.tsx`, `src/App.test.tsx`, `vite.config.ts`
- Modify: `.github/workflows/ci.yml`
- Create: `docs/thu-tay-tinh-chinh.md`

**Interfaces:**
- Consumes:
  - Task 2: `catalog::builtin() -> Result<Vec<Tweak>, String>`; Task 3: `undo::default_path(local_appdata: &Path) -> PathBuf`; Task 5: `engine::{read_all, apply, revert, log_lines, ReadResult, RunReport, TweakEvent}`; Task 8: `sys_windows::real::RealTweakOps` (+ trait `TweakOps` để gọi `restart_explorer`).
  - Task 9–11: `tauriTweakApi`, `TweakApi`, `TinhChinhView({ api, notify, dryRun, onBusyChange })`, `readResult` (testdata).
  - v0.1 Task 8: `winfreeup_core::engine::RestorePointStatus`, `winfreeup_core::log::{log_dir(local_appdata: &Path) -> PathBuf, CleanLog::create(dir: &Path, at: DateTime<Local>) -> Result<CleanLog>, CleanLog::line(&self, text: &str)}`. v0.1 Task 1/9: `Env { local_appdata: PathBuf, sys: Arc<dyn SystemOps>, .. }`, `Env::from_system()`, `SystemOps::create_restore_point(&self, description: &str) -> Result<()>`. v0.1 Task 16: `App({ api? })`, `c.Notify`, `state.dryRun`, `state.phase`. v0.1 Task 17: `main.rs` như kế hoạch v0.1 (biến `env`, `dry_run`, `webview_check::show_fatal`). v0.1 Task 18: `.github/workflows/ci.yml`.
- Produces: lệnh Tauri `tweaks_read() -> Result<ReadResult, String>`, `tweaks_prepare_restore_point() -> Result<RestorePointStatus, String>`, `tweaks_apply(ids: Vec<String>, all_users: bool) -> Result<RunReport, String>`, `tweaks_revert(ids: Vec<String>) -> Result<RunReport, String>`, `tweaks_restart_explorer() -> Result<(), String>`; sự kiện `tweak-progress` (payload `TweakEvent`); lỗi `"busy"` / `"dry_run"`. `App({ api?, tweakApi? })` có hai tab «Dọn ổ đĩa» / «Tinh chỉnh».

- [ ] **Step 1: Đưa crate vào workspace**

`Cargo.toml` (gốc) — đổi dòng `members` (bản sau Task 17 của v0.1 là `["crates/winfreeup-core", "src-tauri"]`):

```toml
members = ["crates/winfreeup-core", "crates/winfreeup-tweaks", "src-tauri"]
```

`crates/winfreeup-tweaks/Cargo.toml` — xoá hai dòng sau (chú thích và bảng rỗng), giữ nguyên phần còn lại:

```toml
# Tách khỏi workspace gốc cho tới Task 12 (Tích hợp) — Cargo.toml gốc do v0.1 sở hữu. Task 12 xoá bảng này.
[workspace]
```

Rồi:

```bash
rtk git rm crates/winfreeup-tweaks/Cargo.lock
```

`crates/winfreeup-tweaks/src/catalog.rs` — trong module `tests`, thay hai dòng:

```rust
    /// Chuỗi tên/mô tả của từng mục — Task Tích hợp đổi đường dẫn sang `src/i18n/vi.json`.
    const VI_JSON: &str = include_str!("../../../src/features/tinh-chinh/catalog.vi.json");
```

bằng:

```rust
    /// Chuỗi tên/mô tả của từng mục nằm chung file tiếng Việt của giao diện.
    const VI_JSON: &str = include_str!("../../../src/i18n/vi.json");
```

(Test này sẽ đỏ cho tới Step 3 — đúng thứ tự TDD: đổi nguồn trước, trộn chuỗi sau.)

- [ ] **Step 2: Chạy test crate, thấy hỏng**

Run: `cargo test -p winfreeup-tweaks`
Expected: FAIL — `builtin_catalog_is_valid` báo `missing string tweaks.item.ads_id.name` … (chuỗi chưa trộn vào `src/i18n/vi.json`).

- [ ] **Step 3: Trộn chuỗi vào `src/i18n/vi.json`, xoá JSON riêng, đổi `strings.ts`**

Tạo file tạm `%TEMP%\tron-vi.cjs` (ngoài repo) với nội dung:

```js
// Task 12: trộn chuỗi của tab Tinh chỉnh vào src/i18n/vi.json, giữ nguyên định dạng phần cũ.
const fs = require('fs');
const base = 'src/i18n/vi.json';
const oldText = fs.readFileSync(base, 'utf8');
const old = JSON.parse(oldText);
const add = {
  'tabs.clean': 'Dọn ổ đĩa',
  ...JSON.parse(fs.readFileSync('src/features/tinh-chinh/vi.json', 'utf8')),
  ...JSON.parse(fs.readFileSync('src/features/tinh-chinh/catalog.vi.json', 'utf8')),
};
for (const k of Object.keys(add)) if (k in old) throw new Error(`trùng khoá: ${k}`);
const titleOld = '"app.title": "WinFreeUp — Dọn ổ đĩa"';
if (!oldText.includes(titleOld)) throw new Error('không thấy app.title cũ — sửa tay rồi chạy lại');
const head = oldText.replace(titleOld, '"app.title": "WinFreeUp"').replace(/\s*\}\s*$/, '');
const lines = Object.entries(add).map(([k, v]) => `  ${JSON.stringify(k)}: ${JSON.stringify(v)}`);
fs.writeFileSync(base, `${head},\n\n${lines.join(',\n')}\n}\n`);
console.log(`đã thêm ${lines.length} khoá`);
```

Run (PowerShell, gốc worktree):

```powershell
node "$env:TEMP\tron-vi.cjs"
rtk git rm src/features/tinh-chinh/vi.json
rtk git rm src/features/tinh-chinh/catalog.vi.json
```

Expected: `đã thêm 160 khoá` (1 `tabs.clean` + 63 khoá giao diện + 96 khoá danh mục). Script dừng nếu có khoá trùng hoặc `app.title` đã bị nhánh khác đổi — khi đó sửa tay `app.title` thành `WinFreeUp` rồi bỏ dòng kiểm trong script.

`src/features/tinh-chinh/strings.ts` — thay TOÀN BỘ nội dung bằng:

```ts
// Chuỗi của tab Tinh chỉnh nằm chung src/i18n/vi.json (Task 12 đã trộn) — giữ tên `tt` để component không phải sửa.
export { t as tt, hasKey } from '../../i18n';
export type { Params } from '../../i18n';
```

`src/features/tinh-chinh/labels.test.ts` — thay dòng import hai JSON:

```ts
import catalogVi from './catalog.vi.json';
import ui from './vi.json';
```

bằng:

```ts
import vi from '../../i18n/vi.json';
```

và thay test cuối (`không có chuỗi rỗng, và khoá UI không trùng khoá danh mục`) bằng:

```ts
  it('mọi chuỗi tweaks.* đã nằm trong vi.json chung và khác rỗng', () => {
    const keys = Object.keys(vi).filter((k) => k.startsWith('tweaks.'));
    expect(keys).toContain('tweaks.item.app_clipchamp.name');
    for (const k of keys) expect((vi as Record<string, string>)[k].trim(), k).not.toBe('');
  });
```

Run: `cargo test -p winfreeup-tweaks` rồi `npx vitest run src/features/tinh-chinh src/i18n`
Expected: Rust `58 passed; 0 failed`; Vitest `0 failed` (45 test tính năng + test i18n của v0.1).

- [ ] **Step 4: Lệnh Tauri**

`src-tauri/Cargo.toml` — thêm vào `[dependencies]`:

```toml
winfreeup-tweaks = { path = "../crates/winfreeup-tweaks" }
chrono = { version = "0.4", default-features = false, features = ["clock", "std"] }
```

`src-tauri/src/tweak_commands.rs`:

```rust
//! Lệnh Tauri của tab Tinh chỉnh. Khoá «busy» riêng; ghi nhật ký chung vào `%LOCALAPPDATA%\WinFreeUp\logs`.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::Local;
use tauri::{AppHandle, Emitter, State};
use winfreeup_core::engine::RestorePointStatus;
use winfreeup_core::log::{log_dir, CleanLog};
use winfreeup_core::SystemOps;
use winfreeup_tweaks::engine::{self, ReadResult, RunReport};
use winfreeup_tweaks::sys_windows::real::RealTweakOps;
use winfreeup_tweaks::{Tweak, TweakOps};

pub struct TweakState {
    catalog: Vec<Tweak>,
    undo_path: PathBuf,
    log_dir: PathBuf,
    sys: Arc<dyn SystemOps>,
    dry_run: bool,
    busy: AtomicBool,
}

pub type TweakShared = Arc<TweakState>;

impl TweakState {
    /// `Err` chỉ khi danh mục nhúng hỏng (test danh mục chặn trước khi build).
    pub fn new(sys: Arc<dyn SystemOps>, local_appdata: &Path, dry_run: bool) -> Result<TweakShared, String> {
        Ok(Arc::new(TweakState {
            catalog: winfreeup_tweaks::catalog::builtin()?,
            undo_path: winfreeup_tweaks::undo::default_path(local_appdata),
            log_dir: log_dir(local_appdata),
            sys,
            dry_run,
            busy: AtomicBool::new(false),
        }))
    }

    fn write_log(&self, action: &str, r: &RunReport) {
        if let Ok(log) = CleanLog::create(&self.log_dir, Local::now()) {
            for l in engine::log_lines(action, r) {
                log.line(&l);
            }
        }
    }
}

struct Busy(TweakShared);

impl Busy {
    fn acquire(st: &TweakShared) -> Result<Busy, String> {
        if st.busy.swap(true, Ordering::SeqCst) {
            Err("busy".into())
        } else {
            Ok(Busy(st.clone()))
        }
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        self.0.busy.store(false, Ordering::SeqCst);
    }
}

fn writable(st: &TweakShared) -> Result<Busy, String> {
    if st.dry_run {
        return Err("dry_run".into());
    }
    Busy::acquire(st)
}

#[tauri::command]
pub async fn tweaks_read(state: State<'_, TweakShared>) -> Result<ReadResult, String> {
    let st: TweakShared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || engine::read_all(&st.catalog, &RealTweakOps, &st.undo_path))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn tweaks_prepare_restore_point(state: State<'_, TweakShared>) -> Result<RestorePointStatus, String> {
    let st: TweakShared = state.inner().clone();
    if st.dry_run {
        return Ok(RestorePointStatus::Skipped);
    }
    let guard = Busy::acquire(&st)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        match st.sys.create_restore_point("WinFreeUp - Tinh chinh") {
            Ok(()) => RestorePointStatus::Created,
            Err(e) => RestorePointStatus::Failed { message: e.to_string() },
        }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn tweaks_apply(app: AppHandle, state: State<'_, TweakShared>, ids: Vec<String>, all_users: bool) -> Result<RunReport, String> {
    let st: TweakShared = state.inner().clone();
    let guard = writable(&st)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let r = engine::apply(&st.catalog, &RealTweakOps, &st.undo_path, &ids, all_users, &|e| {
            let _ = app.emit("tweak-progress", e);
        });
        st.write_log("APPLY", &r);
        r
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn tweaks_revert(app: AppHandle, state: State<'_, TweakShared>, ids: Vec<String>) -> Result<RunReport, String> {
    let st: TweakShared = state.inner().clone();
    let guard = writable(&st)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let r = engine::revert(&st.catalog, &RealTweakOps, &st.undo_path, &ids, &|e| {
            let _ = app.emit("tweak-progress", e);
        });
        st.write_log("REVERT", &r);
        r
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn tweaks_restart_explorer(state: State<'_, TweakShared>) -> Result<(), String> {
    let st: TweakShared = state.inner().clone();
    let guard = writable(&st)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        RealTweakOps.restart_explorer()
    })
    .await
    .map_err(|e| e.to_string())?
}
```

`src-tauri/src/main.rs` — ba chỗ sửa trên bản của kế hoạch v0.1 Task 17:

1. Dưới `mod commands;` thêm `mod tweak_commands;`.
2. Ngay sau khối `let env = match winfreeup_core::Env::from_system() { … };` thêm (trước khi `env` bị chuyển vào `AppState::new`):

```rust
    let tweaks = match tweak_commands::TweakState::new(env.sys.clone(), &env.local_appdata, dry_run) {
        Ok(t) => t,
        Err(e) => {
            webview_check::show_fatal(&e);
            return;
        }
    };
```

3. Trong chuỗi builder: thêm `.manage(tweaks)` ngay sau `.manage(commands::AppState::new(env, dry_run))`, và thêm 5 dòng vào cuối danh sách `tauri::generate_handler![…]`:

```rust
            tweak_commands::tweaks_read,
            tweak_commands::tweaks_prepare_restore_point,
            tweak_commands::tweaks_apply,
            tweak_commands::tweaks_revert,
            tweak_commands::tweaks_restart_explorer,
```

Run: `cargo build -p winfreeup` rồi `cargo clippy -p winfreeup -p winfreeup-tweaks --all-targets -- -D warnings`
Expected: build và clippy không lỗi. (Đo khi viết kế hoạch: `tweak_commands.rs` + `generate_handler!` 5 lệnh biên dịch sạch với `cargo clippy -- -D warnings` trên tauri 2.11.6 và `winfreeup-core` của `feat/v0.1-don-o-dia` @ dd517be; `main.rs` chưa đo vì Task 17 của v0.1 chưa có.)

- [ ] **Step 5: Viết test hỏng cho tab trong `App`**

`src/App.test.tsx` — thêm vào dòng import của Testing Library chữ `within`, thêm hai import dưới `import { App } from './App';`:

```tsx
import type { TweakApi } from './features/tinh-chinh/types';
import { readResult } from './features/tinh-chinh/testdata';
```

và thêm vào CUỐI file:

```tsx
function fakeTweakApi(): TweakApi {
  return {
    read: vi.fn(async () => readResult()),
    prepareRestorePoint: vi.fn(async () => ({ status: 'created' as const })),
    apply: vi.fn(async () => ({ outcomes: [], restart: 'none' as const, notices: [] })),
    revert: vi.fn(async () => ({ outcomes: [], restart: 'none' as const, notices: [] })),
    restartExplorer: vi.fn(async () => {}),
  };
}

describe('App — tab Tinh chỉnh', () => {
  it('chỉ đọc trạng thái máy khi mở tab lần đầu, rồi giữ nguyên khi quay lại', async () => {
    const tweakApi = fakeTweakApi();
    render(<App api={fakeApi()} tweakApi={tweakApi} />);
    await screen.findByText('Ổ C: còn trống 50 GB');
    expect(tweakApi.read).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('tab', { name: 'Tinh chỉnh' }));
    expect(await screen.findByText('Quyền riêng tư & quảng cáo')).toBeTruthy();
    fireEvent.click(screen.getByRole('tab', { name: 'Dọn ổ đĩa' }));
    fireEvent.click(screen.getByRole('tab', { name: 'Tinh chỉnh' }));
    expect(tweakApi.read).toHaveBeenCalledTimes(1);
  });

  it('đang quét thì không chuyển sang tab Tinh chỉnh được', async () => {
    const api = fakeApi({ scanAll: vi.fn(() => new Promise<never>(() => {})) });
    render(<App api={api} tweakApi={fakeTweakApi()} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Quét' }));
    const tab = await screen.findByRole('tab', { name: 'Tinh chỉnh' });
    expect(tab.getAttribute('aria-disabled') === 'true' || (tab as HTMLButtonElement).disabled).toBe(true);
    within(screen.getByRole('tablist')).getByRole('tab', { name: 'Dọn ổ đĩa' });
  });
});
```

Run: `npx vitest run src/App.test.tsx`
Expected: FAIL — `Unable to find role="tab" and name "Tinh chỉnh"`.

- [ ] **Step 6: Lắp tab vào `App.tsx` và nới thời gian chờ của Vitest**

`src/App.tsx` — bản đầy đủ sau khi thêm (so với bản Task 16 của v0.1: import `Tab`, `TabList`, `TweakApi`, `tauriTweakApi`, `TinhChinhView`; prop `tweakApi`; ba state `tab`/`tweaksOpened`/`tweaksBusy`; `TabList`; bọc nội dung cũ trong `<div hidden={tab !== 'clean'}>`; khối Tinh chỉnh chỉ dựng khi mở lần đầu):

```tsx
import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import { Badge, FluentProvider, Tab, TabList, Title2, webDarkTheme, webLightTheme } from '@fluentui/react-components';
import type { Api } from './api/types';
import { tauriApi } from './api/tauri';
import { dismissNotice, installGlobalHooks, pushNotice, type Notice } from './errors/errors';
import { t } from './i18n';
import { initialState, reducer } from './state/machine';
import { createStore } from './state/store';
import * as c from './state/controller';
import { NoticeBar } from './components/NoticeBar';
import { ErrorBoundary } from './components/ErrorBoundary';
import { WelcomeView } from './components/WelcomeView';
import { ScanningView } from './components/ScanningView';
import { PreviewView } from './components/PreviewView';
import { ConfirmDialog } from './components/ConfirmDialog';
import { RestorePointView } from './components/RestorePointView';
import { CleaningView } from './components/CleaningView';
import { ResultView } from './components/ResultView';
import type { TweakApi } from './features/tinh-chinh/types';
import { tauriTweakApi } from './features/tinh-chinh/api';
import { TinhChinhView } from './features/tinh-chinh/TinhChinhView';

const DARK = '(prefers-color-scheme: dark)';

type TabId = 'clean' | 'tweaks';

function usePrefersDark(): boolean {
  const [dark, setDark] = useState(() => window.matchMedia(DARK).matches);
  useEffect(() => {
    const m = window.matchMedia(DARK);
    const on = (e: MediaQueryListEvent) => setDark(e.matches);
    m.addEventListener('change', on);
    return () => m.removeEventListener('change', on);
  }, []);
  return dark;
}

export function App({ api = tauriApi, tweakApi = tauriTweakApi }: { api?: Api; tweakApi?: TweakApi }) {
  const [store] = useState(() => createStore(reducer, initialState));
  const state = useSyncExternalStore(store.subscribe, store.getState);
  const [notices, setNotices] = useState<Notice[]>([]);
  const notify = useCallback<c.Notify>((severity, message) => {
    (severity === 'error' ? console.error : console.warn)('[WinFreeUp]', message);
    setNotices((l) => pushNotice(l, severity, message));
  }, []);
  const deps = useMemo(() => c.createDeps(api, store, notify), [api, store, notify]);
  useEffect(() => installGlobalHooks(window, notify), [notify]);
  useEffect(() => {
    void c.loadInitial(deps);
  }, [deps]);
  const dark = usePrefersDark();
  const run = (p: Promise<void>) => {
    p.catch((e) => notify('error', c.friendly(e)));
  };
  const [tab, setTab] = useState<TabId>('clean');
  // Tab Tinh chỉnh chỉ dựng (và đọc trạng thái máy) khi mở lần đầu; sau đó giữ nguyên để không mất tiến độ.
  const [tweaksOpened, setTweaksOpened] = useState(false);
  const [tweaksBusy, setTweaksBusy] = useState(false);
  const cleanBusy = state.phase === 'scanning' || state.phase === 'confirm' || state.phase === 'restorePoint' || state.phase === 'cleaning';

  return (
    <FluentProvider theme={dark ? webDarkTheme : webLightTheme} className="wfu-root">
      <main className="wfu-app">
        <header className="wfu-header">
          <Title2 as="h1">{t('app.title')}</Title2>
          {state.dryRun && (
            <Badge appearance="filled" color="warning">
              {t('app.dryRunBadge')}
            </Badge>
          )}
        </header>
        <TabList
          selectedValue={tab}
          onTabSelect={(_, d) => {
            const next = d.value as TabId;
            setTab(next);
            if (next === 'tweaks') setTweaksOpened(true);
          }}
        >
          <Tab value="clean" disabled={tweaksBusy}>
            {t('tabs.clean')}
          </Tab>
          <Tab value="tweaks" disabled={cleanBusy}>
            {t('tweaks.tab')}
          </Tab>
        </TabList>
        <NoticeBar notices={notices} onDismiss={(id) => setNotices((l) => dismissNotice(l, id))} />
        <div hidden={tab !== 'clean'}>
          <ErrorBoundary onReset={() => run(c.goHome(deps))}>
            {state.phase === 'welcome' && <WelcomeView state={state} onScan={() => run(c.startScan(deps))} />}
            {state.phase === 'scanning' && <ScanningView state={state} onCancel={() => run(c.cancelScan(deps))} />}
            {(state.phase === 'preview' || state.phase === 'confirm') && (
              <PreviewView
                state={state}
                onToggle={(id) => c.toggle(deps, id)}
                onClean={() => run(c.requestClean(deps))}
                onRescan={() => run(c.startScan(deps))}
              />
            )}
            {state.phase === 'confirm' && (
              <ConfirmDialog state={state} onAccept={() => run(c.acceptConfirm(deps))} onCancel={() => c.cancelConfirm(deps)} />
            )}
            {state.phase === 'restorePoint' && (
              <RestorePointView
                state={state}
                onContinue={() => run(c.continueAfterRestoreFailure(deps))}
                onAbort={() => c.abortAfterRestoreFailure(deps)}
              />
            )}
            {state.phase === 'cleaning' && <CleaningView state={state} />}
            {state.phase === 'result' && <ResultView state={state} onOpenLog={() => c.openLog(deps)} onHome={() => run(c.goHome(deps))} />}
          </ErrorBoundary>
        </div>
        {tweaksOpened && (
          <div hidden={tab !== 'tweaks'}>
            <ErrorBoundary onReset={() => setTab('clean')}>
              <TinhChinhView api={tweakApi} notify={notify} dryRun={state.dryRun} onBusyChange={setTweaksBusy} />
            </ErrorBoundary>
          </div>
        )}
      </main>
    </FluentProvider>
  );
}
```

`vite.config.ts` — trong khối `test`, thêm `testTimeout: 20000` ở đầu:

```ts
  test: { testTimeout: 20000, environment: 'jsdom', setupFiles: ['src/test/setup.ts'], css: false },
```

Vì sao: đo trên máy dev đang tải nặng (2026-09-25), test «đi trọn luồng Chào → … → Về đầu» của v0.1 mất 4,2 s trước tích hợp và 7,1 s sau khi có `TabList` — sát/vượt mặc định 5 s nên hỏng ngẫu nhiên; chạy riêng với thời gian chờ 30 s thì qua.

Run: `npm test` rồi `npm run build`
Expected: `0 failed` (đo khi viết kế hoạch trên bản dựng thử: `Test Files 16 passed`, `Tests 125 passed` — 78 của v0.1 (mã thật trên `feat/v0.1-don-o-dia` + Task 13 + App theo kế hoạch v0.1), 45 của tính năng, 2 test App mới); build ra `dist/`.

- [ ] **Step 7: CI**

`.github/workflows/ci.yml` — thêm hai bước ngay sau bước `clippy (lõi)`:

```yaml
      - name: cargo test (tinh chỉnh)
        run: cargo test -p winfreeup-tweaks
      - name: clippy (tinh chỉnh)
        run: cargo clippy -p winfreeup-tweaks --all-targets -- -D warnings
```

Ghi chú (suy luận, chưa đo trên runner): test của crate ghi `HKCU\Software\WinFreeUpTest\…` của tài khoản runner, đọc SCM/Task Scheduler/PackageManager — đều không cần Admin trên máy dev. Hai test giả định có tác vụ `\Microsoft\Windows\Defrag\ScheduledDefrag` và gói `Microsoft.Windows.ShellExperienceHost_cw5n1h2txyewy`; nếu runner `windows-latest` (Windows Server) thiếu chúng, đổi sang tác vụ/gói có trên runner thay vì bỏ test.

- [ ] **Step 8: Danh sách thử tay `docs/thu-tay-tinh-chinh.md`**

```markdown
# Thử tay tab Tinh chỉnh

Chỉ thử trong **Windows Sandbox** (Win 11 Pro) hoặc **máy ảo** (Win 10 22H2 Pro, Win 11 23H2/24H2 Home) —
không bao giờ trên máy chính. Chép `WinFreeUp.exe` vào máy thử. Mỗi dòng: áp dụng ⇒ đọc lại ⇒ hoàn tác ⇒ đọc lại
(spec mục 7). Sau mỗi lượt mở `%LOCALAPPDATA%\WinFreeUp\logs\` đọc các dòng `TWEAK …` và mở
`%LOCALAPPDATA%\WinFreeUp\tweaks-undo.json`.

| # | Việc làm | Kỳ vọng |
|---|---|---|
| 1 | Mở app, bấm tab «Tinh chỉnh» | Vòng quay 1–3 giây rồi danh sách; mức «Cơ bản» tô đậm; chỉ app có trên máy mới hiện; «Tắt Recall» trên máy build < 26100 hiện mờ «cần Windows 11 24H2 trở lên» |
| 2 | Trước khi áp dụng, chạy `reg query HKCU\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo /v Enabled` và `…\ContentDeliveryManager` | Ghi lại giá trị gốc để so ở bước hoàn tác |
| 3 | Bấm «Áp dụng N thay đổi» ở mức Cơ bản | Không có hộp xác nhận (toàn mục an toàn); tạo điểm khôi phục (Sandbox: băng hổ phách «Không tạo được điểm khôi phục» + «Vẫn áp dụng»/«Dừng lại»); tiến độ từng mục; kết quả ✓; nút «Khởi động lại Explorer» (vì `ads_explorer`) |
| 4 | Bấm «Khởi động lại Explorer» | Thanh tác vụ tắt rồi hiện lại trong vài giây, **không** chạy quyền Admin (`tasklist /v /fi "imagename eq explorer.exe"` cùng tài khoản thường) |
| 5 | Đọc lại (#2) | `Enabled = 0`, các value ContentDeliveryManager = 0; `tweaks-undo.json` có ảnh chụp đúng giá trị gốc (vắng ⇒ `"data": null`) |
| 6 | Bấm «Áp dụng» lần nữa sau khi sửa tay `Enabled` thành 5 | Ảnh chụp trong `tweaks-undo.json` **không đổi** (vẫn giá trị gốc ở #2) |
| 7 | Tích các mục Cơ bản, bấm «Hoàn tác đã chọn» | Registry trở về đúng #2 (value vốn vắng thì bị xoá); ảnh chụp của các mục đó biến khỏi file |
| 8 | Mức «Khuyến nghị» ⇒ Áp dụng | Hộp xác nhận liệt kê «Tắt dịch vụ gửi dữ liệu chẩn đoán (DiagTrack)»; sau áp dụng `sc qc DiagTrack` ⇒ `DISABLED`, `sc query DiagTrack` ⇒ `STOPPED`; `schtasks /query /tn "\Microsoft\Windows\Customer Experience Improvement Program\Consolidator"` ⇒ Disabled |
| 9 | Hoàn tác các mục #8 | `sc qc DiagTrack` ⇒ `AUTO_START` nhưng dịch vụ **không** tự chạy lại; tác vụ CEIP bật lại |
| 10 | Bản **Home** (VM): mục «Hạ dữ liệu chẩn đoán» | Hiện «xuống mức "Bắt buộc"», ghi `AllowTelemetry = 1`; mục «thấp nhất» (Enterprise/Education) hiện «không áp dụng cho bản Windows này» |
| 11 | Gỡ Clipchamp (mức Cơ bản) | App biến khỏi Start; dòng hiện «Đã gỡ» + nút «Cài lại từ Store» |
| 12 | Bấm «Cài lại từ Store» | Store mở **đúng trang Clipchamp**; đóng Store không cài ⇒ nút vẫn còn; cài xong, mở lại tab ⇒ «Chưa gỡ» |
| 13 | Với **từng app** có trên VM (Win 10 22H2 sạch và Win 11 sạch có đủ app quảng cáo): gỡ ⇒ «Cài lại từ Store» | Store mở đúng trang của app đó — ghi ProductId nào sai vào bảng «Xác minh danh mục» của kế hoạch và sửa `catalog.toml` |
| 14 | Tích «Nâng cao: gỡ cho mọi tài khoản…», chọn một app, Áp dụng | Hộp xác nhận liệt kê app và câu «Khó hoàn tác…»; sau đó tạo tài khoản mới, đăng nhập ⇒ app không tự cài lại (`Get-AppxProvisionedPackage -Online` không còn gói đó) |
| 15 | Mức «Triệt để», gỡ Xbox Game Bar | Hộp xác nhận có ⚠; sau gỡ Win+G không mở gì |
| 16 | Chạy app bằng tài khoản **thường**, UAC nhập mật khẩu **admin khác** | Băng hổ phách «WinFreeUp đang chạy bằng một tài khoản quản trị khác…» |
| 17 | Máy vào domain hoặc Intune (nếu có VM): đặt trước `HKLM\SOFTWARE\Policies\Microsoft\Windows\DataCollection AllowTelemetry = 3` | Mục telemetry hiện «Do tổ chức quản lý», không tích được |
| 18 | Ghi rác vào `%LOCALAPPDATA%\WinFreeUp\tweaks-undo.json`, mở tab | Băng hổ phách «Không đọc được dữ liệu hoàn tác; hoàn tác sẽ dùng giá trị mặc định của Windows…»; file thành `tweaks-undo.json.bak` |
| 19 | `WinFreeUp.exe --dry-run`, tab Tinh chỉnh | Băng «Chạy thử…»; nút Áp dụng/Hoàn tác/«mọi tài khoản» bị khoá |
| 20 | Đang Quét ở tab «Dọn ổ đĩa» | Tab «Tinh chỉnh» bị khoá; ngược lại đang áp dụng tinh chỉnh thì tab «Dọn ổ đĩa» bị khoá |
| 21 | Win 10 22H2: `task_appraiser` | Tác vụ `Microsoft Compatibility Appraiser` bị tắt; tác vụ «Exp» vắng thì mục vẫn «Đã áp dụng» |
| 22 | Bật `Animation effects: Off`, mở tab | Vòng quay đổi thành nhịp mờ tỏ, vẫn chuyển động |

Ghi kết quả từng dòng (đạt/không đạt + ảnh chụp) vào PR của bản phát hành.
```

- [ ] **Step 9: Build exe, kiểm và thử tay nhanh trên máy dev (chỉ ĐỌC)**

Run (PowerShell, gốc worktree):

```powershell
cargo test -p winfreeup-core
cargo test -p winfreeup-tweaks
cargo clippy -p winfreeup-tweaks --all-targets -- -D warnings
npm test
npx tauri build --no-bundle
cmd /c "findstr /m requireAdministrator target\release\WinFreeUp.exe"
```

Expected: mọi lệnh thoát mã 0. Mở `target\release\WinFreeUp.exe --dry-run` (UAC) ⇒ bấm tab «Tinh chỉnh» ⇒ vòng quay rồi danh sách; băng xanh «Chạy thử…»; nút Áp dụng/Hoàn tác bị khoá. **Không bấm Áp dụng trên máy chính** — thử thật theo `docs/thu-tay-tinh-chinh.md` trong Sandbox/VM.

- [ ] **Step 10: Commit**

```bash
rtk git add Cargo.toml
rtk git add Cargo.lock
rtk git add crates/winfreeup-tweaks/Cargo.toml
rtk git add crates/winfreeup-tweaks/src/catalog.rs
rtk git add src-tauri/Cargo.toml
rtk git add src-tauri/src/tweak_commands.rs
rtk git add src-tauri/src/main.rs
rtk git add src/i18n/vi.json
rtk git add src/features/tinh-chinh/strings.ts
rtk git add src/features/tinh-chinh/labels.test.ts
rtk git add src/App.tsx
rtk git add src/App.test.tsx
rtk git add vite.config.ts
rtk git add .github/workflows/ci.yml
rtk git add docs/thu-tay-tinh-chinh.md
rtk git commit -m "feat: tích hợp tab Tinh chỉnh — workspace, 5 lệnh tweaks_*, chuỗi chung, TabList, CI, danh sách thử tay

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

(`crates/winfreeup-tweaks/Cargo.lock`, `src/features/tinh-chinh/vi.json`, `catalog.vi.json` đã được `git rm` đưa vào index ở Step 1 và 3.)

---

## Sau Task 12 — trước khi báo xong

- [ ] Trên `feat/tinh-chinh`: `cargo test -p winfreeup-core`, `cargo test -p winfreeup-tweaks` (Expected `58 passed; 0 failed`), `npm test` (`0 failed`), `npx tauri build --no-bundle` thành công.
- [ ] Quét dấu làm dở: `rtk grep -n "TODO\|todo!\|unimplemented!\|\.only(\|\.skip(\|khung —\|nội dung ở Task" crates/winfreeup-tweaks src/features src-tauri` ⇒ không kết quả (dòng khung `//!` phải đã bị thay hết).
- [ ] Chạy đủ `docs/thu-tay-tinh-chinh.md` trong Windows Sandbox (Win 11) và máy ảo (Win 10 22H2, Win 11 Home); ghi đạt/không đạt; cập nhật bảng «Xác minh danh mục» và sửa `catalog.toml` nếu tên gói/ProductId sai.
- [ ] Rà toàn nhánh bằng một reviewer mới (`superpowers:requesting-code-review`).
- [ ] **Dừng và hỏi người dùng** trước khi push `feat/tinh-chinh` hoặc merge vào `main` (CLAUDE.md mục 6).


## Xác minh danh mục (spec mục 2: «phải được xác minh trên máy thật»)

Đo ngày 2026-09-25 trên máy dev **Windows 11 Pro 25H2, build 26200.9445**, terminal không Admin, bằng lệnh **chỉ đọc** (`reg query`, `Get-AppxPackage`, `Get-ScheduledTask`, `Get-CimInstance Win32_Service`) và bằng `read_all` của chính crate (mọi mục đọc được, không lỗi). Không ghi, không gỡ gì trên máy này. Máy dev đã được người dùng dọn bớt app từ trước, nên nhiều app «vắng» chỉ có nghĩa là không đo được ở đây — **không** có nghĩa tên gói sai.

### Registry, dịch vụ, tác vụ

| Mục | Khoá/đối tượng | Đo trên máy dev | `default` trong danh mục |
|---|---|---|---|
| `ads_id` | `HKCU\…\AdvertisingInfo` `Enabled` | key có, `1` | `1` |
| `ads_start_suggestions` | `HKCU\…\ContentDeliveryManager` — `SystemPaneSuggestionsEnabled`, `SubscribedContent-338389Enabled`, `RotatingLockScreenOverlayEnabled`, `SilentInstalledAppsEnabled` | key có; bốn value `= 1` | `1` |
| `ads_start_suggestions` | cùng key — `SubscribedContent-338388Enabled`, `-338387Enabled`, `-353694Enabled`, `-353696Enabled` | value **vắng** | `"absent"` (vắng = Windows coi là bật) — **thử tay Win 10 22H2 / Win 11 23H2 sạch** xem có tồn tại sẵn không |
| `ads_explorer` | `HKCU\…\Explorer\Advanced` `ShowSyncProviderNotifications` | key có, value vắng | `"absent"` |
| `tailored_experiences` | `HKCU\…\Privacy` `TailoredExperiencesWithDiagnosticDataEnabled` | key có, `1` | `1` |
| `scoobe` | `HKCU\…\UserProfileEngagement` `ScoobeSystemSettingEnabled` | **key vắng** | `"absent"` |
| `telemetry_min_*` | `HKLM\SOFTWARE\Policies\Microsoft\Windows\DataCollection` `AllowTelemetry` | key vắng | `"absent"`; bản Pro ⇒ `telemetry_min_security` «không áp dụng» (đo qua `read_all`) |
| `search_no_bing` | `HKCU\Software\Policies\Microsoft\Windows\Explorer` `DisableSearchBoxSuggestions` | key vắng | `"absent"` |
| `widgets_feed_win11` / `_win10` | `HKLM\…\Policies\Microsoft\Dsh` / `…\Windows\Windows Feeds` | key vắng; `_win10` «không hỗ trợ» (build_max 19045) | `"absent"` — **thử tay Win 10 22H2** |
| `activity_history`, `cloud_clipboard` | `HKLM\…\Policies\Microsoft\Windows\System` | key vắng | `"absent"` |
| `inking_typing` | `HKCU\Software\Microsoft\Input\TIPC` `Enabled`; `HKCU\Software\Microsoft\InputPersonalization` `RestrictImplicitInkCollection`, `RestrictImplicitTextCollection` | `1`; `0`; `0` | `1`; `0`; `0` |
| `do_lan_only` | `HKLM\…\Policies\Microsoft\Windows\DeliveryOptimization` `DODownloadMode` | key vắng | `"absent"` |
| `recall_off` | `HKLM\…\Policies\Microsoft\Windows\WindowsAI` `DisableAIDataAnalysis` | key vắng; build 26200 ≥ 26100 ⇒ hỗ trợ | `"absent"` — hiệu lực thật chỉ kiểm được trên máy Copilot+ |
| `settings_sync` | `HKLM\…\Policies\Microsoft\Windows\SettingSync` | key vắng | `"absent"` |
| `svc_diagtrack` | dịch vụ `DiagTrack` | `Auto`, đang chạy (`Start = 2`) | `auto` |
| `svc_dmwappush` | dịch vụ `dmwappushservice` | `Manual`, dừng (`Start = 3`) | `manual` |
| `ceip_tasks` | `\…\Customer Experience Improvement Program\Consolidator`, `\UsbCeip` | cả hai có, `Ready` | (tác vụ: mặc định bật) |
| `task_appraiser` | `\…\Application Experience\Microsoft Compatibility Appraiser` | **vắng** trên build 26200 | — **thử tay Win 10 22H2 / Win 11 23H2** |
| `task_appraiser` | `\…\Application Experience\Microsoft Compatibility Appraiser Exp` | có, `Ready` | — |
| (máy quản lý) | domain / `HKLM\SOFTWARE\Microsoft\Enrollments` | không domain; 35 key, ProviderID chỉ rỗng / `Local Authority` / `Cloud Authority` / `Deploy Authority` | ⇒ `managed = false` (đo qua `system_info`) |

### Gói app

| Mục | Mức | PackageFamily | ProductId | Máy dev (`Get-AppxPackage`) | API Store `storeedgefd…/v9.0/products/<id>` |
|---|---|---|---|---|---|
| `app_candy_crush` | Cơ bản | `king.com.CandyCrushSaga_kgqvnymyfvs32` | `9NBLGGH18846` | vắng — thử tay VM | «Candy Crush Saga»; family khớp |
| `app_candy_crush` | Cơ bản | `king.com.CandyCrushSodaSaga_kgqvnymyfvs32` | `9NBLGGH1ZRPV` | vắng — thử tay VM | «Candy Crush Soda Saga»; family khớp |
| `app_candy_crush` | Cơ bản | `king.com.CandyCrushFriends_kgqvnymyfvs32` | `9PL3B0VQLQQ8` | vắng — thử tay VM | «Candy Crush Friends Saga»; family khớp |
| `app_tiktok` | Cơ bản | `BytedancePte.Ltd.TikTok_6yccndn6064se` | `9NH2GPH4JZS4` | vắng — thử tay VM | «TikTok»; family khớp |
| `app_instagram` | Cơ bản | `Facebook.InstagramBeta_8xx8rvfyw5nnt` | `9NBLGGH5L9XT` | vắng — thử tay VM | «Instagram»; family khớp |
| `app_disney` | Cơ bản | `Disney.37853FC22B2CE_6rarf9sa4v8jt` | `9NXQXXLFST89` | vắng — thử tay VM | «Disney+»; family khớp |
| `app_spotify` | Cơ bản | `SpotifyAB.SpotifyMusic_zpdnekdrzrea0` | `9NCBCSZSJRSB` | vắng — thử tay VM | «Spotify - Music and Podcasts»; family khớp |
| `app_tips` | Cơ bản | `Microsoft.Getstarted_8wekyb3d8bbwe` | `9WZDNCRDTBJJ` | vắng — thử tay VM | «Microsoft Tips»; family khớp |
| `app_feedback_hub` | Cơ bản | `Microsoft.WindowsFeedbackHub_8wekyb3d8bbwe` | `9NBLGGH4R32N` | vắng — thử tay VM | «Feedback Hub»; family khớp |
| `app_news` | Cơ bản | `Microsoft.BingNews_8wekyb3d8bbwe` | `9WZDNCRFHVFW` | vắng — thử tay VM | «Microsoft News»; family khớp |
| `app_office_hub` | Cơ bản | `Microsoft.MicrosoftOfficeHub_8wekyb3d8bbwe` | `9WZDNCRD29V9` | **có** — family khớp | «Microsoft 365 Copilot»; family khớp |
| `app_clipchamp` | Cơ bản | `Clipchamp.Clipchamp_yxz26nhyzhsrt` | `9P1J8S7CCWWT` | **có** — family khớp | «Microsoft Clipchamp»; family khớp |
| `app_power_automate` | Cơ bản | `Microsoft.PowerAutomateDesktop_8wekyb3d8bbwe` | `9NFTCH6J7FHV` | vắng — thử tay VM | «Power Automate»; family khớp |
| `app_mixed_reality` | Cơ bản | `Microsoft.MixedReality.Portal_8wekyb3d8bbwe` | `9NG1H8B3ZC7M` | vắng — thử tay VM | «Mixed Reality Portal»; family khớp |
| `app_3d_viewer` | Cơ bản | `Microsoft.Microsoft3DViewer_8wekyb3d8bbwe` | `9NBLGGH42THS` | vắng — thử tay VM | «3D Viewer»; family khớp |
| `app_paint_3d` | Cơ bản | `Microsoft.MSPaint_8wekyb3d8bbwe` | `9NBLGGH5FV99` | vắng — thử tay VM | «Paint 3D»; family khớp |
| `app_skype` | Cơ bản | `Microsoft.SkypeApp_kzf8qxf38zg5c` | `9WZDNCRFJ364` | vắng — thử tay VM | «Skype»; family khớp |
| `app_people` | Cơ bản | `Microsoft.People_8wekyb3d8bbwe` | `9NBLGGH10PG8` | vắng — thử tay VM | «Microsoft People»; family khớp |
| `app_weather` | Khuyến nghị | `Microsoft.BingWeather_8wekyb3d8bbwe` | `9WZDNCRFJ3Q2` | vắng — thử tay VM | «MSN Weather»; family khớp |
| `app_maps` | Khuyến nghị | `Microsoft.WindowsMaps_8wekyb3d8bbwe` | `9WZDNCRDTBVB` | vắng — thử tay VM | «Windows Maps»; family khớp |
| `app_solitaire` | Khuyến nghị | `Microsoft.MicrosoftSolitaireCollection_8wekyb3d8bbwe` | `9WZDNCRFHWD2` | vắng — thử tay VM | «Microsoft Solitaire Collection»; family khớp |
| `app_copilot` | Khuyến nghị | `Microsoft.Copilot_8wekyb3d8bbwe` | `9NHT9RB2F4HD` | **có** — family khớp | «Microsoft Copilot on Windows»; family khớp |
| `app_widgets` | Khuyến nghị | `MicrosoftWindows.Client.WebExperience_cw5n1h2txyewy` | `9MSSGKG348SP` | **có** — family khớp | «Windows Web Experience Pack»; family khớp |
| `app_family` | Khuyến nghị | `MicrosoftCorporationII.MicrosoftFamily_8wekyb3d8bbwe` | `9PDJDJS743XF` | vắng — thử tay VM | «Microsoft Family Safety»; family khớp |
| `app_cortana` | Khuyến nghị | `Microsoft.549981C3F5F10_8wekyb3d8bbwe` | `9NFFX4SZZ23L` | vắng — thử tay VM | «Cortana»; family khớp |
| `app_xbox` | Triệt để | `Microsoft.GamingApp_8wekyb3d8bbwe` | `9MV0B5HZVK9Z` | **có** — family khớp | «XBOX»; family khớp |
| `app_xbox` | Triệt để | `Microsoft.XboxApp_8wekyb3d8bbwe` | `9WZDNCRFJBD8` | vắng — thử tay VM | «Xbox Console Companion»; family khớp |
| `app_game_bar` | Triệt để | `Microsoft.XboxGamingOverlay_8wekyb3d8bbwe` | `9NZKPSTSNW4P` | **có** — family khớp | «Game Bar»; family khớp |
| `app_game_bar` | Triệt để | `Microsoft.XboxGameOverlay_8wekyb3d8bbwe` | `9NZKPSTSNW4P` | vắng — thử tay VM | «Game Bar»; family KHÁC: Microsoft.XboxGamingOverlay_8wekyb3d8bbwe |
| `app_game_bar` | Triệt để | `Microsoft.XboxSpeechToTextOverlay_8wekyb3d8bbwe` | `9NZKPSTSNW4P` | **có** — family khớp | «Game Bar»; family KHÁC: Microsoft.XboxGamingOverlay_8wekyb3d8bbwe |
| `app_phone_link` | Triệt để | `Microsoft.YourPhone_8wekyb3d8bbwe` | `9NMPJ99VJBWV` | **có** — family khớp | «Phone Link»; family khớp |
| `app_outlook_new` | Triệt để | `Microsoft.OutlookForWindows_8wekyb3d8bbwe` | `9NRX63209R7B` | **có** — family khớp | «Outlook for Windows»; family khớp |
| `app_mail_calendar` | Triệt để | `microsoft.windowscommunicationsapps_8wekyb3d8bbwe` | `9WZDNCRFHVQM` | vắng — thử tay VM | «Mail and Calendar»; family khớp |

Cột API Store: đo ngày 2026-09-25 bằng `GET https://storeedgefd.dsx.mp.microsoft.com/v9.0/products/<ProductId>?market=US&locale=en-us&deviceFamily=Windows.Desktop` (API công khai mà app Store dùng), so `Payload.Title` và `Payload.PackageFamilyNames`. Hai chỗ **KHÁC** là hai gói phụ của Game Bar (`XboxGameOverlay`, `XboxSpeechToTextOverlay`) không có trang Store riêng; danh mục tạm cho «Cài lại» mở trang Game Bar — đang chờ người dùng chọn (mục «Câu hỏi còn mở») và phải thử tay xem cài lại Game Bar có đem hai gói này về không. People (`9NBLGGH10PG8`) API vẫn trả «Microsoft People» và family khớp, dù Microsoft đã ngừng app. Khi viết kế hoạch, cách đo này đã bắt được **3 ProductId sai** (Candy Crush Soda Saga `9NBLGGH2S8Z3` và Friends Saga `9NBLGGH26TZT` trả 404; `9MSPC6MP8FM4` là Microsoft Whiteboard — mục Teams cá nhân sau đó bị bỏ khỏi danh mục, xem «Câu hỏi còn mở») — hai ID Candy Crush đã sửa trong danh mục trên. Phần còn phải thử tay: gỡ thật và «Cài lại từ Store» cho các gói «vắng» trên máy dev (bảng thử tay #11–#15).

Ghi chú: `MSTeams_8wekyb3d8bbwe` (Teams mới, dùng chung công việc và cá nhân) **có** trên máy dev và nằm trong `BLOCKED_PACKAGES` (test `exact_and_prefix_matching`, `blocked_package_in_catalog_is_rejected`). `MicrosoftTeams_8wekyb3d8bbwe` (Teams cá nhân cũ) vắng trên máy dev và hiện không có trong danh mục. `Microsoft.Paint_8wekyb3d8bbwe` (Paint mới) có trên máy và nằm trong danh sách cấm; Paint 3D là `Microsoft.MSPaint`. `Microsoft.Edge.GameAssist` khớp mẫu `Microsoft.Edge*` của PowerShell nhưng **không** khớp mẫu cấm `Microsoft.MicrosoftEdge*` — không nằm trong danh mục nên không ảnh hưởng.

## Câu hỏi còn mở (chờ người dùng chọn — không chặn Đợt A–F)

Spec bắt buộc «mọi `appx_remove` có `store_product_id`» và «mọi mục hoàn tác được từ trong app». Ba gói không có trang Store riêng (đã tra API Store, không bịa ID):

| Gói | Tình trạng | Phương án A (khuyên dùng) | Phương án B | Phương án C |
|---|---|---|---|---|
| Teams cá nhân `MicrosoftTeams_8wekyb3d8bbwe` (Chat của Win 11 21H2/22H2) | Microsoft đã bỏ; trang Store duy nhất là Teams mới `MSTeams` — app khác, lại nằm trong danh sách cấm | **Bỏ khỏi danh mục** (kế hoạch đang theo phương án này) — không gỡ thứ không cài lại được | Giữ mục, hoàn tác mở trang Teams mới `XP8BT8DW290MPQ` và nói rõ «cài bản Teams mới thay thế» | Giữ mục, cho phép `store_product_id` rỗng; mục ghi «Không cài lại được» và luôn vào hộp xác nhận |
| `Microsoft.XboxGameOverlay`, `Microsoft.XboxSpeechToTextOverlay` (gói phụ Game Bar, trong mục `app_game_bar`) | Không có trang riêng | **Giữ trong `app_game_bar`, «Cài lại» mở trang Game Bar `9NZKPSTSNW4P`** (kế hoạch đang theo phương án này); thử tay bước #15 xem cài lại Game Bar có đem hai gói về không | Bỏ hai gói phụ khỏi mục, chỉ gỡ `XboxGamingOverlay` (Win+G vẫn mất; hai gói phụ còn lại vô hại) | — |

Chọn khác phương án A thì chỉ sửa `catalog.toml`, `catalog.vi.json` (Task 2) và bảng này; phương án C cho Teams thì thêm cả sửa luật `validate`.
