# WinFreeUp v0.1 «Dọn ổ đĩa» — Kế hoạch triển khai

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dựng một file `WinFreeUp.exe` (Tauri 2 + React) cho người dùng phổ thông: bấm **Quét** thấy dung lượng lấy lại được theo 9 nhóm, bấm **Dọn** thấy số GB thật đã lấy lại — không xóa gì trước khi người dùng đồng ý, và máy không hỏng gì.

**Architecture:** Crate Rust `winfreeup-core` (không phụ thuộc Tauri) chứa khuôn `Cleaner`, luật an toàn (gốc được phép + `canonicalize`, không đi theo reparse point, dry-run, dịch vụ luôn bật lại qua `Drop`), 9 module dọn, bộ điều phối quét/dọn và nhật ký; mọi thao tác chạm hệ thống đi qua trait `SystemOps` để test chạy trên cây thư mục giả. Vỏ Tauri 2 (`src-tauri`) chỉ nối lệnh/sự kiện và kiểm WebView2 trước khi tạo cửa sổ. Giao diện React + Fluent UI v9 là một máy trạng thái thuần (reducer) cộng một bộ điều khiển (controller) gọi API qua interface `Api` để test bằng Vitest.

**Tech Stack:** Rust 1.90 (edition 2021), Tauri 2.11, windows-sys 0.61, serde, thiserror 2, chrono; React 19.3, TypeScript 5.9, Fluent UI React v9 (9.74), Vite 8, Vitest 5 + jsdom + Testing Library; GitHub Actions `windows-latest`.

**Spec:** `docs/superpowers/specs/2026-09-25-winfreeup-don-o-dia-design.md`

## Global Constraints

Giá trị chép nguyên văn từ spec; mọi task ngầm bao gồm mục này.

- Hệ điều hành: «Windows 10 (1903 trở lên) và Windows 11, 64-bit.»
- Phát hành: «một file .exe chạy liền, không trình cài đặt» — «`tauri build` không bundler ⇒ một file `WinFreeUp.exe`».
- Ngôn ngữ: «Giao diện **tiếng Việt**; chuỗi hiển thị nằm trong file ngôn ngữ» — `src/i18n/vi.json`. «Lõi không chứa chữ hiển thị» (lõi chỉ trả mã/khoá và thông điệp lỗi kỹ thuật nguyên văn).
- «**An toàn trên hết**: không xóa gì trước khi người dùng thấy danh sách và bấm đồng ý.»
- «Không quảng cáo, không bản trả phí.»
- Lõi: «Không phụ thuộc Tauri, test độc lập bằng `cargo test`.» «Test không bao giờ đụng hệ thống thật.»
- Xóa: «cache và file tạm (chương trình tự tạo lại được) thì **xóa hẳn**. v0.1 **không đụng file của người dùng**.»
- `ScanResult`: «tổng số byte, số file, và **20 mục lớn nhất**». `CleanReport`: «số byte đã xóa, số file bỏ qua vì bị khóa, danh sách lỗi (thông điệp nguyên văn)».
- DISM: «`/Online /Cleanup-Image /StartComponentCleanup`» — «**Cấm** `/ResetBase`».
- `wu_download`: «Dừng dịch vụ `wuauserv`, xóa, **luôn** bật lại kể cả khi lỗi».
- `windows_old`: «phải gõ `XOA`».
- Điểm khôi phục: «trước khi dọn bất kỳ nhóm Cân nhắc/Rủi ro nào (trừ `recycle_bin`), tạo điểm khôi phục hệ thống. Không tạo được … ⇒ cảnh báo hổ phách, người dùng tự chọn dọn tiếp hay dừng.»
- Nhật ký: «`%LOCALAPPDATA%\WinFreeUp\logs\YYYY-MM-DD_HHmmss.log`».
- Lệnh Tauri: «`scan_all`, `cancel_scan`, `clean(ids, dry_run)`, `disk_free(drive)`, `open_log_folder`» (kế hoạch thêm `app_info` và `prepare_restore_point` — xem Task 17). Sự kiện: «`scan-progress`», «`clean-progress` (kèm % cho DISM)».
- Manifest «`requireAdministrator`». «`main.rs` kiểm tra WebView2 **trước khi** tạo cửa sổ. Không có ⇒ `MessageBoxW` tiếng Việt kèm link tải WebView2 của Microsoft.»
- Lỗi giao diện: «Lỗi chặn … băng đỏ», «Lỗi một phần … băng hổ phách», «Móc `window.addEventListener('error')` và `'unhandledrejection'`», «Gộp trùng, tối đa 5 dòng», «Mọi thao tác chờ lõi đều có vòng quay/chuyển động», «`prefers-reduced-motion`: đổi hiệu ứng quay thành nhịp mờ tỏ, không bỏ hẳn».
- Chế độ chạy thử: «`dry_run`, cờ dòng lệnh `--dry-run`».
- CI: «GitHub Actions `windows-latest` — `cargo test`, Vitest, `tauri build`».
- **Phiên bản đo trên máy dev (2026-09-25):** rustc 1.90.0, cargo 1.90.0, target `x86_64-pc-windows-msvc`, Visual Studio 2022 có sẵn; Node v22.14.0, npm 10.9.2 (không có pnpm — kế hoạch dùng npm); `tauri-cli` 2.11.4 (cargo) / `@tauri-apps/cli` 2.11.5 (npm); WebView2 Runtime 153.0.4234.48. Crate: tauri 2.11.6, tauri-build 2.6.3, windows-sys 0.61.2, serde 1.0.229, serde_json 1.0.151, thiserror 2.0.21, chrono 0.4.45, tempfile 3.27.0, filetime 0.2.29, junction 2.1.0. npm: react 19.3.0, @fluentui/react-components 9.74.9, @tauri-apps/api 2.11.1, vite 8.3.1, @vitejs/plugin-react 6.1.1, vitest 5.0.1, jsdom 30.1.1, @testing-library/react 16.3.3, @testing-library/dom 10.4.2, @testing-library/user-event 14.6.7, typescript 5.9.3 (cố ý **không** dùng TypeScript 7.x).
- **Không dùng Tauri 3** (crates.io đã có `3.0.0-alpha`): mọi `Cargo.toml` ghi `"2"`.
- Git: tiền tố `rtk` cho lệnh git; `git add` **từng tệp một** (không `-A`, không `.`); mọi commit kết thúc bằng dòng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; mỗi task commit trên nhánh `task/<số>-<tên-ngắn>` trong worktree riêng; không push, không merge vào `main` khi chưa hỏi người dùng.
- `cargo test` chỉ chạy `-p winfreeup-core`. Không chạy `cargo test` cho `src-tauri`: tauri-build nhúng manifest `requireAdministrator` vào cả file test ⇒ lỗi `os error 740` ở terminal thường. Vì cùng lý do, `npm run tauri dev` phải chạy trong terminal **Run as administrator**.
- Luật chung của người dùng không áp dụng cho v0.1 (ghi để người rà khỏi hỏi): bộ lọc trong URL, đánh dấu T7/CN, tên người/ID — ứng dụng không có bộ lọc, trục ngày hay tên người.

## Review Focus

1. Người dùng gõ xác nhận nhóm Rủi ro theo thói quen tiếng Việt — `xóa`, ` Xóa `, `XÓA`, `xoa` — thì phải được chấp nhận như `XOA`; `XO`, `XOAA`, chuỗi rỗng thì không. (Test: Task 11 `isConfirmWord`, Task 14 `ConfirmDialog`.)
2. File mang thuộc tính chỉ-đọc (read-only) trong `%TEMP%` (rất hay gặp với bộ cài) phải xóa được — bỏ cờ chỉ-đọc rồi xóa — chứ không bị đếm là «bị khóa». (Test: Task 2.)
3. File biến mất giữa lúc quét và lúc dọn (chương trình tự dọn) không được tính là lỗi và không sinh băng hổ phách. (Test: Task 2 `Vanished`, Task 3 `vanished_file_between_scan_and_clean_is_not_an_error`.)
4. Thư mục gốc không tồn tại (không cài Firefox/Cốc Cốc, không có `Windows.old`, `Minidump`, `MEMORY.DMP`) ⇒ nhóm hiện 0 byte, không lỗi, và không gọi `take_ownership`. (Test: Task 3, Task 5, Task 6.)
5. Dung lượng trống sau khi dọn nhỏ hơn trước (ứng dụng khác ghi đĩa cùng lúc) ⇒ màn Kết quả hiện «0 MB», không bao giờ số âm. (Test: Task 11 `reclaimedBytes`, Task 15 `ResultView`.)

---

## File Structure

```
WinFreeUp/
├─ Cargo.toml                         workspace (Task 1; Task 17 thêm member src-tauri)
├─ Cargo.lock                         (Task 1; Task 17 cập nhật khi thêm tauri)
├─ .gitignore                         thêm target/, node_modules/, dist/ (Task 1)
├─ package.json, package-lock.json    ĐỦ mọi phụ thuộc ngay từ đầu (Task 10)
├─ vite.config.ts, tsconfig.json      (Task 10)
├─ index.html                         (Task 16)
├─ crates/winfreeup-core/
│  ├─ Cargo.toml                      ĐỦ mọi phụ thuộc ngay từ đầu (Task 1)
│  └─ src/
│     ├─ lib.rs                       khai báo SẴN mọi module (Task 1) — task sau không sửa
│     ├─ types.rs                     RiskLevel, Item, ScanResult, CleanReport, CleanOptions, CancelToken, ItemAction, Progress, NoProgress, Cleaner (Task 1)
│     ├─ error.rs                     CoreError, Result, io_err (Task 1)
│     ├─ env.rs                       Env, SystemOps (Task 1)
│     ├─ testutil.rs                  #[cfg(test)] FakeSys, Recorder, FakeCleaner, fake_env, write_file, age (Task 1)
│     ├─ safety.rs                    khung (Task 1) → Guard, delete_path, remove_empty_dir (Task 2)
│     ├─ fsclean.rs                   khung (Task 1) → Filter, Target, walk, scan_targets, clean_targets, FileCleaner, DAY (Task 3)
│     ├─ service.rs                   khung (Task 1) → ServiceGuard (Task 6)
│     ├─ log.rs                       khung (Task 1) → CleanLog, log_dir, log_file_name (Task 8)
│     ├─ engine.rs                    khung (Task 1) → GroupScan, GroupClean, CleanEvent, CleanSummary, RestorePointStatus, scan_all, needs_restore_point, prepare_restore_point, run_clean (Task 8)
│     ├─ sys_windows.rs               khung (Task 1) → RealSystem, Env::from_system, disk_free, split_lines (Task 9)
│     └─ cleaners/
│        ├─ mod.rs                    khai báo SẴN 10 module con (Task 1) — task sau không sửa
│        ├─ user_temp.rs, system_temp.rs, win_caches.rs, delivery_opt.rs   khung (Task 1) → Task 4
│        ├─ browser_cache.rs          khung (Task 1) → Task 5
│        ├─ wu_download.rs, windows_old.rs   khung (Task 1) → Task 6
│        ├─ component_store.rs, recycle_bin.rs   khung (Task 1) → Task 7
│        └─ registry.rs               khung (Task 1) → ALL_IDS, all_cleaners (Task 17)
├─ src/                               giao diện
│  ├─ i18n/vi.json                    ĐỦ mọi chuỗi của v0.1 (Task 10) — task sau không sửa
│  ├─ i18n/index.ts, i18n/i18n.test.ts (Task 10)
│  ├─ catalog.ts, format.ts, format.test.ts (Task 10)
│  ├─ api/types.ts                    kiểu dữ liệu trao đổi với lõi (Task 10)
│  ├─ test/setup.ts                   (Task 10)
│  ├─ state/machine.ts, state/selectors.ts, state/machine.test.ts (Task 11)
│  ├─ errors/errors.ts, errors/errors.test.ts   (Task 12)
│  ├─ styles.css                      ĐỦ mọi lớp CSS của v0.1 (Task 12)
│  ├─ components/Busy.tsx, NoticeBar.tsx, ErrorBoundary.tsx, components.test.tsx (Task 12)
│  ├─ state/store.ts, state/controller.ts, state/controller.test.ts, api/tauri.ts, api/tauri.test.ts (Task 13)
│  ├─ components/WelcomeView.tsx, ScanningView.tsx, PreviewView.tsx, ConfirmDialog.tsx, views1.test.tsx (Task 14)
│  ├─ components/RestorePointView.tsx, CleaningView.tsx, ResultView.tsx, views2.test.tsx (Task 15)
│  └─ main.tsx, App.tsx, App.test.tsx (Task 16)
├─ src-tauri/                         (Task 17)
│  ├─ Cargo.toml, build.rs, app.manifest, tauri.conf.json, capabilities/default.json
│  ├─ app-icon.png, icons/icon.ico
│  └─ src/main.rs, src/commands.rs, src/webview_check.rs
├─ .github/workflows/ci.yml           (Task 18)
└─ docs/thu-tay-v0.1.md               danh sách thử tay trong Windows Sandbox/VM (Task 18)
```

Nguyên tắc chia: mỗi module dọn một file; mọi lời gọi chạm hệ thống thật (dịch vụ, DISM, Thùng rác, điểm khôi phục, quyền sở hữu, tiến trình) chỉ nằm ở `sys_windows.rs`, sau trait `SystemOps`. Giao diện tách logic thuần (`state/`, `errors/`, `format.ts`) khỏi component để Vitest thử được không cần Tauri.

Nguyên tắc chống đụng file khi thi công song song: những file mà nhiều task cùng cần (`lib.rs`, `cleaners/mod.rs`, hai `Cargo.toml`, `package.json`, `vi.json`, `styles.css`, `testutil.rs`, `api/types.ts`) được dựng **đầy đủ ở Đợt 0 hoặc trong đúng một task**; các task sau chỉ **đọc** chúng. Module chưa tới lượt được tạo ở Task 1 dưới dạng **file khung** chỉ có một dòng chú thích `//!`, để `lib.rs` biên dịch được ngay; task chủ sở hữu viết nội dung vào đúng file đó (thay toàn bộ dòng khung).

## Đợt thi công

Theo `CLAUDE.md` mục «Quy trình thi công»: mỗi task một worktree + nhánh `task/<số>-<tên-ngắn>` tách từ `feat/v0.1-don-o-dia`; hết mỗi đợt gộp vào nhánh tính năng rồi chạy **toàn bộ** `cargo test -p winfreeup-core` + `npm test`; mỗi task một lượt rà riêng trước khi gộp.

| Đợt | Task (song song trong đợt) | Chờ task nào | File dùng chung — ai sở hữu |
|---|---|---|---|
| 0 | **Task 1** rồi **Task 10** — một đội, tuần tự | — | Task 1: `Cargo.toml`, `Cargo.lock`, `lib.rs`, `cleaners/mod.rs`, `testutil.rs`, mọi file khung lõi. Task 10: `package.json`, `package-lock.json`, `vi.json`, `api/types.ts`, `catalog.ts` |
| 1 | Task 2 (safety) · Task 7 (component_store, recycle_bin) · Task 8 (log, engine) · Task 9 (sys_windows) · Task 11 (machine, selectors) · Task 12 (errors, Busy, NoticeBar, ErrorBoundary, styles.css) | Đợt 0 | mỗi task chỉ sửa file riêng |
| 2 | Task 3 (fsclean) · Task 13 (store, controller, api/tauri) · Task 14 (Welcome, Scanning, Preview, Confirm) · Task 15 (RestorePoint, Cleaning, Result) | Task 3 ← 2; Task 13, 14, 15 ← 11, 12 | không chung file |
| 3 | Task 4 (4 nhóm file) · Task 5 (browser_cache) · Task 6 (service, wu_download, windows_old) · Task 16 (App, main, index.html) | Task 4, 5, 6 ← 3; Task 16 ← 13, 14, 15 | không chung file |
| 4 | Task 17 (vỏ Tauri + `cleaners/registry.rs`) | mọi task lõi + Task 16 | task duy nhất sửa `Cargo.toml` gốc và `Cargo.lock` sau Đợt 0 |
| 5 | Task 18 (CI + danh sách thử tay) | Task 17 | — |

Đường găng: Task 1 → 10 → 11/12 → 13/14/15 → 16 → 17 → 18. Nhánh lõi (1 → 2 → 3 → 4/5/6) chạy song song và xong trước Đợt 4.

Vì test đếm theo bộ lọc, mỗi task báo số test **của riêng nó** (vd `cargo test -p winfreeup-core fsclean` → 11 passed) và yêu cầu toàn bộ bộ test «0 failed»; tổng số test toàn dự án chỉ kiểm ở bước gộp đợt.

## Chuẩn bị (một lần, trước Đợt 0)

- [ ] Tạo nhánh tính năng từ `main` (kế hoạch này nằm sẵn trên `main`):

```bash
cd D:/META/WinFreeUp
rtk git checkout main
rtk git checkout -b feat/v0.1-don-o-dia
```

Expected: `Switched to a new branch 'feat/v0.1-don-o-dia'`.

- [ ] Mỗi task: dựng worktree riêng bằng skill `superpowers:using-git-worktrees`, nhánh `task/<số>-<tên-ngắn>` tách từ `feat/v0.1-don-o-dia` (vd `task/02-safety`). Mọi lệnh trong task chạy ở thư mục worktree đó. Từ Đợt 1, worktree mới cần `npm ci` một lần trước khi chạy Vitest.

---
### Task 1: Workspace Rust và các kiểu lõi

**Files:**
- Create: `Cargo.toml`
- Modify: `.gitignore`
- Create: `crates/winfreeup-core/Cargo.toml`
- Create: `crates/winfreeup-core/src/lib.rs`
- Create: `crates/winfreeup-core/src/types.rs`
- Create: `crates/winfreeup-core/src/error.rs`
- Create: `crates/winfreeup-core/src/env.rs`
- Create: `crates/winfreeup-core/src/testutil.rs`
- Create (file khung, một dòng `//!`): `crates/winfreeup-core/src/{safety,fsclean,service,log,engine,sys_windows}.rs`
- Create: `crates/winfreeup-core/src/cleaners/mod.rs`
- Create (file khung): `crates/winfreeup-core/src/cleaners/{user_temp,system_temp,win_caches,delivery_opt,browser_cache,wu_download,windows_old,component_store,recycle_bin,registry}.rs`
- Test: `crates/winfreeup-core/src/types.rs` (module `tests`)

**Interfaces:**
- Consumes: không có.
- Produces (mọi task lõi sau dùng nguyên văn các tên này):
  - `pub enum RiskLevel { Safe, Caution, Risky }` — serde `"safe" | "caution" | "risky"`.
  - `pub const TOP_ITEMS: usize = 20;`
  - `pub struct Item { pub path: String, pub bytes: u64 }`
  - `pub struct ScanResult { pub total_bytes: u64, pub file_count: u64, pub top_items: Vec<Item>, pub estimated: bool, pub notices: Vec<String> }` + `fn add_file(&mut self, path: &Path, bytes: u64)`
  - `pub struct CleanReport { pub bytes_freed: u64, pub files_deleted: u64, pub skipped_locked: u64, pub errors: Vec<String>, pub dry_run: bool }`
  - `pub struct CleanOptions { pub dry_run: bool }`
  - `pub struct CancelToken` + `new()`, `cancel(&self)`, `is_cancelled(&self) -> bool` (clone chia sẻ trạng thái)
  - `pub enum ItemAction { Deleted, WouldDelete, SkippedLocked, Failed }` + `fn as_str(self) -> &'static str`
  - `pub trait Progress: Send + Sync { fn item(&self, action: ItemAction, path: &Path, bytes: u64, detail: Option<&str>); fn percent(&self, pct: f32); }`, `pub struct NoProgress;`
  - `pub trait Cleaner: Send + Sync` đúng chữ ký spec: `id`, `risk`, `default_selected`, `allowed_roots(&self, env: &Env) -> Vec<PathBuf>`, `scan(&self, env: &Env, cancel: &CancelToken) -> Result<ScanResult>`, `clean(&self, env: &Env, scan: &ScanResult, opts: &CleanOptions, progress: &dyn Progress) -> Result<CleanReport>`
  - `pub enum CoreError { Io { path, source }, OutsideRoots(PathBuf), Cancelled, System(String) }`, `pub type Result<T>`, `pub fn io_err(path: &Path, source: std::io::Error) -> CoreError`
  - `pub trait SystemOps: Send + Sync` (8 hàm, xem code) và `#[derive(Clone)] pub struct Env { temp, windir, local_appdata, program_data, system_drive: PathBuf, now: SystemTime, sys: Arc<dyn SystemOps> }`
  - `#[cfg(test)]` trong `crate::testutil`:
    - `FakeSys { calls: Mutex<Vec<String>>, running: Vec<String>, service_was_running: bool, start_fails: bool, dism_output: String, dism_fails: bool, recycle: (u64, u64), restore_error: Option<String> }` (`Default`) + `calls(&self) -> Vec<String>`. Chuỗi ghi lại: `"stop:<name>"`, `"start:<name>"`, `"dism:<args nối bằng dấu cách>"`, `"rb_size"`, `"rb_empty"`, `"restore"`, `"own:<path.display()>"`. `run_dism` gọi `on_line` cho từng dòng của `dism_output` (tách `\r`/`\n`, bỏ dòng rỗng).
    - `fake_env(root: &Path, sys: Arc<FakeSys>) -> Env` — `temp = root\Temp`, `windir = root\Windows`, `local_appdata = root\Local`, `program_data = root\ProgramData`, `system_drive = root`, `now = SystemTime::now()`; tạo sẵn bốn thư mục.
    - `write_file(path: &Path, len: usize) -> PathBuf` (tạo cả thư mục cha), `age(path: &Path, hours: u64)` (lùi mtime).
    - `Recorder { items: Mutex<Vec<(ItemAction, PathBuf, u64)>>, percents: Mutex<Vec<f32>> }` (`Default`, `impl Progress`) + `actions(&self) -> Vec<ItemAction>`.
    - `FakeCleaner { id: &'static str, risk: RiskLevel, bytes: u64, fail: Option<&'static str>, panics: bool }` (`impl Cleaner`; `default_selected = risk == Safe`; scan trả `total_bytes = bytes, file_count = 1`, trả `Err(Cancelled)` nếu token đã hủy; clean gọi `progress.item(Deleted, Path::new(id), bytes, None)` rồi `progress.percent(50.0)`) + `FakeCleaner::ok(id, risk, bytes) -> Box<dyn Cleaner>`.
  - `lib.rs` khai báo sẵn: `cleaners, engine, env, error, fsclean, log, safety, service, sys_windows, types` và `#[cfg(test)] testutil`; `cleaners/mod.rs` khai báo sẵn 10 module con. Không task nào sau này sửa hai file này.

- [ ] **Step 1: Tạo workspace và crate**

`Cargo.toml` (gốc repo):

```toml
[workspace]
resolver = "2"
members = ["crates/winfreeup-core"]

[profile.release]
opt-level = "s"
lto = true
codegen-units = 1
strip = true
```

Thêm vào cuối `.gitignore` (giữ dòng `.omc/` đang có):

```
target/
node_modules/
dist/
```

`crates/winfreeup-core/Cargo.toml`:

```toml
[package]
name = "winfreeup-core"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
thiserror = "2"
chrono = { version = "0.4", default-features = false, features = ["clock", "std"] }

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.61", features = [
  "Win32_Foundation",
  "Win32_Storage_FileSystem",
  "Win32_System_Diagnostics_ToolHelp",
  "Win32_UI_Shell",
] }

[dev-dependencies]
tempfile = "3"
filetime = "0.2"
junction = "2"
serde_json = "1"
```

`crates/winfreeup-core/src/lib.rs` (khai báo sẵn MỌI module — các task sau không sửa file này):

```rust
//! Lõi WinFreeUp: quét, dọn, luật an toàn, nhật ký. Không phụ thuộc Tauri, không chứa chữ hiển thị.

pub mod cleaners;
pub mod engine;
pub mod env;
pub mod error;
pub mod fsclean;
pub mod log;
pub mod safety;
pub mod service;
pub mod sys_windows;
pub mod types;

#[cfg(test)]
pub(crate) mod testutil;

pub use env::{Env, SystemOps};
pub use error::{CoreError, Result};
pub use types::*;
```

`crates/winfreeup-core/src/cleaners/mod.rs` (khai báo sẵn — các task sau không sửa file này):

```rust
//! Mỗi nhóm dọn một file. `registry` giữ danh sách đầy đủ theo thứ tự bảng spec mục 3.
pub mod browser_cache;
pub mod component_store;
pub mod delivery_opt;
pub mod recycle_bin;
pub mod registry;
pub mod system_temp;
pub mod user_temp;
pub mod win_caches;
pub mod windows_old;
pub mod wu_download;
```

File khung — mỗi file đúng MỘT dòng, task chủ sở hữu sẽ thay bằng nội dung thật:

| File | Nội dung dòng khung |
|---|---|
| `src/safety.rs` | `//! Luật an toàn — nội dung ở Task 2.` |
| `src/fsclean.rs` | `//! Duyệt và dọn theo Target — nội dung ở Task 3.` |
| `src/service.rs` | `//! ServiceGuard — nội dung ở Task 6.` |
| `src/log.rs` | `//! Nhật ký lượt dọn — nội dung ở Task 8.` |
| `src/engine.rs` | `//! Điều phối quét/dọn — nội dung ở Task 8.` |
| `src/sys_windows.rs` | `//! RealSystem — nội dung ở Task 9.` |
| `src/cleaners/user_temp.rs`, `system_temp.rs`, `win_caches.rs`, `delivery_opt.rs` | `//! Nhóm dọn — nội dung ở Task 4.` |
| `src/cleaners/browser_cache.rs` | `//! Nhóm dọn — nội dung ở Task 5.` |
| `src/cleaners/wu_download.rs`, `windows_old.rs` | `//! Nhóm dọn — nội dung ở Task 6.` |
| `src/cleaners/component_store.rs`, `recycle_bin.rs` | `//! Nhóm dọn — nội dung ở Task 7.` |
| `src/cleaners/registry.rs` | `//! Danh sách nhóm — nội dung ở Task 17.` |

(Các đường dẫn trong bảng tính từ `crates/winfreeup-core/`.)

- [ ] **Step 2: Viết `error.rs` và `env.rs`**

`crates/winfreeup-core/src/error.rs`:

```rust
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{}: {}", .path.display(), .source)]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("path outside allowed roots: {}", .0.display())]
    OutsideRoots(PathBuf),
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    System(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

pub fn io_err(path: &Path, source: std::io::Error) -> CoreError {
    CoreError::Io { path: path.to_path_buf(), source }
}
```

`crates/winfreeup-core/src/env.rs`:

```rust
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use crate::error::Result;
use crate::types::CancelToken;

/// Mọi thao tác chạm hệ thống thật. Bản thật ở `sys_windows::RealSystem`, bản giả ở `testutil::FakeSys`.
pub trait SystemOps: Send + Sync {
    fn is_process_running(&self, exe_name: &str) -> bool;
    /// Ok(true) = dịch vụ đang chạy và đã dừng; Ok(false) = vốn đã dừng.
    fn stop_service(&self, name: &str) -> Result<bool>;
    fn start_service(&self, name: &str) -> Result<()>;
    /// Gọi `on_line` cho từng dòng đầu ra (tách theo `\r` hoặc `\n`, bỏ dòng rỗng). Lỗi nếu mã thoát khác 0/3010.
    fn run_dism(&self, args: &[&str], cancel: &CancelToken, on_line: &mut dyn FnMut(&str)) -> Result<()>;
    /// (tổng byte, số mục) của Thùng rác mọi ổ.
    fn recycle_bin_size(&self) -> Result<(u64, u64)>;
    fn empty_recycle_bin(&self) -> Result<()>;
    fn create_restore_point(&self, description: &str) -> Result<()>;
    fn take_ownership(&self, path: &Path) -> Result<()>;
}

#[derive(Clone)]
pub struct Env {
    pub temp: PathBuf,
    pub windir: PathBuf,
    pub local_appdata: PathBuf,
    pub program_data: PathBuf,
    /// Dạng `C:\` (có dấu gạch chéo cuối).
    pub system_drive: PathBuf,
    pub now: SystemTime,
    pub sys: Arc<dyn SystemOps>,
}
```

- [ ] **Step 3: Viết test hỏng cho `types.rs`**

Tạo `crates/winfreeup-core/src/types.rs` chỉ với phần test (phần mã viết ở Step 5):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn scan_result_keeps_twenty_largest_sorted() {
        let mut r = ScanResult::default();
        for i in 1..=30u64 {
            r.add_file(Path::new(&format!("C:/t/{i}.tmp")), i * 10);
        }
        assert_eq!(r.file_count, 30);
        assert_eq!(r.total_bytes, (1..=30u64).map(|i| i * 10).sum::<u64>());
        assert_eq!(r.top_items.len(), TOP_ITEMS);
        assert_eq!(r.top_items[0].bytes, 300);
        assert_eq!(r.top_items[19].bytes, 110);
        assert!(r.top_items.windows(2).all(|w| w[0].bytes >= w[1].bytes));
    }

    #[test]
    fn zero_byte_entries_are_counted_but_not_listed() {
        let mut r = ScanResult::default();
        r.add_file(Path::new("C:/t/link"), 0);
        assert_eq!(r.file_count, 1);
        assert!(r.top_items.is_empty());
    }

    #[test]
    fn risk_level_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&RiskLevel::Caution).unwrap(), "\"caution\"");
        assert_eq!(serde_json::to_string(&RiskLevel::Risky).unwrap(), "\"risky\"");
    }

    #[test]
    fn cancel_token_clones_share_state() {
        let a = CancelToken::new();
        let b = a.clone();
        assert!(!b.is_cancelled());
        a.cancel();
        assert!(b.is_cancelled());
    }

    #[test]
    fn item_action_labels_are_stable() {
        assert_eq!(ItemAction::Deleted.as_str(), "DELETED");
        assert_eq!(ItemAction::WouldDelete.as_str(), "WOULD_DELETE");
        assert_eq!(ItemAction::SkippedLocked.as_str(), "SKIPPED_LOCKED");
        assert_eq!(ItemAction::Failed.as_str(), "FAILED");
    }
}
```

- [ ] **Step 4: Chạy test, thấy hỏng**

Run: `cargo test -p winfreeup-core`
Expected: FAIL biên dịch — `cannot find type ScanResult in this scope` (và `testutil` chưa có file).

- [ ] **Step 5: Viết mã `types.rs` (đặt TRÊN khối `#[cfg(test)]`)**

```rust
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::env::Env;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Safe,
    Caution,
    Risky,
}

pub const TOP_ITEMS: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ScanResult {
    pub total_bytes: u64,
    pub file_count: u64,
    pub top_items: Vec<Item>,
    /// true khi số byte là ước tính (DISM).
    pub estimated: bool,
    /// Mã thông báo cho giao diện dịch, vd "browser_running:chrome".
    pub notices: Vec<String>,
}

impl ScanResult {
    pub fn add_file(&mut self, path: &Path, bytes: u64) {
        self.total_bytes += bytes;
        self.file_count += 1;
        if bytes == 0 {
            return;
        }
        let pos = self.top_items.partition_point(|i| i.bytes >= bytes);
        if pos >= TOP_ITEMS {
            return;
        }
        self.top_items.insert(pos, Item { path: path.display().to_string(), bytes });
        self.top_items.truncate(TOP_ITEMS);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CleanReport {
    pub bytes_freed: u64,
    pub files_deleted: u64,
    pub skipped_locked: u64,
    pub errors: Vec<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CleanOptions {
    pub dry_run: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemAction {
    Deleted,
    WouldDelete,
    SkippedLocked,
    Failed,
}

impl ItemAction {
    pub fn as_str(self) -> &'static str {
        match self {
            ItemAction::Deleted => "DELETED",
            ItemAction::WouldDelete => "WOULD_DELETE",
            ItemAction::SkippedLocked => "SKIPPED_LOCKED",
            ItemAction::Failed => "FAILED",
        }
    }
}

pub trait Progress: Send + Sync {
    fn item(&self, action: ItemAction, path: &Path, bytes: u64, detail: Option<&str>);
    fn percent(&self, pct: f32);
}

pub struct NoProgress;

impl Progress for NoProgress {
    fn item(&self, _: ItemAction, _: &Path, _: u64, _: Option<&str>) {}
    fn percent(&self, _: f32) {}
}

pub trait Cleaner: Send + Sync {
    fn id(&self) -> &'static str;
    fn risk(&self) -> RiskLevel;
    fn default_selected(&self) -> bool;
    fn allowed_roots(&self, env: &Env) -> Vec<PathBuf>;
    fn scan(&self, env: &Env, cancel: &CancelToken) -> Result<ScanResult>;
    fn clean(
        &self,
        env: &Env,
        scan: &ScanResult,
        opts: &CleanOptions,
        progress: &dyn Progress,
    ) -> Result<CleanReport>;
}
```

- [ ] **Step 6: Viết `testutil.rs` (dùng từ Task 2 trở đi)**

`crates/winfreeup-core/src/testutil.rs`:

```rust
//! Công cụ test: hệ thống giả và cây thư mục giả. Không bao giờ chạm hệ thống thật.
#![allow(dead_code)]
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crate::env::{Env, SystemOps};
use crate::error::{CoreError, Result};
use crate::types::{CancelToken, CleanOptions, CleanReport, Cleaner, ItemAction, Progress, RiskLevel, ScanResult};

#[derive(Default)]
pub struct FakeSys {
    pub calls: Mutex<Vec<String>>,
    pub running: Vec<String>,
    pub service_was_running: bool,
    pub start_fails: bool,
    pub dism_output: String,
    pub dism_fails: bool,
    pub recycle: (u64, u64),
    pub restore_error: Option<String>,
}

impl FakeSys {
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    fn record(&self, s: String) {
        self.calls.lock().unwrap().push(s);
    }
}

impl SystemOps for FakeSys {
    fn is_process_running(&self, exe_name: &str) -> bool {
        self.running.iter().any(|r| r.eq_ignore_ascii_case(exe_name))
    }
    fn stop_service(&self, name: &str) -> Result<bool> {
        self.record(format!("stop:{name}"));
        Ok(self.service_was_running)
    }
    fn start_service(&self, name: &str) -> Result<()> {
        self.record(format!("start:{name}"));
        if self.start_fails {
            Err(CoreError::System("start failed".into()))
        } else {
            Ok(())
        }
    }
    fn run_dism(&self, args: &[&str], cancel: &CancelToken, on_line: &mut dyn FnMut(&str)) -> Result<()> {
        self.record(format!("dism:{}", args.join(" ")));
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        if self.dism_fails {
            return Err(CoreError::System("dism failed".into()));
        }
        for line in self.dism_output.split(['\r', '\n']).map(str::trim).filter(|l| !l.is_empty()) {
            on_line(line);
        }
        Ok(())
    }
    fn recycle_bin_size(&self) -> Result<(u64, u64)> {
        self.record("rb_size".into());
        Ok(self.recycle)
    }
    fn empty_recycle_bin(&self) -> Result<()> {
        self.record("rb_empty".into());
        Ok(())
    }
    fn create_restore_point(&self, _description: &str) -> Result<()> {
        self.record("restore".into());
        match &self.restore_error {
            Some(m) => Err(CoreError::System(m.clone())),
            None => Ok(()),
        }
    }
    fn take_ownership(&self, path: &Path) -> Result<()> {
        self.record(format!("own:{}", path.display()));
        Ok(())
    }
}

/// Env trỏ vào cây giả: root/Temp, root/Windows, root/Local, root/ProgramData, ổ hệ thống = root.
pub fn fake_env(root: &Path, sys: Arc<FakeSys>) -> Env {
    let env = Env {
        temp: root.join("Temp"),
        windir: root.join("Windows"),
        local_appdata: root.join("Local"),
        program_data: root.join("ProgramData"),
        system_drive: root.to_path_buf(),
        now: SystemTime::now(),
        sys,
    };
    for d in [&env.temp, &env.windir, &env.local_appdata, &env.program_data] {
        fs::create_dir_all(d).unwrap();
    }
    env
}

pub fn write_file(path: &Path, len: usize) -> PathBuf {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![b'x'; len]).unwrap();
    path.to_path_buf()
}

/// Đặt giờ sửa đổi của file/thư mục lùi về `hours` giờ trước.
pub fn age(path: &Path, hours: u64) {
    let t = SystemTime::now() - Duration::from_secs(hours * 3600);
    filetime::set_file_mtime(path, filetime::FileTime::from_system_time(t)).unwrap();
}

#[derive(Default)]
pub struct Recorder {
    pub items: Mutex<Vec<(ItemAction, PathBuf, u64)>>,
    pub percents: Mutex<Vec<f32>>,
}

impl Recorder {
    pub fn actions(&self) -> Vec<ItemAction> {
        self.items.lock().unwrap().iter().map(|i| i.0).collect()
    }
}

impl Progress for Recorder {
    fn item(&self, action: ItemAction, path: &Path, bytes: u64, _detail: Option<&str>) {
        self.items.lock().unwrap().push((action, path.to_path_buf(), bytes));
    }
    fn percent(&self, pct: f32) {
        self.percents.lock().unwrap().push(pct);
    }
}

pub struct FakeCleaner {
    pub id: &'static str,
    pub risk: RiskLevel,
    pub bytes: u64,
    pub fail: Option<&'static str>,
    pub panics: bool,
}

impl FakeCleaner {
    pub fn ok(id: &'static str, risk: RiskLevel, bytes: u64) -> Box<dyn Cleaner> {
        Box::new(FakeCleaner { id, risk, bytes, fail: None, panics: false })
    }
}

impl Cleaner for FakeCleaner {
    fn id(&self) -> &'static str {
        self.id
    }
    fn risk(&self) -> RiskLevel {
        self.risk
    }
    fn default_selected(&self) -> bool {
        self.risk == RiskLevel::Safe
    }
    fn allowed_roots(&self, _env: &Env) -> Vec<PathBuf> {
        vec![]
    }
    fn scan(&self, _env: &Env, cancel: &CancelToken) -> Result<ScanResult> {
        if self.panics {
            panic!("boom in {}", self.id);
        }
        if let Some(m) = self.fail {
            return Err(CoreError::System(m.into()));
        }
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        Ok(ScanResult { total_bytes: self.bytes, file_count: 1, ..Default::default() })
    }
    fn clean(&self, _env: &Env, _scan: &ScanResult, opts: &CleanOptions, progress: &dyn Progress) -> Result<CleanReport> {
        if self.panics {
            panic!("boom in {}", self.id);
        }
        if let Some(m) = self.fail {
            return Err(CoreError::System(m.into()));
        }
        progress.item(ItemAction::Deleted, Path::new(self.id), self.bytes, None);
        progress.percent(50.0);
        Ok(CleanReport { bytes_freed: self.bytes, files_deleted: 1, dry_run: opts.dry_run, ..Default::default() })
    }
}
```

- [ ] **Step 7: Chạy test, thấy qua**

Run: `cargo test -p winfreeup-core`
Expected: `test result: ok. 5 passed; 0 failed`.

- [ ] **Step 8: Commit**

```bash
rtk git add Cargo.toml
rtk git add .gitignore
rtk git add Cargo.lock
rtk git add crates/winfreeup-core/Cargo.toml
rtk git add crates/winfreeup-core/src/lib.rs
rtk git add crates/winfreeup-core/src/types.rs
rtk git add crates/winfreeup-core/src/error.rs
rtk git add crates/winfreeup-core/src/env.rs
rtk git add crates/winfreeup-core/src/testutil.rs
rtk git add crates/winfreeup-core/src/safety.rs
rtk git add crates/winfreeup-core/src/fsclean.rs
rtk git add crates/winfreeup-core/src/service.rs
rtk git add crates/winfreeup-core/src/log.rs
rtk git add crates/winfreeup-core/src/engine.rs
rtk git add crates/winfreeup-core/src/sys_windows.rs
rtk git add crates/winfreeup-core/src/cleaners/mod.rs
rtk git add crates/winfreeup-core/src/cleaners/user_temp.rs
rtk git add crates/winfreeup-core/src/cleaners/system_temp.rs
rtk git add crates/winfreeup-core/src/cleaners/win_caches.rs
rtk git add crates/winfreeup-core/src/cleaners/delivery_opt.rs
rtk git add crates/winfreeup-core/src/cleaners/browser_cache.rs
rtk git add crates/winfreeup-core/src/cleaners/wu_download.rs
rtk git add crates/winfreeup-core/src/cleaners/windows_old.rs
rtk git add crates/winfreeup-core/src/cleaners/component_store.rs
rtk git add crates/winfreeup-core/src/cleaners/recycle_bin.rs
rtk git add crates/winfreeup-core/src/cleaners/registry.rs
rtk git commit -m "feat(core): khung workspace, kiểu lõi Cleaner/ScanResult/CleanReport/Env/SystemOps, công cụ test

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Luật an toàn — gốc được phép, reparse point, file khóa

**Files:**
- Modify: `crates/winfreeup-core/src/safety.rs` (thay dòng khung từ Task 1)
- Test: `crates/winfreeup-core/src/safety.rs` (module `tests`)

**Interfaces:**
- Consumes (Task 1): `CoreError::{Io, OutsideRoots, System}`, `io_err(path: &Path, source: std::io::Error) -> CoreError`, `Result<T>`; `testutil::write_file(path: &Path, len: usize) -> PathBuf`. Crate dev `junction = "2"` (`junction::create(target, link)`), `tempfile = "3"` đã khai báo ở Task 1.
- Produces:
  - `pub fn is_reparse_point(meta: &std::fs::Metadata) -> bool`
  - `pub struct Guard` + `Guard::new(roots: &[PathBuf]) -> Guard`, `is_allowed(&self, path: &Path) -> bool`, `check(&self, path: &Path) -> Result<()>`
  - `pub enum DeleteOutcome { Deleted(u64), WouldDelete(u64), Locked, Vanished }`
  - `pub fn delete_path(guard: &Guard, path: &Path, dry_run: bool) -> Result<DeleteOutcome>` — chỉ xóa file hoặc bản thân liên kết; từ chối thư mục thường.
  - `pub fn remove_empty_dir(guard: &Guard, dir: &Path) -> bool`

- [ ] **Step 1: Viết test hỏng**

Thay dòng khung của `crates/winfreeup-core/src/safety.rs` bằng phần test dưới đây (mã viết ở Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::write_file;
    use std::fs;
    use std::os::windows::fs::OpenOptionsExt;

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        let outside = tmp.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        (tmp, root, outside)
    }

    #[test]
    fn deleting_outside_allowed_roots_is_refused() {
        let (_t, root, outside) = setup();
        let secret = write_file(&outside.join("secret.txt"), 5);
        let guard = Guard::new(std::slice::from_ref(&root));
        for dry in [false, true] {
            let err = delete_path(&guard, &secret, dry).unwrap_err();
            assert!(matches!(err, CoreError::OutsideRoots(_)), "{err}");
        }
        assert!(secret.exists());
    }

    #[test]
    fn dotdot_escape_is_refused() {
        let (_t, root, outside) = setup();
        let secret = write_file(&outside.join("secret.txt"), 5);
        let sneaky = root.join("..").join("outside").join("secret.txt");
        let guard = Guard::new(&[root]);
        assert!(matches!(delete_path(&guard, &sneaky, false), Err(CoreError::OutsideRoots(_))));
        assert!(secret.exists());
    }

    #[test]
    fn file_inside_root_is_deleted_and_bytes_reported() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("a").join("x.tmp"), 7);
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &f, false).unwrap(), DeleteOutcome::Deleted(7));
        assert!(!f.exists());
    }

    #[test]
    fn dry_run_keeps_the_file() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("x.tmp"), 5);
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &f, true).unwrap(), DeleteOutcome::WouldDelete(5));
        assert!(f.exists());
    }

    #[test]
    fn locked_file_is_skipped_not_failed() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("busy.tmp"), 5);
        let _handle = fs::OpenOptions::new().read(true).share_mode(0).open(&f).unwrap();
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &f, false).unwrap(), DeleteOutcome::Locked);
        assert!(f.exists());
    }

    // Review Focus 2
    #[test]
    fn readonly_file_is_deleted_not_counted_as_locked() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("ro.tmp"), 4);
        let mut p = fs::metadata(&f).unwrap().permissions();
        p.set_readonly(true);
        fs::set_permissions(&f, p).unwrap();
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &f, false).unwrap(), DeleteOutcome::Deleted(4));
        assert!(!f.exists());
    }

    // Review Focus 3
    #[test]
    fn vanished_file_is_reported_as_vanished() {
        let (_t, root, _o) = setup();
        let guard = Guard::new(std::slice::from_ref(&root));
        assert_eq!(delete_path(&guard, &root.join("gone.tmp"), false).unwrap(), DeleteOutcome::Vanished);
    }

    #[test]
    fn junction_is_removed_without_touching_its_target() {
        let (_t, root, outside) = setup();
        let secret = write_file(&outside.join("secret.txt"), 5);
        let link = root.join("link");
        junction::create(&outside, &link).unwrap();
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &link, false).unwrap(), DeleteOutcome::Deleted(0));
        assert!(fs::symlink_metadata(&link).is_err());
        assert!(secret.exists());
    }

    #[test]
    fn plain_directory_is_refused_by_delete_path() {
        let (_t, root, _o) = setup();
        let d = root.join("dir");
        fs::create_dir_all(&d).unwrap();
        let guard = Guard::new(&[root]);
        assert!(matches!(delete_path(&guard, &d, false), Err(CoreError::System(_))));
        assert!(d.exists());
    }

    #[test]
    fn path_longer_than_260_chars_is_deleted() {
        let (_t, root, _o) = setup();
        let mut deep = root.clone();
        for i in 0..25 {
            deep = deep.join(format!("thu_muc_sau_{i:02}"));
        }
        let f = write_file(&deep.join("tệp tạm.tmp"), 3);
        assert!(f.as_os_str().len() > 260);
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &f, false).unwrap(), DeleteOutcome::Deleted(3));
    }

    #[test]
    fn remove_empty_dir_respects_emptiness_and_roots() {
        let (_t, root, outside) = setup();
        let empty = root.join("empty");
        fs::create_dir_all(&empty).unwrap();
        let full = root.join("full");
        write_file(&full.join("f"), 1);
        let guard = Guard::new(&[root]);
        assert!(remove_empty_dir(&guard, &empty));
        assert!(!empty.exists());
        assert!(!remove_empty_dir(&guard, &full));
        assert!(full.exists());
        assert!(!remove_empty_dir(&guard, &outside));
        assert!(outside.exists());
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test -p winfreeup-core safety`
Expected: FAIL biên dịch — `cannot find type Guard`, `cannot find function delete_path`.

- [ ] **Step 3: Viết mã (đặt TRÊN khối test)**

```rust
//! Luật an toàn (spec mục 6): gốc được phép + canonicalize, không đi theo reparse point,
//! file khóa/không quyền thì bỏ qua và đếm.
use std::fs;
use std::io;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::error::{io_err, CoreError, Result};

const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const ERROR_SHARING_VIOLATION: i32 = 32;
const ERROR_LOCK_VIOLATION: i32 = 33;

pub fn is_reparse_point(meta: &fs::Metadata) -> bool {
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Chuẩn hóa thư mục cha rồi ghép tên: bản thân liên kết không bị phân giải sang đích.
fn canonical_location(path: &Path) -> io::Result<PathBuf> {
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => {
            Ok(fs::canonicalize(parent)?.join(name))
        }
        _ => fs::canonicalize(path),
    }
}

#[derive(Debug, Clone)]
pub struct Guard {
    roots: Vec<PathBuf>,
}

impl Guard {
    /// Gốc không tồn tại bị bỏ qua (không có gì để xóa ở đó).
    pub fn new(roots: &[PathBuf]) -> Guard {
        Guard { roots: roots.iter().filter_map(|r| fs::canonicalize(r).ok()).collect() }
    }

    pub fn is_allowed(&self, path: &Path) -> bool {
        match canonical_location(path) {
            Ok(loc) => self.roots.iter().any(|r| loc.starts_with(r)),
            Err(_) => false,
        }
    }

    pub fn check(&self, path: &Path) -> Result<()> {
        if self.is_allowed(path) {
            Ok(())
        } else {
            Err(CoreError::OutsideRoots(path.to_path_buf()))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted(u64),
    WouldDelete(u64),
    Locked,
    Vanished,
}

fn remove_entry(path: &Path, dir_link: bool) -> io::Result<()> {
    if dir_link {
        fs::remove_dir(path)
    } else {
        fs::remove_file(path)
    }
}

fn is_locked(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(ERROR_SHARING_VIOLATION) | Some(ERROR_LOCK_VIOLATION))
}

pub fn delete_path(guard: &Guard, path: &Path, dry_run: bool) -> Result<DeleteOutcome> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(DeleteOutcome::Vanished),
        Err(e) => return Err(io_err(path, e)),
    };
    guard.check(path)?;
    let reparse = is_reparse_point(&meta);
    if meta.is_dir() && !reparse {
        return Err(CoreError::System(format!(
            "refusing to delete a directory as a file: {}",
            path.display()
        )));
    }
    let bytes = if reparse { 0 } else { meta.len() };
    if dry_run {
        return Ok(DeleteOutcome::WouldDelete(bytes));
    }
    let dir_link = reparse && meta.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0;
    let err = match remove_entry(path, dir_link) {
        Ok(()) => return Ok(DeleteOutcome::Deleted(bytes)),
        Err(e) => e,
    };
    if err.kind() == io::ErrorKind::NotFound {
        return Ok(DeleteOutcome::Vanished);
    }
    if is_locked(&err) {
        return Ok(DeleteOutcome::Locked);
    }
    if err.kind() == io::ErrorKind::PermissionDenied {
        if meta.file_attributes() & FILE_ATTRIBUTE_READONLY != 0 {
            let mut perms = meta.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            if fs::set_permissions(path, perms).is_ok() && remove_entry(path, dir_link).is_ok() {
                return Ok(DeleteOutcome::Deleted(bytes));
            }
        }
        return Ok(DeleteOutcome::Locked);
    }
    Err(io_err(path, err))
}

/// Xóa thư mục nếu rỗng, nằm trong gốc được phép và không phải reparse point.
pub fn remove_empty_dir(guard: &Guard, dir: &Path) -> bool {
    if !guard.is_allowed(dir) {
        return false;
    }
    match fs::symlink_metadata(dir) {
        Ok(m) if m.is_dir() && !is_reparse_point(&m) => fs::remove_dir(dir).is_ok(),
        _ => false,
    }
}
```

- [ ] **Step 4: Chạy test, thấy qua**

Run: `cargo test -p winfreeup-core safety`
Expected: `test result: ok. 11 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-core/src/safety.rs
rtk git commit -m "feat(core): luật an toàn — gốc được phép, reparse point, file khóa/chỉ-đọc

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Duyệt và dọn theo `Target` + `FileCleaner`

**Files:**
- Modify: `crates/winfreeup-core/src/fsclean.rs` (thay dòng khung từ Task 1)
- Test: `crates/winfreeup-core/src/fsclean.rs` (module `tests`)

**Interfaces:**
- Consumes: từ Task 2 — `Guard::new(roots: &[PathBuf]) -> Guard`, `delete_path(guard: &Guard, path: &Path, dry_run: bool) -> Result<DeleteOutcome>` với `DeleteOutcome::{Deleted(u64), WouldDelete(u64), Locked, Vanished}`, `remove_empty_dir(guard: &Guard, dir: &Path) -> bool`, `is_reparse_point(meta: &fs::Metadata) -> bool`; từ Task 1 — `ScanResult::add_file`, `CleanReport`, `CleanOptions`, `CancelToken`, `ItemAction`, `Progress`, `NoProgress`, `Cleaner`, `RiskLevel`, `Env`, `CoreError::Cancelled`, `testutil::{write_file, age, Recorder, fake_env, FakeSys}`.
- Produces:
  - `pub const DAY: Duration`
  - `pub enum Filter { All, OlderThan(Duration), NamePrefix(&'static str) }` + `fn matches(&self, f: &Found, now: SystemTime) -> bool`
  - `pub struct Target { pub root: PathBuf, pub filter: Filter, pub recursive: bool, pub prune_empty_dirs: bool, pub remove_root: bool }` + `Target::all(root)`, `Target::older_than(root, age)`, `Target::prefixed(root, prefix)`
  - `pub struct Found { pub path: PathBuf, pub bytes: u64, pub modified: SystemTime, pub is_link: bool }`
  - `pub struct Walk { pub files: Vec<Found>, pub dirs: Vec<Found>, pub unreadable: u64 }` + `pub fn walk(root: &Path, recursive: bool, cancel: Option<&CancelToken>) -> Result<Walk>`
  - `pub fn roots_of(targets: &[Target]) -> Vec<PathBuf>`
  - `pub fn scan_targets(targets: &[Target], now: SystemTime, cancel: &CancelToken) -> Result<ScanResult>`
  - `pub fn clean_targets(targets: &[Target], now: SystemTime, opts: &CleanOptions, progress: &dyn Progress) -> CleanReport` (không bao giờ hỏng cả nhóm)
  - `pub struct FileCleaner { pub id: &'static str, pub risk: RiskLevel, pub default_selected: bool, pub targets: fn(&Env) -> Vec<Target> }` — `impl Cleaner`

- [ ] **Step 1: Viết test hỏng**

Thay dòng khung của `crates/winfreeup-core/src/fsclean.rs` bằng phần test dưới đây:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{age, fake_env, write_file, FakeSys, Recorder};
    use crate::types::NoProgress;
    use std::fs;
    use std::os::windows::fs::OpenOptionsExt;
    use std::sync::Arc;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn scan_counts_only_files_older_than_the_limit() {
        let t = tmp();
        let root = t.path().join("Temp");
        let old = write_file(&root.join("old.tmp"), 100);
        write_file(&root.join("new.tmp"), 50);
        age(&old, 48);
        let r = scan_targets(&[Target::older_than(root, DAY)], SystemTime::now(), &CancelToken::new()).unwrap();
        assert_eq!((r.total_bytes, r.file_count), (100, 1));
        assert_eq!(r.top_items.len(), 1);
    }

    // Review Focus 4
    #[test]
    fn missing_root_scans_as_empty_not_error() {
        let t = tmp();
        let r = scan_targets(&[Target::all(t.path().join("khong-co"))], SystemTime::now(), &CancelToken::new()).unwrap();
        assert_eq!(r, ScanResult::default());
        let rep = clean_targets(&[Target::all(t.path().join("khong-co"))], SystemTime::now(), &CleanOptions::default(), &NoProgress);
        assert!(rep.errors.is_empty());
    }

    #[test]
    fn scan_stops_when_cancelled() {
        let t = tmp();
        write_file(&t.path().join("r").join("a"), 1);
        let c = CancelToken::new();
        c.cancel();
        let err = scan_targets(&[Target::all(t.path().join("r"))], SystemTime::now(), &c).unwrap_err();
        assert!(matches!(err, CoreError::Cancelled));
    }

    #[test]
    fn clean_deletes_old_files_prunes_old_empty_dirs_keeps_fresh_ones_and_root() {
        let t = tmp();
        let root = t.path().join("Temp");
        let old_file = write_file(&root.join("old_dir").join("a.tmp"), 10);
        let new_file = write_file(&root.join("new.tmp"), 20);
        let old_in_fresh = write_file(&root.join("fresh_dir").join("b.tmp"), 30);
        age(&old_file, 48);
        age(&old_in_fresh, 48);
        age(&root.join("old_dir"), 48);
        let rec = Recorder::default();
        let rep = clean_targets(&[Target::older_than(root.clone(), DAY)], SystemTime::now(), &CleanOptions::default(), &rec);
        assert_eq!(rep.bytes_freed, 40);
        assert_eq!(rep.files_deleted, 2);
        assert!(!old_file.exists() && !old_in_fresh.exists());
        assert!(new_file.exists());
        assert!(!root.join("old_dir").exists());
        assert!(root.join("fresh_dir").exists());
        assert!(root.exists());
        assert_eq!(rec.actions(), vec![ItemAction::Deleted, ItemAction::Deleted]);
    }

    #[test]
    fn dry_run_deletes_nothing_but_reports_what_it_would_free() {
        let t = tmp();
        let root = t.path().join("c");
        let a = write_file(&root.join("sub").join("a"), 10);
        let b = write_file(&root.join("b"), 5);
        let rec = Recorder::default();
        let rep = clean_targets(&[Target::all(root.clone())], SystemTime::now(), &CleanOptions { dry_run: true }, &rec);
        assert!(rep.dry_run);
        assert_eq!((rep.bytes_freed, rep.files_deleted), (15, 2));
        assert!(a.exists() && b.exists() && root.join("sub").exists());
        assert!(rec.actions().iter().all(|x| *x == ItemAction::WouldDelete));
    }

    #[test]
    fn junction_inside_root_is_not_traversed() {
        let t = tmp();
        let root = t.path().join("cache");
        let outside = t.path().join("Documents");
        let precious = write_file(&outside.join("luan-van.docx"), 999);
        fs::create_dir_all(&root).unwrap();
        junction::create(&outside, root.join("link")).unwrap();
        let scan = scan_targets(&[Target::all(root.clone())], SystemTime::now(), &CancelToken::new()).unwrap();
        assert_eq!(scan.total_bytes, 0);
        clean_targets(&[Target::all(root.clone())], SystemTime::now(), &CleanOptions::default(), &NoProgress);
        assert!(precious.exists());
        assert!(fs::symlink_metadata(root.join("link")).is_err());
    }

    #[test]
    fn locked_file_is_counted_and_the_rest_still_cleaned() {
        let t = tmp();
        let root = t.path().join("c");
        let busy = write_file(&root.join("busy"), 5);
        let free = write_file(&root.join("free"), 7);
        let _h = fs::OpenOptions::new().read(true).share_mode(0).open(&busy).unwrap();
        let rep = clean_targets(&[Target::all(root)], SystemTime::now(), &CleanOptions::default(), &NoProgress);
        assert_eq!(rep.skipped_locked, 1);
        assert_eq!(rep.bytes_freed, 7);
        assert!(busy.exists() && !free.exists());
        assert!(rep.errors.is_empty());
    }

    #[test]
    fn file_root_is_treated_as_a_single_candidate() {
        let t = tmp();
        let dump = write_file(&t.path().join("MEMORY.DMP"), 64);
        let rep = clean_targets(&[Target::all(dump.clone())], SystemTime::now(), &CleanOptions::default(), &NoProgress);
        assert_eq!(rep.bytes_freed, 64);
        assert!(!dump.exists());
    }

    #[test]
    fn remove_root_deletes_the_emptied_root() {
        let t = tmp();
        let root = t.path().join("Windows.old");
        write_file(&root.join("a").join("b").join("c"), 3);
        let target = Target { remove_root: true, ..Target::all(root.clone()) };
        clean_targets(&[target], SystemTime::now(), &CleanOptions::default(), &NoProgress);
        assert!(!root.exists());
    }

    #[test]
    fn prefixed_target_matches_prefix_only_and_does_not_recurse() {
        let t = tmp();
        let root = t.path().join("Explorer");
        let thumb = write_file(&root.join("thumbcache_256.db"), 8);
        let icon = write_file(&root.join("iconcache_16.db"), 8);
        let nested = write_file(&root.join("sub").join("thumbcache_1.db"), 8);
        clean_targets(&[Target::prefixed(root, "thumbcache_")], SystemTime::now(), &CleanOptions::default(), &NoProgress);
        assert!(!thumb.exists());
        assert!(icon.exists() && nested.exists());
    }

    // Review Focus 3
    #[test]
    fn vanished_file_between_scan_and_clean_is_not_an_error() {
        let t = tmp();
        let sys = Arc::new(FakeSys::default());
        let env = fake_env(t.path(), sys);
        let f = write_file(&env.temp.join("x.tmp"), 9);
        age(&f, 48);
        let c = FileCleaner { id: "t", risk: RiskLevel::Safe, default_selected: true, targets: |e| vec![Target::older_than(e.temp.clone(), DAY)] };
        let scan = c.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan.total_bytes, 9);
        fs::remove_file(&f).unwrap();
        let rep = c.clean(&env, &scan, &CleanOptions::default(), &NoProgress).unwrap();
        assert!(rep.errors.is_empty());
        assert_eq!(rep.files_deleted, 0);
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test -p winfreeup-core fsclean`
Expected: FAIL biên dịch — `cannot find struct Target`, `cannot find function scan_targets`.

- [ ] **Step 3: Viết mã (đặt TRÊN khối test)**

```rust
//! Duyệt cây thư mục (không đi theo reparse point) và dọn theo danh sách `Target`.
use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::env::Env;
use crate::error::{io_err, CoreError, Result};
use crate::safety::{delete_path, is_reparse_point, remove_empty_dir, DeleteOutcome, Guard};
use crate::types::{
    CancelToken, CleanOptions, CleanReport, Cleaner, ItemAction, Progress, RiskLevel, ScanResult,
};

pub const DAY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filter {
    All,
    OlderThan(Duration),
    NamePrefix(&'static str),
}

#[derive(Debug, Clone)]
pub struct Target {
    pub root: PathBuf,
    pub filter: Filter,
    pub recursive: bool,
    pub prune_empty_dirs: bool,
    pub remove_root: bool,
}

impl Target {
    pub fn all(root: PathBuf) -> Target {
        Target { root, filter: Filter::All, recursive: true, prune_empty_dirs: true, remove_root: false }
    }
    pub fn older_than(root: PathBuf, age: Duration) -> Target {
        Target { filter: Filter::OlderThan(age), ..Target::all(root) }
    }
    pub fn prefixed(root: PathBuf, prefix: &'static str) -> Target {
        Target { root, filter: Filter::NamePrefix(prefix), recursive: false, prune_empty_dirs: false, remove_root: false }
    }
}

#[derive(Debug, Clone)]
pub struct Found {
    pub path: PathBuf,
    pub bytes: u64,
    pub modified: SystemTime,
    pub is_link: bool,
}

fn is_old(modified: SystemTime, age: Duration, now: SystemTime) -> bool {
    now.duration_since(modified).map(|d| d >= age).unwrap_or(false)
}

impl Filter {
    pub fn matches(&self, f: &Found, now: SystemTime) -> bool {
        match self {
            Filter::All => true,
            Filter::OlderThan(age) => is_old(f.modified, *age, now),
            Filter::NamePrefix(p) => f
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase().starts_with(&p.to_lowercase()))
                .unwrap_or(false),
        }
    }

    fn may_prune_dir(&self, d: &Found, now: SystemTime) -> bool {
        match self {
            Filter::All => true,
            Filter::OlderThan(age) => is_old(d.modified, *age, now),
            Filter::NamePrefix(_) => false,
        }
    }
}

pub struct Walk {
    pub files: Vec<Found>,
    pub dirs: Vec<Found>,
    pub unreadable: u64,
}

fn found(path: PathBuf, meta: &fs::Metadata) -> Found {
    let is_link = is_reparse_point(meta);
    Found {
        bytes: if is_link || meta.is_dir() { 0 } else { meta.len() },
        // Không đọc được giờ ⇒ coi như mới ⇒ không bị lọc "cũ hơn 24 giờ" xóa nhầm.
        modified: meta.modified().unwrap_or_else(|_| SystemTime::now()),
        is_link,
        path,
    }
}

/// Liệt kê file (kể cả liên kết, coi như "file" 0 byte) và thư mục con. Không bao giờ đi vào reparse point.
/// Gốc là reparse point ⇒ bỏ qua cả gốc. Gốc là file ⇒ chính nó là ứng viên.
pub fn walk(root: &Path, recursive: bool, cancel: Option<&CancelToken>) -> Result<Walk> {
    let mut w = Walk { files: vec![], dirs: vec![], unreadable: 0 };
    let meta = match fs::symlink_metadata(root) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(w),
        Err(e) => return Err(io_err(root, e)),
    };
    if is_reparse_point(&meta) {
        return Ok(w);
    }
    if !meta.is_dir() {
        w.files.push(found(root.to_path_buf(), &meta));
        return Ok(w);
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if cancel.is_some_and(|c| c.is_cancelled()) {
            return Err(CoreError::Cancelled);
        }
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => {
                w.unreadable += 1;
                continue;
            }
        };
        for entry in entries {
            let Ok(entry) = entry else {
                w.unreadable += 1;
                continue;
            };
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                w.unreadable += 1;
                continue;
            };
            if is_reparse_point(&meta) {
                w.files.push(found(path, &meta));
            } else if meta.is_dir() {
                if recursive {
                    w.dirs.push(found(path.clone(), &meta));
                    stack.push(path);
                }
            } else {
                w.files.push(found(path, &meta));
            }
        }
    }
    Ok(w)
}

pub fn roots_of(targets: &[Target]) -> Vec<PathBuf> {
    targets.iter().map(|t| t.root.clone()).collect()
}

pub fn scan_targets(targets: &[Target], now: SystemTime, cancel: &CancelToken) -> Result<ScanResult> {
    let mut r = ScanResult::default();
    for t in targets {
        let w = walk(&t.root, t.recursive, Some(cancel))?;
        for f in w.files.iter().filter(|f| t.filter.matches(f, now)) {
            r.add_file(&f.path, f.bytes);
        }
    }
    Ok(r)
}

pub fn clean_targets(targets: &[Target], now: SystemTime, opts: &CleanOptions, progress: &dyn Progress) -> CleanReport {
    let guard = Guard::new(&roots_of(targets));
    let mut rep = CleanReport { dry_run: opts.dry_run, ..Default::default() };
    for t in targets {
        let mut w = match walk(&t.root, t.recursive, None) {
            Ok(w) => w,
            Err(e) => {
                rep.errors.push(e.to_string());
                continue;
            }
        };
        rep.skipped_locked += w.unreadable;
        for f in w.files.iter().filter(|f| t.filter.matches(f, now)) {
            match delete_path(&guard, &f.path, opts.dry_run) {
                Ok(DeleteOutcome::Deleted(b)) => {
                    rep.bytes_freed += b;
                    rep.files_deleted += 1;
                    progress.item(ItemAction::Deleted, &f.path, b, None);
                }
                Ok(DeleteOutcome::WouldDelete(b)) => {
                    rep.bytes_freed += b;
                    rep.files_deleted += 1;
                    progress.item(ItemAction::WouldDelete, &f.path, b, None);
                }
                Ok(DeleteOutcome::Locked) => {
                    rep.skipped_locked += 1;
                    progress.item(ItemAction::SkippedLocked, &f.path, f.bytes, None);
                }
                Ok(DeleteOutcome::Vanished) => {}
                Err(e) => {
                    let msg = e.to_string();
                    progress.item(ItemAction::Failed, &f.path, 0, Some(&msg));
                    rep.errors.push(msg);
                }
            }
        }
        if opts.dry_run {
            continue;
        }
        if t.prune_empty_dirs {
            w.dirs.sort_by_key(|d| Reverse(d.path.components().count()));
            for d in w.dirs.iter().filter(|d| t.filter.may_prune_dir(d, now)) {
                remove_empty_dir(&guard, &d.path);
            }
        }
        if t.remove_root {
            remove_empty_dir(&guard, &t.root);
        }
    }
    rep
}

/// Nhóm dọn chỉ gồm file trong các `Target` (không cần dịch vụ hay lệnh hệ thống).
pub struct FileCleaner {
    pub id: &'static str,
    pub risk: RiskLevel,
    pub default_selected: bool,
    pub targets: fn(&Env) -> Vec<Target>,
}

impl Cleaner for FileCleaner {
    fn id(&self) -> &'static str {
        self.id
    }
    fn risk(&self) -> RiskLevel {
        self.risk
    }
    fn default_selected(&self) -> bool {
        self.default_selected
    }
    fn allowed_roots(&self, env: &Env) -> Vec<PathBuf> {
        roots_of(&(self.targets)(env))
    }
    fn scan(&self, env: &Env, cancel: &CancelToken) -> Result<ScanResult> {
        scan_targets(&(self.targets)(env), env.now, cancel)
    }
    fn clean(&self, env: &Env, _scan: &ScanResult, opts: &CleanOptions, progress: &dyn Progress) -> Result<CleanReport> {
        Ok(clean_targets(&(self.targets)(env), env.now, opts, progress))
    }
}

```

- [ ] **Step 4: Chạy test, thấy qua**

Run: `cargo test -p winfreeup-core fsclean`
Expected: `test result: ok. 11 passed; 0 failed`.

- [ ] **Step 5: Chạy toàn bộ lõi**

Run: `cargo test -p winfreeup-core`
Expected: `0 failed` (số passed tùy các task đã gộp).

- [ ] **Step 6: Commit**

```bash
rtk git add crates/winfreeup-core/src/fsclean.rs
rtk git commit -m "feat(core): duyệt/dọn theo Target, dry-run, tỉa thư mục rỗng, FileCleaner

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 4: Bốn nhóm file đơn giản — `user_temp`, `system_temp`, `win_caches`, `delivery_opt`

**Files:**
- Modify (thay dòng khung từ Task 1): `crates/winfreeup-core/src/cleaners/user_temp.rs`
- Modify (thay dòng khung): `crates/winfreeup-core/src/cleaners/system_temp.rs`
- Modify (thay dòng khung): `crates/winfreeup-core/src/cleaners/win_caches.rs`
- Modify (thay dòng khung): `crates/winfreeup-core/src/cleaners/delivery_opt.rs`
- Test: trong từng file module (module `tests`)

**Interfaces:**
- Consumes: từ Task 3 — `FileCleaner { id: &'static str, risk: RiskLevel, default_selected: bool, targets: fn(&Env) -> Vec<Target> }` (đã `impl Cleaner`), `Target::older_than(root: PathBuf, age: Duration)`, `Target::all(root: PathBuf)`, `Target::prefixed(root: PathBuf, prefix: &'static str)` (không đệ quy, không tỉa thư mục), `DAY: Duration`; từ Task 1 — `Env`, `RiskLevel`, `Cleaner`, `CancelToken`, `CleanOptions`, `NoProgress`, `testutil::{fake_env, write_file, age, FakeSys}`.
- Produces:
  - `cleaners::user_temp::{targets(env: &Env) -> Vec<Target>, cleaner() -> FileCleaner}` — id `"user_temp"`, Safe, tích sẵn.
  - `cleaners::system_temp::{targets, cleaner}` — id `"system_temp"`, Safe, tích sẵn.
  - `cleaners::win_caches::{targets, cleaner}` — id `"win_caches"`, Safe, tích sẵn.
  - `cleaners::delivery_opt::{targets, cleaner}` — id `"delivery_opt"`, Safe, tích sẵn.

- [ ] **Step 1: Viết test hỏng**

Thay dòng khung của bốn file bằng phần test tương ứng dưới đây.

`crates/winfreeup-core/src/cleaners/user_temp.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{age, fake_env, write_file, FakeSys};
    use crate::types::{CancelToken, CleanOptions, Cleaner, NoProgress};
    use std::sync::Arc;

    #[test]
    fn metadata_matches_spec_table() {
        let c = cleaner();
        assert_eq!((c.id, c.risk, c.default_selected), ("user_temp", RiskLevel::Safe, true));
    }

    #[test]
    fn cleans_only_user_temp_files_older_than_24h() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys::default()));
        let old = write_file(&env.temp.join("setup.log"), 100);
        let new = write_file(&env.temp.join("dang-dung.tmp"), 50);
        age(&old, 30);
        let c = cleaner();
        let scan = c.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan.total_bytes, 100);
        c.clean(&env, &scan, &CleanOptions::default(), &NoProgress).unwrap();
        assert!(!old.exists() && new.exists());
    }
}
```

`crates/winfreeup-core/src/cleaners/system_temp.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{age, fake_env, write_file, FakeSys};
    use crate::types::{CancelToken, Cleaner};
    use std::sync::Arc;

    #[test]
    fn metadata_matches_spec_table() {
        let c = cleaner();
        assert_eq!((c.id, c.risk, c.default_selected), ("system_temp", RiskLevel::Safe, true));
    }

    #[test]
    fn scans_windows_temp_older_than_24h_only() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys::default()));
        let old = write_file(&env.windir.join("Temp").join("a.tmp"), 10);
        write_file(&env.windir.join("Temp").join("b.tmp"), 20);
        write_file(&env.temp.join("user.tmp"), 40);
        age(&old, 25);
        assert_eq!(cleaner().scan(&env, &CancelToken::new()).unwrap().total_bytes, 10);
    }
}
```

`crates/winfreeup-core/src/cleaners/win_caches.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{fake_env, write_file, FakeSys};
    use crate::types::{CancelToken, CleanOptions, Cleaner, NoProgress};
    use std::sync::Arc;

    #[test]
    fn metadata_matches_spec_table() {
        let c = cleaner();
        assert_eq!((c.id, c.risk, c.default_selected), ("win_caches", RiskLevel::Safe, true));
    }

    #[test]
    fn cleans_thumbnails_wer_and_dumps_but_not_neighbours() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys::default()));
        let la = env.local_appdata.clone();
        let thumb = write_file(&la.join(r"Microsoft\Windows\Explorer\thumbcache_256.db"), 1);
        let icon = write_file(&la.join(r"Microsoft\Windows\Explorer\iconcache_32.db"), 1);
        let wer = write_file(&env.program_data.join(r"Microsoft\Windows\WER\ReportArchive\r1\Report.wer"), 2);
        let wer_q = write_file(&la.join(r"Microsoft\Windows\WER\ReportQueue\q1\Report.wer"), 3);
        let dump = write_file(&la.join(r"CrashDumps\app.exe.123.dmp"), 4);
        let mini = write_file(&env.windir.join(r"Minidump\092526-01.dmp"), 5);
        let memory = write_file(&env.windir.join("MEMORY.DMP"), 6);
        let other = write_file(&env.windir.join("win.ini"), 7);
        let c = cleaner();
        let scan = c.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan.total_bytes, 1 + 2 + 3 + 4 + 5 + 6);
        c.clean(&env, &scan, &CleanOptions::default(), &NoProgress).unwrap();
        for gone in [&thumb, &wer, &wer_q, &dump, &mini, &memory] {
            assert!(!gone.exists(), "{}", gone.display());
        }
        assert!(icon.exists() && other.exists());
        assert!(env.windir.exists());
    }

    // Review Focus 4
    #[test]
    fn machine_without_any_of_these_folders_scans_zero() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys::default()));
        let scan = cleaner().scan(&env, &CancelToken::new()).unwrap();
        assert_eq!((scan.total_bytes, scan.file_count), (0, 0));
    }
}
```

`crates/winfreeup-core/src/cleaners/delivery_opt.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{fake_env, write_file, FakeSys};
    use crate::types::{CancelToken, Cleaner};
    use std::sync::Arc;

    #[test]
    fn metadata_matches_spec_table() {
        let c = cleaner();
        assert_eq!((c.id, c.risk, c.default_selected), ("delivery_opt", RiskLevel::Safe, true));
    }

    #[test]
    fn scans_delivery_optimization_cache() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys::default()));
        write_file(&env.windir.join(CACHE_REL).join("ab").join("blob"), 42);
        assert_eq!(cleaner().scan(&env, &CancelToken::new()).unwrap().total_bytes, 42);
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test -p winfreeup-core cleaners::user_temp`
Expected: FAIL biên dịch — `cannot find function cleaner in this scope`.

- [ ] **Step 3: Viết mã (đặt TRÊN khối test của từng file)**

`user_temp.rs`:

```rust
use crate::env::Env;
use crate::fsclean::{FileCleaner, Target, DAY};
use crate::types::RiskLevel;

pub fn targets(env: &Env) -> Vec<Target> {
    vec![Target::older_than(env.temp.clone(), DAY)]
}

pub fn cleaner() -> FileCleaner {
    FileCleaner { id: "user_temp", risk: RiskLevel::Safe, default_selected: true, targets }
}
```

`system_temp.rs`:

```rust
use crate::env::Env;
use crate::fsclean::{FileCleaner, Target, DAY};
use crate::types::RiskLevel;

pub fn targets(env: &Env) -> Vec<Target> {
    vec![Target::older_than(env.windir.join("Temp"), DAY)]
}

pub fn cleaner() -> FileCleaner {
    FileCleaner { id: "system_temp", risk: RiskLevel::Safe, default_selected: true, targets }
}
```

`win_caches.rs`:

```rust
use crate::env::Env;
use crate::fsclean::{FileCleaner, Target};
use crate::types::RiskLevel;

pub fn targets(env: &Env) -> Vec<Target> {
    let la = &env.local_appdata;
    let pd = &env.program_data;
    let w = &env.windir;
    vec![
        Target::prefixed(la.join(r"Microsoft\Windows\Explorer"), "thumbcache_"),
        Target::all(pd.join(r"Microsoft\Windows\WER\ReportArchive")),
        Target::all(pd.join(r"Microsoft\Windows\WER\ReportQueue")),
        Target::all(la.join(r"Microsoft\Windows\WER\ReportArchive")),
        Target::all(la.join(r"Microsoft\Windows\WER\ReportQueue")),
        Target::all(la.join("CrashDumps")),
        Target::all(w.join("Minidump")),
        Target::all(w.join("MEMORY.DMP")),
    ]
}

pub fn cleaner() -> FileCleaner {
    FileCleaner { id: "win_caches", risk: RiskLevel::Safe, default_selected: true, targets }
}
```

`delivery_opt.rs`:

```rust
use crate::env::Env;
use crate::fsclean::{FileCleaner, Target};
use crate::types::RiskLevel;

pub const CACHE_REL: &str = r"ServiceProfiles\NetworkService\AppData\Local\Microsoft\Windows\DeliveryOptimization\Cache";

pub fn targets(env: &Env) -> Vec<Target> {
    vec![Target::all(env.windir.join(CACHE_REL))]
}

pub fn cleaner() -> FileCleaner {
    FileCleaner { id: "delivery_opt", risk: RiskLevel::Safe, default_selected: true, targets }
}
```

- [ ] **Step 4: Chạy test, thấy qua**

Run lần lượt: `cargo test -p winfreeup-core cleaners::user_temp`, `… cleaners::system_temp`, `… cleaners::win_caches`, `… cleaners::delivery_opt`
Expected: `2 passed`, `2 passed`, `3 passed`, `2 passed`; 0 failed.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-core/src/cleaners/user_temp.rs
rtk git add crates/winfreeup-core/src/cleaners/system_temp.rs
rtk git add crates/winfreeup-core/src/cleaners/win_caches.rs
rtk git add crates/winfreeup-core/src/cleaners/delivery_opt.rs
rtk git commit -m "feat(core): nhóm user_temp, system_temp, win_caches, delivery_opt

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Nhóm `browser_cache` (Chrome, Edge, Firefox, Cốc Cốc)

**Files:**
- Modify: `crates/winfreeup-core/src/cleaners/browser_cache.rs` (thay dòng khung từ Task 1)
- Test: `crates/winfreeup-core/src/cleaners/browser_cache.rs` (module `tests`)

**Interfaces:**
- Consumes: từ Task 3 — `Target::all(root: PathBuf) -> Target`, `roots_of(targets: &[Target]) -> Vec<PathBuf>`, `scan_targets(targets: &[Target], now: SystemTime, cancel: &CancelToken) -> Result<ScanResult>`, `clean_targets(targets: &[Target], now: SystemTime, opts: &CleanOptions, progress: &dyn Progress) -> CleanReport`; từ Task 2 — `safety::is_reparse_point(meta: &fs::Metadata) -> bool`; từ Task 1 — `SystemOps::is_process_running(&self, exe_name: &str) -> bool`, `Cleaner`, `RiskLevel`, `ScanResult.notices`, `testutil::FakeSys { running, .. }`.
- Produces:
  - `pub struct BrowserCache;` — `impl Cleaner`, id `"browser_cache"`, Safe, tích sẵn.
  - `pub fn targets(env: &Env) -> Vec<Target>` — chỉ thư mục cache của mọi profile.
  - Mã thông báo trong `ScanResult.notices`: `"browser_running:<key>"` với key ∈ `chrome | edge | firefox | coccoc`.

- [ ] **Step 1: Viết test hỏng**

Thay dòng khung của `browser_cache.rs` bằng phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{fake_env, write_file, FakeSys};
    use crate::types::{CancelToken, CleanOptions, NoProgress};
    use std::sync::Arc;

    #[test]
    fn cleans_cache_dirs_of_every_profile_and_keeps_user_data() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys::default()));
        let chrome = env.local_appdata.join(r"Google\Chrome\User Data");
        let c1 = write_file(&chrome.join(r"Default\Cache\Cache_Data\f_000001"), 10);
        let c2 = write_file(&chrome.join(r"Profile 1\Code Cache\js\a"), 20);
        let c3 = write_file(&chrome.join(r"Default\GPUCache\data_1"), 30);
        let cookies = write_file(&chrome.join(r"Default\Network\Cookies"), 5);
        let history = write_file(&chrome.join(r"Default\History"), 5);
        let logins = write_file(&chrome.join(r"Default\Login Data"), 5);
        let coccoc = write_file(&env.local_appdata.join(r"CocCoc\Browser\User Data\Default\Cache\x"), 40);
        let edge = write_file(&env.local_appdata.join(r"Microsoft\Edge\User Data\Default\Cache\y"), 50);
        let ff = env.local_appdata.join(r"Mozilla\Firefox\Profiles\abc.default-release");
        let ff_cache = write_file(&ff.join(r"cache2\entries\E1"), 60);
        let ff_places = write_file(&ff.join("places.sqlite"), 5);

        let c = BrowserCache;
        let scan = c.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan.total_bytes, 10 + 20 + 30 + 40 + 50 + 60);
        c.clean(&env, &scan, &CleanOptions::default(), &NoProgress).unwrap();
        for gone in [&c1, &c2, &c3, &coccoc, &edge, &ff_cache] {
            assert!(!gone.exists(), "{}", gone.display());
        }
        for kept in [&cookies, &history, &logins, &ff_places] {
            assert!(kept.exists(), "{}", kept.display());
        }
        assert!(chrome.join(r"Default\Cache").exists(), "thư mục gốc cache được giữ");
    }

    #[test]
    fn running_installed_browser_produces_notice() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys { running: vec!["chrome.exe".into(), "firefox.exe".into()], ..Default::default() });
        let env = fake_env(t.path(), sys);
        write_file(&env.local_appdata.join(r"Google\Chrome\User Data\Default\Cache\x"), 1);
        let scan = BrowserCache.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan.notices, vec!["browser_running:chrome".to_string()]);
    }

    // Review Focus 4
    #[test]
    fn no_browser_installed_scans_zero_without_notice() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys::default()));
        let scan = BrowserCache.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan, ScanResult::default());
    }

    #[test]
    fn metadata_matches_spec_table() {
        assert_eq!((BrowserCache.id(), BrowserCache.risk(), BrowserCache.default_selected()), ("browser_cache", RiskLevel::Safe, true));
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test -p winfreeup-core browser_cache`
Expected: FAIL biên dịch — `cannot find value BrowserCache`.

- [ ] **Step 3: Viết mã (đặt TRÊN khối test)**

```rust
//! Chỉ thư mục cache của từng profile. Không đụng cookie, lịch sử, mật khẩu.
use std::fs;
use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::error::Result;
use crate::fsclean::{clean_targets, roots_of, scan_targets, Target};
use crate::safety::is_reparse_point;
use crate::types::{CancelToken, CleanOptions, CleanReport, Cleaner, Progress, RiskLevel, ScanResult};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Chromium,
    Firefox,
}

const CHROMIUM_CACHE_DIRS: &[&str] = &["Cache", "Code Cache", "GPUCache"];

/// (khoá, tên tiến trình, loại, thư mục chứa các profile)
fn browsers(env: &Env) -> Vec<(&'static str, &'static str, Kind, PathBuf)> {
    let la = &env.local_appdata;
    vec![
        ("chrome", "chrome.exe", Kind::Chromium, la.join(r"Google\Chrome\User Data")),
        ("edge", "msedge.exe", Kind::Chromium, la.join(r"Microsoft\Edge\User Data")),
        ("coccoc", "browser.exe", Kind::Chromium, la.join(r"CocCoc\Browser\User Data")),
        ("firefox", "firefox.exe", Kind::Firefox, la.join(r"Mozilla\Firefox\Profiles")),
    ]
}

fn profile_dirs(data_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(data_dir) else { return vec![] };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .filter(|e| fs::symlink_metadata(e.path()).map(|m| m.is_dir() && !is_reparse_point(&m)).unwrap_or(false))
        .map(|e| e.path())
        .collect();
    dirs.sort();
    dirs
}

pub fn targets(env: &Env) -> Vec<Target> {
    let mut out = Vec::new();
    for (_, _, kind, data_dir) in browsers(env) {
        for profile in profile_dirs(&data_dir) {
            match kind {
                Kind::Chromium => {
                    for d in CHROMIUM_CACHE_DIRS {
                        out.push(Target::all(profile.join(d)));
                    }
                }
                Kind::Firefox => out.push(Target::all(profile.join("cache2"))),
            }
        }
    }
    out
}

pub struct BrowserCache;

impl Cleaner for BrowserCache {
    fn id(&self) -> &'static str {
        "browser_cache"
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Safe
    }
    fn default_selected(&self) -> bool {
        true
    }
    fn allowed_roots(&self, env: &Env) -> Vec<PathBuf> {
        roots_of(&targets(env))
    }
    fn scan(&self, env: &Env, cancel: &CancelToken) -> Result<ScanResult> {
        let mut r = scan_targets(&targets(env), env.now, cancel)?;
        for (key, exe, _, data_dir) in browsers(env) {
            if data_dir.exists() && env.sys.is_process_running(exe) {
                r.notices.push(format!("browser_running:{key}"));
            }
        }
        Ok(r)
    }
    fn clean(&self, env: &Env, _scan: &ScanResult, opts: &CleanOptions, progress: &dyn Progress) -> Result<CleanReport> {
        Ok(clean_targets(&targets(env), env.now, opts, progress))
    }
}
```

- [ ] **Step 4: Chạy test, thấy qua**

Run: `cargo test -p winfreeup-core browser_cache`
Expected: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-core/src/cleaners/browser_cache.rs
rtk git commit -m "feat(core): nhóm browser_cache — chỉ thư mục cache mọi profile, báo trình duyệt đang mở

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: `ServiceGuard` (luôn bật lại dịch vụ), nhóm `wu_download` và `windows_old`

**Files:**
- Modify (thay dòng khung từ Task 1): `crates/winfreeup-core/src/service.rs`
- Modify (thay dòng khung): `crates/winfreeup-core/src/cleaners/wu_download.rs`
- Modify (thay dòng khung): `crates/winfreeup-core/src/cleaners/windows_old.rs`
- Test: trong từng file (module `tests`)

**Interfaces:**
- Consumes: từ Task 1 — `SystemOps::stop_service(&self, name: &str) -> Result<bool>` (true = đang chạy và đã dừng), `start_service(&self, name: &str) -> Result<()>`, `take_ownership(&self, path: &Path) -> Result<()>`, `Env.sys: Arc<dyn SystemOps>`, `testutil::FakeSys { service_was_running, start_fails, .. }` với chuỗi ghi `"stop:<n>"`, `"start:<n>"`, `"own:<path>"`; từ Task 3 — `Target { root, filter, recursive, prune_empty_dirs, remove_root }`, `Target::all`, `scan_targets`, `clean_targets`, `roots_of` (chữ ký như Task 5).
- Produces:
  - `pub struct ServiceGuard<'a>` + `ServiceGuard::stop(sys: &'a dyn SystemOps, name: &'static str) -> Result<ServiceGuard<'a>>`, `finish(self) -> Result<()>`; `Drop` bật lại nếu chưa `finish`.
  - `cleaners::wu_download::{WuDownload, SERVICE, targets}` — id `"wu_download"`, Caution, không tích sẵn.
  - `cleaners::windows_old::{WindowsOld, target}` — id `"windows_old"`, Risky, không tích sẵn.

- [ ] **Step 1: Viết test hỏng cho `service.rs`**

Thay dòng khung của `service.rs` bằng phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::FakeSys;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    #[test]
    fn finish_restarts_a_service_that_was_running() {
        let sys = FakeSys { service_was_running: true, ..Default::default() };
        ServiceGuard::stop(&sys, "wuauserv").unwrap().finish().unwrap();
        assert_eq!(sys.calls(), vec!["stop:wuauserv", "start:wuauserv"]);
    }

    #[test]
    fn service_that_was_already_stopped_is_left_stopped() {
        let sys = FakeSys { service_was_running: false, ..Default::default() };
        ServiceGuard::stop(&sys, "wuauserv").unwrap().finish().unwrap();
        assert_eq!(sys.calls(), vec!["stop:wuauserv"]);
    }

    #[test]
    fn drop_restarts_even_when_the_work_panics() {
        let sys = FakeSys { service_was_running: true, ..Default::default() };
        let r = catch_unwind(AssertUnwindSafe(|| {
            let _g = ServiceGuard::stop(&sys, "wuauserv").unwrap();
            panic!("lỗi giữa chừng");
        }));
        assert!(r.is_err());
        assert_eq!(sys.calls(), vec!["stop:wuauserv", "start:wuauserv"]);
    }

    #[test]
    fn drop_restarts_on_early_return_with_error() {
        let sys = FakeSys { service_was_running: true, ..Default::default() };
        fn work(sys: &FakeSys) -> Result<()> {
            let _g = ServiceGuard::stop(sys, "wuauserv")?;
            Err(crate::error::CoreError::System("xóa hỏng".into()))
        }
        assert!(work(&sys).is_err());
        assert_eq!(sys.calls(), vec!["stop:wuauserv", "start:wuauserv"]);
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test -p winfreeup-core service::`
Expected: FAIL biên dịch — `cannot find type ServiceGuard`.

- [ ] **Step 3: Viết `service.rs` (đặt TRÊN khối test)**

```rust
//! Dịch vụ đã dừng thì LUÔN được bật lại (spec mục 6.5), kể cả khi lỗi hoặc panic giữa chừng.
use crate::env::SystemOps;
use crate::error::Result;

pub struct ServiceGuard<'a> {
    sys: &'a dyn SystemOps,
    name: &'static str,
    restart: bool,
}

impl<'a> ServiceGuard<'a> {
    pub fn stop(sys: &'a dyn SystemOps, name: &'static str) -> Result<ServiceGuard<'a>> {
        let was_running = sys.stop_service(name)?;
        Ok(ServiceGuard { sys, name, restart: was_running })
    }

    /// Bật lại ngay và trả lỗi bật lại (nếu có) cho người gọi ghi vào báo cáo.
    pub fn finish(mut self) -> Result<()> {
        if self.restart {
            self.restart = false;
            self.sys.start_service(self.name)
        } else {
            Ok(())
        }
    }
}

impl Drop for ServiceGuard<'_> {
    fn drop(&mut self) {
        if self.restart {
            self.restart = false;
            let _ = self.sys.start_service(self.name);
        }
    }
}
```

- [ ] **Step 4: Chạy test, thấy qua**

Run: `cargo test -p winfreeup-core service::`
Expected: `test result: ok. 4 passed`.

- [ ] **Step 5: Viết test hỏng cho `wu_download` và `windows_old`**

Thay dòng khung của `crates/winfreeup-core/src/cleaners/wu_download.rs` bằng phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{fake_env, write_file, FakeSys};
    use crate::types::{CancelToken, NoProgress};
    use std::sync::Arc;

    fn setup(sys: FakeSys) -> (tempfile::TempDir, Arc<FakeSys>, Env, std::path::PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(sys);
        let env = fake_env(t.path(), sys.clone());
        let f = write_file(&env.windir.join(r"SoftwareDistribution\Download\abc\update.cab"), 70);
        (t, sys, env, f)
    }

    #[test]
    fn stops_cleans_and_restarts_wuauserv() {
        let (_t, sys, env, f) = setup(FakeSys { service_was_running: true, ..Default::default() });
        let scan = WuDownload.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan.total_bytes, 70);
        let rep = WuDownload.clean(&env, &scan, &CleanOptions::default(), &NoProgress).unwrap();
        assert_eq!(rep.bytes_freed, 70);
        assert!(!f.exists());
        assert_eq!(sys.calls(), vec!["stop:wuauserv", "start:wuauserv"]);
    }

    #[test]
    fn restart_failure_is_reported_not_swallowed() {
        let (_t, _sys, env, _f) = setup(FakeSys { service_was_running: true, start_fails: true, ..Default::default() });
        let rep = WuDownload.clean(&env, &ScanResult::default(), &CleanOptions::default(), &NoProgress).unwrap();
        assert_eq!(rep.errors.len(), 1);
        assert!(rep.errors[0].contains("wuauserv"), "{:?}", rep.errors);
    }

    #[test]
    fn dry_run_touches_neither_service_nor_files() {
        let (_t, sys, env, f) = setup(FakeSys { service_was_running: true, ..Default::default() });
        let rep = WuDownload.clean(&env, &ScanResult::default(), &CleanOptions { dry_run: true }, &NoProgress).unwrap();
        assert_eq!(rep.bytes_freed, 70);
        assert!(f.exists());
        assert!(sys.calls().is_empty());
    }

    #[test]
    fn metadata_matches_spec_table() {
        assert_eq!((WuDownload.id(), WuDownload.risk(), WuDownload.default_selected()), ("wu_download", RiskLevel::Caution, false));
    }
}
```

Thay dòng khung của `crates/winfreeup-core/src/cleaners/windows_old.rs` bằng phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{fake_env, write_file, FakeSys};
    use crate::types::{CancelToken, NoProgress};
    use std::sync::Arc;

    #[test]
    fn removes_windows_old_entirely_without_following_junctions() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys::default());
        let env = fake_env(t.path(), sys.clone());
        let old = env.system_drive.join("Windows.old");
        write_file(&old.join(r"Windows\System32\kernel32.dll"), 100);
        write_file(&old.join(r"Program Files\app\a.exe"), 50);
        let users = t.path().join("Users");
        let precious = write_file(&users.join(r"me\Documents\anh-cuoi.jpg"), 9);
        std::fs::create_dir_all(old.join("Documents and Settings").parent().unwrap()).unwrap();
        junction::create(&users, old.join("Documents and Settings")).unwrap();

        let scan = WindowsOld.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!(scan.total_bytes, 150);
        let rep = WindowsOld.clean(&env, &scan, &CleanOptions::default(), &NoProgress).unwrap();
        assert_eq!(rep.bytes_freed, 150);
        assert!(!old.exists(), "Windows.old phải biến mất");
        assert!(precious.exists(), "đích của junction không được đụng");
        assert_eq!(sys.calls(), vec![format!("own:{}", old.display())]);
    }

    // Review Focus 4
    #[test]
    fn missing_windows_old_scans_zero_and_never_takes_ownership() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys::default());
        let env = fake_env(t.path(), sys.clone());
        assert_eq!(WindowsOld.scan(&env, &CancelToken::new()).unwrap().total_bytes, 0);
        let rep = WindowsOld.clean(&env, &ScanResult::default(), &CleanOptions::default(), &NoProgress).unwrap();
        assert!(rep.errors.is_empty());
        assert!(sys.calls().is_empty());
    }

    #[test]
    fn dry_run_keeps_everything_and_skips_ownership() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys::default());
        let env = fake_env(t.path(), sys.clone());
        let f = write_file(&env.system_drive.join(r"Windows.old\x.bin"), 5);
        let rep = WindowsOld.clean(&env, &ScanResult::default(), &CleanOptions { dry_run: true }, &NoProgress).unwrap();
        assert_eq!(rep.bytes_freed, 5);
        assert!(f.exists());
        assert!(sys.calls().is_empty());
    }

    #[test]
    fn metadata_matches_spec_table() {
        assert_eq!((WindowsOld.id(), WindowsOld.risk(), WindowsOld.default_selected()), ("windows_old", RiskLevel::Risky, false));
    }
}
```

- [ ] **Step 6: Chạy test, thấy hỏng**

Run: `cargo test -p winfreeup-core cleaners::wu_download`
Expected: FAIL biên dịch — `cannot find value WuDownload`, `cannot find value WindowsOld`.

- [ ] **Step 7: Viết mã (đặt TRÊN khối test)**

`wu_download.rs`:

```rust
use std::path::PathBuf;

use crate::env::Env;
use crate::error::Result;
use crate::fsclean::{clean_targets, roots_of, scan_targets, Target};
use crate::service::ServiceGuard;
use crate::types::{CancelToken, CleanOptions, CleanReport, Cleaner, Progress, RiskLevel, ScanResult};

pub const SERVICE: &str = "wuauserv";

pub fn targets(env: &Env) -> Vec<Target> {
    vec![Target::all(env.windir.join(r"SoftwareDistribution\Download"))]
}

pub struct WuDownload;

impl Cleaner for WuDownload {
    fn id(&self) -> &'static str {
        "wu_download"
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Caution
    }
    fn default_selected(&self) -> bool {
        false
    }
    fn allowed_roots(&self, env: &Env) -> Vec<PathBuf> {
        roots_of(&targets(env))
    }
    fn scan(&self, env: &Env, cancel: &CancelToken) -> Result<ScanResult> {
        scan_targets(&targets(env), env.now, cancel)
    }
    fn clean(&self, env: &Env, _scan: &ScanResult, opts: &CleanOptions, progress: &dyn Progress) -> Result<CleanReport> {
        if opts.dry_run {
            return Ok(clean_targets(&targets(env), env.now, opts, progress));
        }
        let guard = ServiceGuard::stop(env.sys.as_ref(), SERVICE)?;
        let mut report = clean_targets(&targets(env), env.now, opts, progress);
        if let Err(e) = guard.finish() {
            report.errors.push(format!("could not restart {SERVICE}: {e}"));
        }
        Ok(report)
    }
}
```

`windows_old.rs`:

```rust
use std::path::PathBuf;

use crate::env::Env;
use crate::error::Result;
use crate::fsclean::{clean_targets, scan_targets, Target};
use crate::types::{CancelToken, CleanOptions, CleanReport, Cleaner, Progress, RiskLevel, ScanResult};

pub fn target(env: &Env) -> Target {
    Target { remove_root: true, ..Target::all(env.system_drive.join("Windows.old")) }
}

pub struct WindowsOld;

impl Cleaner for WindowsOld {
    fn id(&self) -> &'static str {
        "windows_old"
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Risky
    }
    fn default_selected(&self) -> bool {
        false
    }
    fn allowed_roots(&self, env: &Env) -> Vec<PathBuf> {
        vec![target(env).root]
    }
    fn scan(&self, env: &Env, cancel: &CancelToken) -> Result<ScanResult> {
        scan_targets(&[target(env)], env.now, cancel)
    }
    fn clean(&self, env: &Env, _scan: &ScanResult, opts: &CleanOptions, progress: &dyn Progress) -> Result<CleanReport> {
        let t = target(env);
        let mut pre_errors = Vec::new();
        if !opts.dry_run && t.root.exists() {
            if let Err(e) = env.sys.take_ownership(&t.root) {
                pre_errors.push(e.to_string());
            }
        }
        let mut report = clean_targets(&[t], env.now, opts, progress);
        report.errors.splice(0..0, pre_errors);
        Ok(report)
    }
}
```

- [ ] **Step 8: Chạy test, thấy qua**

Run: `cargo test -p winfreeup-core cleaners::wu_download`, `cargo test -p winfreeup-core cleaners::windows_old`, rồi `cargo test -p winfreeup-core`
Expected: `4 passed`; `4 passed`; toàn bộ `0 failed; 0 ignored`.

- [ ] **Step 9: Commit**

```bash
rtk git add crates/winfreeup-core/src/service.rs
rtk git add crates/winfreeup-core/src/cleaners/wu_download.rs
rtk git add crates/winfreeup-core/src/cleaners/windows_old.rs
rtk git commit -m "feat(core): ServiceGuard RAII, nhóm wu_download và windows_old

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Nhóm `component_store` (DISM) và `recycle_bin`

**Files:**
- Modify (thay dòng khung từ Task 1): `crates/winfreeup-core/src/cleaners/component_store.rs`
- Modify (thay dòng khung): `crates/winfreeup-core/src/cleaners/recycle_bin.rs`
- Test: trong từng file (module `tests`)

**Interfaces:**
- Consumes (chỉ Task 1 — task này KHÔNG cần fsclean): `SystemOps::run_dism(&self, args: &[&str], cancel: &CancelToken, on_line: &mut dyn FnMut(&str)) -> Result<()>`, `recycle_bin_size(&self) -> Result<(u64, u64)>`, `empty_recycle_bin(&self) -> Result<()>`; `Cleaner`, `ScanResult { estimated, .. }`, `CleanReport`, `ItemAction`, `Progress`, `CoreError::System`; `testutil::{FakeSys { dism_output, dism_fails, recycle, .. }, Recorder { percents, .. }, fake_env}` — FakeSys ghi `"dism:<args>"`, `"rb_size"`, `"rb_empty"`.
- Produces:
  - `cleaners::component_store::{ComponentStore, ANALYZE_ARGS, CLEANUP_ARGS, parse_size(&str) -> Option<u64>, parse_analyze(&str) -> Option<u64>, parse_percent(&str) -> Option<f32>}` — id `"component_store"`, Caution, không tích sẵn, `ScanResult.estimated = true`.
  - `cleaners::recycle_bin::RecycleBin` — id `"recycle_bin"`, Caution, không tích sẵn.

- [ ] **Step 1: Viết test hỏng**

Thay dòng khung của `component_store.rs` bằng phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{fake_env, FakeSys, Recorder};
    use crate::types::NoProgress;
    use std::sync::Arc;

    const SAMPLE: &str = "Deployment Image Servicing and Management tool\r\nVersion: 10.0.26100.1\r\n\r\nImage Version: 10.0.26100.1742\r\n\r\n[==========================100.0%==========================]\r\nComponent Store (WinSxS) information:\r\n\r\nWindows Explorer Reported Size of Component Store : 8.21 GB\r\n\r\nActual Size of Component Store : 7.95 GB\r\n\r\n    Shared with Windows : 5.72 GB\r\n    Backups and Disabled Features : 2.05 GB\r\n    Cache and Temporary Data :  176.54 MB\r\n\r\nDate of Last Cleanup : 2026-08-10 12:30:12\r\n\r\nNumber of Reclaimable Packages : 3\r\nComponent Store Cleanup Recommended : Yes\r\n\r\nThe operation completed successfully.\r\n";

    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MB: f64 = 1024.0 * 1024.0;

    #[test]
    fn parses_sizes_with_units() {
        assert_eq!(parse_size("2.05 GB"), Some((2.05 * GB) as u64));
        assert_eq!(parse_size(" 176.54 MB"), Some((176.54 * MB) as u64));
        assert_eq!(parse_size("512 bytes"), Some(512));
        assert_eq!(parse_size("abc"), None);
    }

    #[test]
    fn estimate_is_backups_plus_cache_when_recommended() {
        assert_eq!(parse_analyze(SAMPLE), Some((2.05 * GB) as u64 + (176.54 * MB) as u64));
    }

    #[test]
    fn estimate_is_cache_only_when_not_recommended() {
        let s = SAMPLE.replace("Recommended : Yes", "Recommended : No");
        assert_eq!(parse_analyze(&s), Some((176.54 * MB) as u64));
    }

    #[test]
    fn unrecognised_output_is_none() {
        assert_eq!(parse_analyze("Error: 740\r\nElevated permissions are required"), None);
    }

    #[test]
    fn parses_progress_percent() {
        assert_eq!(parse_percent("[=====                      10.0%                          ]"), Some(10.0));
        assert_eq!(parse_percent("[==========================100.0%==========================]"), Some(100.0));
        assert_eq!(parse_percent("The operation completed successfully."), None);
    }

    #[test]
    fn scan_is_an_estimate_from_analyze() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys { dism_output: SAMPLE.into(), ..Default::default() });
        let env = fake_env(t.path(), sys.clone());
        let r = ComponentStore.scan(&env, &CancelToken::new()).unwrap();
        assert!(r.estimated);
        assert!(r.total_bytes > 0);
        assert_eq!(sys.calls(), vec![format!("dism:{}", ANALYZE_ARGS.join(" "))]);
    }

    #[test]
    fn scan_fails_loudly_on_unrecognised_output() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys { dism_output: "garbage".into(), ..Default::default() }));
        assert!(ComponentStore.scan(&env, &CancelToken::new()).is_err());
    }

    #[test]
    fn clean_reports_percent_and_never_uses_resetbase() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys { dism_output: "[==  10.0%  ]\r[======  55.5%  ]\rThe operation completed successfully.".into(), ..Default::default() });
        let env = fake_env(t.path(), sys.clone());
        let rec = Recorder::default();
        let scan = ScanResult { total_bytes: 1000, estimated: true, ..Default::default() };
        let rep = ComponentStore.clean(&env, &scan, &CleanOptions::default(), &rec).unwrap();
        assert_eq!(rep.bytes_freed, 1000);
        assert_eq!(*rec.percents.lock().unwrap(), vec![10.0, 55.5, 100.0]);
        let calls = sys.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].contains("/StartComponentCleanup"));
        assert!(!calls.iter().any(|c| c.to_lowercase().contains("resetbase")));
        assert!(!CLEANUP_ARGS.iter().chain(ANALYZE_ARGS).any(|a| a.to_lowercase().contains("resetbase")));
    }

    #[test]
    fn dry_run_never_calls_dism() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys::default());
        let env = fake_env(t.path(), sys.clone());
        let scan = ScanResult { total_bytes: 7, ..Default::default() };
        let rep = ComponentStore.clean(&env, &scan, &CleanOptions { dry_run: true }, &NoProgress).unwrap();
        assert_eq!((rep.bytes_freed, rep.dry_run), (7, true));
        assert!(sys.calls().is_empty());
    }

    #[test]
    fn dism_failure_is_an_error_for_the_group() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys { dism_fails: true, ..Default::default() }));
        assert!(ComponentStore.clean(&env, &ScanResult::default(), &CleanOptions::default(), &NoProgress).is_err());
    }
}
```

Thay dòng khung của `recycle_bin.rs` bằng phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{fake_env, FakeSys};
    use crate::types::NoProgress;
    use std::sync::Arc;

    #[test]
    fn scan_reads_size_and_count() {
        let t = tempfile::tempdir().unwrap();
        let env = fake_env(t.path(), Arc::new(FakeSys { recycle: (5000, 3), ..Default::default() }));
        let r = RecycleBin.scan(&env, &CancelToken::new()).unwrap();
        assert_eq!((r.total_bytes, r.file_count), (5000, 3));
    }

    #[test]
    fn clean_empties_the_bin() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys::default());
        let env = fake_env(t.path(), sys.clone());
        let scan = ScanResult { total_bytes: 5000, file_count: 3, ..Default::default() };
        let rep = RecycleBin.clean(&env, &scan, &CleanOptions::default(), &NoProgress).unwrap();
        assert_eq!((rep.bytes_freed, rep.files_deleted), (5000, 3));
        assert_eq!(sys.calls(), vec!["rb_empty"]);
    }

    #[test]
    fn dry_run_does_not_empty() {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(FakeSys::default());
        let env = fake_env(t.path(), sys.clone());
        let scan = ScanResult { total_bytes: 10, file_count: 1, ..Default::default() };
        let rep = RecycleBin.clean(&env, &scan, &CleanOptions { dry_run: true }, &NoProgress).unwrap();
        assert_eq!(rep.bytes_freed, 10);
        assert!(sys.calls().is_empty());
    }

    #[test]
    fn metadata_matches_spec_table() {
        assert_eq!((RecycleBin.id(), RecycleBin.risk(), RecycleBin.default_selected()), ("recycle_bin", RiskLevel::Caution, false));
        assert!(RecycleBin.allowed_roots(&fake_env(tempfile::tempdir().unwrap().path(), Arc::new(FakeSys::default()))).is_empty());
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test -p winfreeup-core cleaners::component_store` rồi `cargo test -p winfreeup-core cleaners::recycle_bin`
Expected: FAIL biên dịch — `cannot find function parse_size`, `cannot find value RecycleBin`.

- [ ] **Step 3: Viết mã (đặt TRÊN khối test)**

`component_store.rs`:

```rust
//! DISM /StartComponentCleanup. CẤM /ResetBase. Dung lượng quét là ước tính từ /AnalyzeComponentStore.
use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::error::{CoreError, Result};
use crate::types::{CancelToken, CleanOptions, CleanReport, Cleaner, ItemAction, Progress, RiskLevel, ScanResult};

/// `/English` để đầu ra không bị dịch theo ngôn ngữ Windows.
pub const ANALYZE_ARGS: &[&str] = &["/Online", "/English", "/Cleanup-Image", "/AnalyzeComponentStore"];
pub const CLEANUP_ARGS: &[&str] = &["/Online", "/English", "/Cleanup-Image", "/StartComponentCleanup"];
const LOG_LABEL: &str = "DISM /StartComponentCleanup";

pub fn parse_size(s: &str) -> Option<u64> {
    let mut parts = s.split_whitespace();
    let n: f64 = parts.next()?.parse().ok()?;
    let mult: f64 = match parts.next()?.to_ascii_lowercase().as_str() {
        "bytes" | "byte" => 1.0,
        "kb" => 1024.0,
        "mb" => 1024.0 * 1024.0,
        "gb" => 1024.0 * 1024.0 * 1024.0,
        "tb" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((n * mult) as u64)
}

pub fn parse_analyze(out: &str) -> Option<u64> {
    let (mut backups, mut cache, mut recommended) = (None, None, false);
    for line in out.lines() {
        let Some((k, v)) = line.split_once(" : ") else { continue };
        match k.trim() {
            "Backups and Disabled Features" => backups = parse_size(v),
            "Cache and Temporary Data" => cache = parse_size(v),
            "Component Store Cleanup Recommended" => recommended = v.trim().eq_ignore_ascii_case("yes"),
            _ => {}
        }
    }
    let (backups, cache) = (backups?, cache?);
    Some(if recommended { backups + cache } else { cache })
}

pub fn parse_percent(line: &str) -> Option<f32> {
    let end = line.find('%')?;
    let start = line[..end]
        .rfind(|c: char| !(c.is_ascii_digit() || c == '.'))
        .map(|i| i + 1)
        .unwrap_or(0);
    line[start..end].parse().ok()
}

pub struct ComponentStore;

impl Cleaner for ComponentStore {
    fn id(&self) -> &'static str {
        "component_store"
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Caution
    }
    fn default_selected(&self) -> bool {
        false
    }
    fn allowed_roots(&self, _env: &Env) -> Vec<PathBuf> {
        vec![]
    }
    fn scan(&self, env: &Env, cancel: &CancelToken) -> Result<ScanResult> {
        let mut out = String::new();
        env.sys.run_dism(ANALYZE_ARGS, cancel, &mut |l| {
            out.push_str(l);
            out.push('\n');
        })?;
        let bytes = parse_analyze(&out)
            .ok_or_else(|| CoreError::System("unrecognized DISM /AnalyzeComponentStore output".into()))?;
        Ok(ScanResult { total_bytes: bytes, estimated: true, ..Default::default() })
    }
    fn clean(&self, env: &Env, scan: &ScanResult, opts: &CleanOptions, progress: &dyn Progress) -> Result<CleanReport> {
        if opts.dry_run {
            progress.item(ItemAction::WouldDelete, Path::new(LOG_LABEL), scan.total_bytes, None);
            return Ok(CleanReport { bytes_freed: scan.total_bytes, dry_run: true, ..Default::default() });
        }
        let never = CancelToken::new();
        env.sys.run_dism(CLEANUP_ARGS, &never, &mut |l| {
            if let Some(p) = parse_percent(l) {
                progress.percent(p);
            }
        })?;
        progress.percent(100.0);
        progress.item(ItemAction::Deleted, Path::new(LOG_LABEL), scan.total_bytes, None);
        Ok(CleanReport { bytes_freed: scan.total_bytes, ..Default::default() })
    }
}
```

Lưu ý test `clean_reports_percent_and_never_uses_resetbase` mong `[10.0, 55.5, 100.0]`: dòng mẫu không có `100.0%` nên phần tử 100.0 đến từ `progress.percent(100.0)` cuối hàm.

`recycle_bin.rs`:

```rust
//! Thùng rác mọi ổ qua SHEmptyRecycleBinW (trong RealSystem). Không lấy lại được.
use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::error::Result;
use crate::types::{CancelToken, CleanOptions, CleanReport, Cleaner, ItemAction, Progress, RiskLevel, ScanResult};

const LOG_LABEL: &str = "Recycle Bin";

pub struct RecycleBin;

impl Cleaner for RecycleBin {
    fn id(&self) -> &'static str {
        "recycle_bin"
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Caution
    }
    fn default_selected(&self) -> bool {
        false
    }
    fn allowed_roots(&self, _env: &Env) -> Vec<PathBuf> {
        vec![]
    }
    fn scan(&self, env: &Env, _cancel: &CancelToken) -> Result<ScanResult> {
        let (bytes, items) = env.sys.recycle_bin_size()?;
        Ok(ScanResult { total_bytes: bytes, file_count: items, ..Default::default() })
    }
    fn clean(&self, env: &Env, scan: &ScanResult, opts: &CleanOptions, progress: &dyn Progress) -> Result<CleanReport> {
        let report = CleanReport {
            bytes_freed: scan.total_bytes,
            files_deleted: scan.file_count,
            dry_run: opts.dry_run,
            ..Default::default()
        };
        if opts.dry_run {
            progress.item(ItemAction::WouldDelete, Path::new(LOG_LABEL), scan.total_bytes, None);
            return Ok(report);
        }
        env.sys.empty_recycle_bin()?;
        progress.item(ItemAction::Deleted, Path::new(LOG_LABEL), scan.total_bytes, None);
        Ok(report)
    }
}
```

- [ ] **Step 4: Chạy test, thấy qua**

Run: `cargo test -p winfreeup-core cleaners::component_store` rồi `cargo test -p winfreeup-core cleaners::recycle_bin`
Expected: `10 passed`; `4 passed`; 0 failed.

- [ ] **Step 5: Commit**

```bash
rtk git add crates/winfreeup-core/src/cleaners/component_store.rs
rtk git add crates/winfreeup-core/src/cleaners/recycle_bin.rs
rtk git commit -m "feat(core): nhóm component_store (DISM, cấm ResetBase) và recycle_bin

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 8: Nhật ký và bộ điều phối quét/dọn

**Files:**
- Modify (thay dòng khung từ Task 1): `crates/winfreeup-core/src/log.rs`
- Modify (thay dòng khung): `crates/winfreeup-core/src/engine.rs`
- Test: `log.rs`, `engine.rs` (module `tests`)

**Interfaces:**
- Consumes (chỉ Task 1 — task này KHÔNG cần các nhóm dọn thật, test dùng `FakeCleaner`): `Cleaner` (chữ ký đầy đủ ở Task 1), `SystemOps::create_restore_point(&self, description: &str) -> Result<()>`, `Progress`, `ItemAction::as_str`, `CleanOptions`, `CancelToken`, `ScanResult`, `CleanReport`, `RiskLevel`, `io_err`, `testutil::{FakeCleaner, FakeCleaner::ok, FakeSys { restore_error, .. }, fake_env}`. Crate `chrono` (feature `clock`) đã khai báo ở Task 1.
- Produces:
  - `log::log_dir(local_appdata: &Path) -> PathBuf` (= `…\WinFreeUp\logs`), `log::log_file_name(at: DateTime<Local>) -> String` (= `YYYY-MM-DD_HHmmss.log`), `log::CleanLog` + `create(dir: &Path, at: DateTime<Local>) -> Result<CleanLog>`, `path(&self) -> &Path`, `line(&self, text: &str)`, `write_failed(&self) -> bool`.
  - `engine::GroupScan { id: String, risk: RiskLevel, default_selected: bool, result: Option<ScanResult>, error: Option<String> }` (Serialize, Clone)
  - `engine::GroupClean { id: String, report: Option<CleanReport>, error: Option<String> }` (Serialize, Clone)
  - `engine::CleanEvent` — serde `{"kind":"started","id"}` | `{"kind":"percent","id","percent"}` | `{"kind":"finished","result":GroupClean}`
  - `engine::RestorePointStatus` — serde `{"status":"not_needed"}` | `{"status":"created"}` | `{"status":"skipped"}` | `{"status":"failed","message"}`
  - `engine::CleanSummary { groups: Vec<GroupClean>, log_path: String, dry_run: bool, log_write_failed: bool }`
  - `engine::scan_all(cleaners: &[Box<dyn Cleaner>], env: &Env, cancel: &CancelToken, on_done: &(dyn Fn(&GroupScan) + Sync)) -> Vec<GroupScan>` — song song, giữ thứ tự, lỗi/panic của một nhóm không làm hỏng nhóm khác.
  - `engine::needs_restore_point(cleaners: &[Box<dyn Cleaner>], ids: &[String]) -> bool` — true khi có id (khác `"recycle_bin"`) thuộc nhóm có `risk() != Safe`; id lạ bị bỏ qua.
  - `engine::prepare_restore_point(cleaners: &[Box<dyn Cleaner>], env: &Env, ids: &[String], dry_run: bool) -> RestorePointStatus`
  - `engine::run_clean(cleaners: &[Box<dyn Cleaner>], env: &Env, ids: &[String], scans: &HashMap<String, ScanResult>, opts: &CleanOptions, log_dir: &Path, on_event: &(dyn Fn(CleanEvent) + Sync)) -> Result<CleanSummary>` — tuần tự, ghi nhật ký từng đường dẫn.

- [ ] **Step 1: Viết test hỏng cho nhật ký**

Thay dòng khung của `log.rs` bằng phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn log_dir_and_file_name_follow_spec() {
        assert_eq!(log_dir(Path::new(r"C:\Users\a\AppData\Local")), PathBuf::from(r"C:\Users\a\AppData\Local\WinFreeUp\logs"));
        let at = Local.with_ymd_and_hms(2026, 9, 5, 7, 3, 9).unwrap();
        assert_eq!(log_file_name(at), "2026-09-05_070309.log");
    }

    #[test]
    fn creates_dir_and_appends_timestamped_lines() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("WinFreeUp").join("logs");
        let log = CleanLog::create(&dir, Local::now()).unwrap();
        log.line("START dry_run=false ids=user_temp");
        log.line("[user_temp] DELETED 10 C:\\Temp\\tệp.tmp");
        let text = std::fs::read_to_string(log.path()).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with("[user_temp] DELETED 10 C:\\Temp\\tệp.tmp"));
        assert!(!log.write_failed());
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test -p winfreeup-core log::`
Expected: FAIL biên dịch — `cannot find function log_dir`.

- [ ] **Step 3: Viết `log.rs` (trên khối test)**

```rust
//! Nhật ký mỗi lượt dọn: %LOCALAPPDATA%\WinFreeUp\logs\YYYY-MM-DD_HHmmss.log (UTF-8).
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use chrono::{DateTime, Local};

use crate::error::{io_err, Result};

pub fn log_dir(local_appdata: &Path) -> PathBuf {
    local_appdata.join("WinFreeUp").join("logs")
}

pub fn log_file_name(at: DateTime<Local>) -> String {
    at.format("%Y-%m-%d_%H%M%S.log").to_string()
}

pub struct CleanLog {
    path: PathBuf,
    file: Mutex<File>,
    failed: AtomicBool,
}

impl CleanLog {
    pub fn create(dir: &Path, at: DateTime<Local>) -> Result<CleanLog> {
        fs::create_dir_all(dir).map_err(|e| io_err(dir, e))?;
        let path = dir.join(log_file_name(at));
        let file = OpenOptions::new().create(true).append(true).open(&path).map_err(|e| io_err(&path, e))?;
        Ok(CleanLog { path, file: Mutex::new(file), failed: AtomicBool::new(false) })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn line(&self, text: &str) {
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
        let ok = match self.file.lock() {
            Ok(mut f) => writeln!(f, "{ts} {text}").and_then(|_| f.flush()).is_ok(),
            Err(_) => false,
        };
        if !ok {
            self.failed.store(true, Ordering::SeqCst);
        }
    }

    /// true nếu có ít nhất một dòng không ghi được (đĩa đầy, bị khóa…) — giao diện phải báo.
    pub fn write_failed(&self) -> bool {
        self.failed.load(Ordering::SeqCst)
    }
}
```

- [ ] **Step 4: Chạy test, thấy qua**

Run: `cargo test -p winfreeup-core log::`
Expected: `2 passed`.

- [ ] **Step 5: Viết test hỏng cho `engine.rs`**

Thay dòng khung của `engine.rs` bằng phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{fake_env, FakeCleaner, FakeSys};
    use crate::types::RiskLevel::*;
    use std::sync::{Arc, Mutex};

    fn env_with(sys: FakeSys) -> (tempfile::TempDir, Arc<FakeSys>, Env) {
        let t = tempfile::tempdir().unwrap();
        let sys = Arc::new(sys);
        let env = fake_env(t.path(), sys.clone());
        (t, sys, env)
    }

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn scan_all_reports_every_group_in_order_and_streams_each() {
        let (_t, _s, env) = env_with(FakeSys::default());
        let cs = vec![FakeCleaner::ok("a", Safe, 1), FakeCleaner::ok("b", Caution, 2), FakeCleaner::ok("c", Risky, 3)];
        let seen = Mutex::new(Vec::new());
        let out = scan_all(&cs, &env, &CancelToken::new(), &|g| seen.lock().unwrap().push(g.id.clone()));
        assert_eq!(out.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(), vec!["a", "b", "c"]);
        assert_eq!(out[1].result.as_ref().unwrap().total_bytes, 2);
        assert!(out[0].default_selected && !out[1].default_selected);
        let mut s = seen.into_inner().unwrap();
        s.sort();
        assert_eq!(s, vec!["a", "b", "c"]);
    }

    #[test]
    fn one_failing_or_panicking_group_does_not_break_the_others() {
        let (_t, _s, env) = env_with(FakeSys::default());
        let cs: Vec<Box<dyn Cleaner>> = vec![
            FakeCleaner::ok("a", Safe, 1),
            Box::new(FakeCleaner { id: "b", risk: Safe, bytes: 0, fail: Some("disk error"), panics: false }),
            Box::new(FakeCleaner { id: "c", risk: Safe, bytes: 0, fail: None, panics: true }),
        ];
        let out = scan_all(&cs, &env, &CancelToken::new(), &|_| {});
        assert!(out[0].result.is_some() && out[0].error.is_none());
        assert_eq!(out[1].error.as_deref(), Some("disk error"));
        assert!(out[2].error.as_deref().unwrap().contains("boom in c"));
    }

    #[test]
    fn cancelled_scan_marks_groups_cancelled() {
        let (_t, _s, env) = env_with(FakeSys::default());
        let c = CancelToken::new();
        c.cancel();
        let out = scan_all(&[FakeCleaner::ok("a", Safe, 1)], &env, &c, &|_| {});
        assert_eq!(out[0].error.as_deref(), Some("cancelled"));
    }

    fn table() -> Vec<Box<dyn Cleaner>> {
        vec![
            FakeCleaner::ok("user_temp", Safe, 1),
            FakeCleaner::ok("browser_cache", Safe, 1),
            FakeCleaner::ok("recycle_bin", Caution, 1),
            FakeCleaner::ok("wu_download", Caution, 1),
            FakeCleaner::ok("windows_old", Risky, 1),
        ]
    }

    #[test]
    fn restore_point_needed_only_for_caution_or_risky_except_recycle_bin() {
        let cs = table();
        assert!(!needs_restore_point(&cs, &ids(&["user_temp", "browser_cache"])));
        assert!(!needs_restore_point(&cs, &ids(&["recycle_bin"])));
        assert!(!needs_restore_point(&cs, &ids(&["recycle_bin", "user_temp"])));
        assert!(needs_restore_point(&cs, &ids(&["wu_download"])));
        assert!(needs_restore_point(&cs, &ids(&["windows_old"])));
        assert!(!needs_restore_point(&cs, &ids(&["khong_co"])));
    }

    #[test]
    fn prepare_restore_point_outcomes() {
        let cs = table();
        let (_t, sys, env) = env_with(FakeSys::default());
        assert_eq!(prepare_restore_point(&cs, &env, &ids(&["user_temp"]), false), RestorePointStatus::NotNeeded);
        assert_eq!(prepare_restore_point(&cs, &env, &ids(&["wu_download"]), true), RestorePointStatus::Skipped);
        assert!(sys.calls().is_empty(), "không tạo điểm khôi phục khi chạy thử");
        assert_eq!(prepare_restore_point(&cs, &env, &ids(&["wu_download"]), false), RestorePointStatus::Created);
        let (_t2, _s2, env2) = env_with(FakeSys { restore_error: Some("System Protection is off".into()), ..Default::default() });
        assert_eq!(
            prepare_restore_point(&cs, &env2, &ids(&["windows_old"]), false),
            RestorePointStatus::Failed { message: "System Protection is off".into() }
        );
    }

    #[test]
    fn run_clean_logs_every_group_emits_events_and_isolates_failures() {
        let (t, _s, env) = env_with(FakeSys::default());
        let cs: Vec<Box<dyn Cleaner>> = vec![
            FakeCleaner::ok("a", Safe, 10),
            Box::new(FakeCleaner { id: "b", risk: Safe, bytes: 0, fail: None, panics: true }),
        ];
        let events = Mutex::new(Vec::new());
        let dir = t.path().join("logs");
        let s = run_clean(&cs, &env, &ids(&["a", "b", "zzz", "a"]), &HashMap::new(), &CleanOptions { dry_run: true }, &dir, &|e| {
            events.lock().unwrap().push(serde_json::to_value(&e).unwrap())
        })
        .unwrap();
        assert!(s.dry_run);
        assert_eq!(s.groups.len(), 3, "trùng id chỉ dọn một lần");
        assert_eq!(s.groups[0].report.as_ref().unwrap().bytes_freed, 10);
        assert!(s.groups[1].error.as_deref().unwrap().contains("boom in b"));
        assert_eq!(s.groups[2].error.as_deref(), Some("unknown cleaner id: zzz"));
        let kinds: Vec<String> = events.lock().unwrap().iter().map(|v| v["kind"].as_str().unwrap().to_string()).collect();
        assert_eq!(kinds, vec!["started", "percent", "finished", "started", "finished", "started", "finished"]);
        let log = std::fs::read_to_string(&s.log_path).unwrap();
        assert!(log.contains("START dry_run=true"));
        assert!(log.contains("[a] DELETED 10 a"));
        assert!(log.contains("GROUP b ERROR"));
        assert!(log.contains("END"));
    }

    #[test]
    fn events_serialize_to_the_shape_the_ui_expects() {
        let v = serde_json::to_value(CleanEvent::Percent { id: "component_store".into(), percent: 42.5 }).unwrap();
        assert_eq!(v, serde_json::json!({"kind": "percent", "id": "component_store", "percent": 42.5}));
        let v = serde_json::to_value(RestorePointStatus::Failed { message: "x".into() }).unwrap();
        assert_eq!(v, serde_json::json!({"status": "failed", "message": "x"}));
        let v = serde_json::to_value(RestorePointStatus::NotNeeded).unwrap();
        assert_eq!(v, serde_json::json!({"status": "not_needed"}));
    }
}
```

- [ ] **Step 6: Chạy test, thấy hỏng**

Run: `cargo test -p winfreeup-core engine`
Expected: FAIL biên dịch — `cannot find function scan_all`, `cannot find type RestorePointStatus`.

- [ ] **Step 7: Viết `engine.rs` (trên khối test)**

```rust
//! Điều phối: quét song song, quyết định điểm khôi phục, dọn tuần tự có nhật ký và sự kiện.
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use chrono::Local;
use serde::Serialize;

use crate::env::Env;
use crate::error::Result;
use crate::log::CleanLog;
use crate::types::{CancelToken, CleanOptions, CleanReport, Cleaner, ItemAction, Progress, RiskLevel, ScanResult};

#[derive(Debug, Clone, Serialize)]
pub struct GroupScan {
    pub id: String,
    pub risk: RiskLevel,
    pub default_selected: bool,
    pub result: Option<ScanResult>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupClean {
    pub id: String,
    pub report: Option<CleanReport>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CleanEvent {
    Started { id: String },
    Percent { id: String, percent: f32 },
    Finished { result: GroupClean },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RestorePointStatus {
    NotNeeded,
    Created,
    Skipped,
    Failed { message: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanSummary {
    pub groups: Vec<GroupClean>,
    pub log_path: String,
    pub dry_run: bool,
    pub log_write_failed: bool,
}

fn panic_message(p: &(dyn Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        format!("panic: {s}")
    } else if let Some(s) = p.downcast_ref::<String>() {
        format!("panic: {s}")
    } else {
        "panic".to_string()
    }
}

pub fn scan_all(
    cleaners: &[Box<dyn Cleaner>],
    env: &Env,
    cancel: &CancelToken,
    on_done: &(dyn Fn(&GroupScan) + Sync),
) -> Vec<GroupScan> {
    std::thread::scope(|s| {
        let handles: Vec<_> = cleaners
            .iter()
            .map(|c| {
                s.spawn(move || {
                    let outcome = catch_unwind(AssertUnwindSafe(|| c.scan(env, cancel)));
                    let (result, error) = match outcome {
                        Ok(Ok(r)) => (Some(r), None),
                        Ok(Err(e)) => (None, Some(e.to_string())),
                        Err(p) => (None, Some(panic_message(p.as_ref()))),
                    };
                    let g = GroupScan {
                        id: c.id().to_string(),
                        risk: c.risk(),
                        default_selected: c.default_selected(),
                        result,
                        error,
                    };
                    on_done(&g);
                    g
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("scan thread")).collect()
    })
}

pub fn needs_restore_point(cleaners: &[Box<dyn Cleaner>], ids: &[String]) -> bool {
    ids.iter().any(|id| {
        id != "recycle_bin"
            && cleaners.iter().find(|c| c.id() == id).is_some_and(|c| c.risk() != RiskLevel::Safe)
    })
}

pub fn prepare_restore_point(
    cleaners: &[Box<dyn Cleaner>],
    env: &Env,
    ids: &[String],
    dry_run: bool,
) -> RestorePointStatus {
    if !needs_restore_point(cleaners, ids) {
        return RestorePointStatus::NotNeeded;
    }
    if dry_run {
        return RestorePointStatus::Skipped;
    }
    match env.sys.create_restore_point("WinFreeUp") {
        Ok(()) => RestorePointStatus::Created,
        Err(e) => RestorePointStatus::Failed { message: e.to_string() },
    }
}

struct GroupProgress<'a> {
    log: &'a CleanLog,
    id: &'a str,
    on_event: &'a (dyn Fn(CleanEvent) + Sync),
}

impl Progress for GroupProgress<'_> {
    fn item(&self, action: ItemAction, path: &Path, bytes: u64, detail: Option<&str>) {
        let tail = detail.map(|d| format!(" -- {d}")).unwrap_or_default();
        self.log.line(&format!("[{}] {} {} {}{}", self.id, action.as_str(), bytes, path.display(), tail));
    }
    fn percent(&self, pct: f32) {
        (self.on_event)(CleanEvent::Percent { id: self.id.to_string(), percent: pct });
    }
}

pub fn run_clean(
    cleaners: &[Box<dyn Cleaner>],
    env: &Env,
    ids: &[String],
    scans: &HashMap<String, ScanResult>,
    opts: &CleanOptions,
    log_dir: &Path,
    on_event: &(dyn Fn(CleanEvent) + Sync),
) -> Result<CleanSummary> {
    let log = CleanLog::create(log_dir, Local::now())?;
    log.line(&format!("START dry_run={} ids={}", opts.dry_run, ids.join(",")));
    let mut seen = HashSet::new();
    let mut groups = Vec::new();
    let empty = ScanResult::default();
    for id in ids.iter().filter(|id| seen.insert(id.as_str())) {
        on_event(CleanEvent::Started { id: id.clone() });
        log.line(&format!("GROUP {id} START"));
        let g = match cleaners.iter().find(|c| c.id() == id) {
            None => GroupClean { id: id.clone(), report: None, error: Some(format!("unknown cleaner id: {id}")) },
            Some(c) => {
                let progress = GroupProgress { log: &log, id: id.as_str(), on_event };
                let scan = scans.get(id).unwrap_or(&empty);
                match catch_unwind(AssertUnwindSafe(|| c.clean(env, scan, opts, &progress))) {
                    Ok(Ok(r)) => GroupClean { id: id.clone(), report: Some(r), error: None },
                    Ok(Err(e)) => GroupClean { id: id.clone(), report: None, error: Some(e.to_string()) },
                    Err(p) => GroupClean { id: id.clone(), report: None, error: Some(panic_message(p.as_ref())) },
                }
            }
        };
        match (&g.report, &g.error) {
            (Some(r), _) => {
                log.line(&format!(
                    "GROUP {id} END bytes={} deleted={} skipped_locked={} errors={}",
                    r.bytes_freed,
                    r.files_deleted,
                    r.skipped_locked,
                    r.errors.len()
                ));
                for e in &r.errors {
                    log.line(&format!("GROUP {id} ITEM_ERROR {e}"));
                }
            }
            (None, Some(e)) => log.line(&format!("GROUP {id} ERROR {e}")),
            (None, None) => {}
        }
        on_event(CleanEvent::Finished { result: g.clone() });
        groups.push(g);
    }
    log.line("END");
    Ok(CleanSummary {
        groups,
        log_path: log.path().display().to_string(),
        dry_run: opts.dry_run,
        log_write_failed: log.write_failed(),
    })
}
```

- [ ] **Step 8: Chạy test, thấy qua**

Run: `cargo test -p winfreeup-core engine` rồi `cargo test -p winfreeup-core`
Expected: `7 passed`; toàn bộ `0 failed`. Dòng `thread '<unnamed>' panicked at … boom in …` trên stderr là mong đợi (test panic có chủ đích).

- [ ] **Step 9: Commit**

```bash
rtk git add crates/winfreeup-core/src/log.rs
rtk git add crates/winfreeup-core/src/engine.rs
rtk git commit -m "feat(core): nhật ký lượt dọn, điều phối quét/dọn và điểm khôi phục

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: `RealSystem` — lời gọi Windows thật

**Files:**
- Modify: `crates/winfreeup-core/src/sys_windows.rs` (thay dòng khung từ Task 1)
- Test: `crates/winfreeup-core/src/sys_windows.rs` (module `tests` — chỉ thao tác **đọc**, không sửa hệ thống)

**Interfaces:**
- Consumes (chỉ Task 1): trait `SystemOps` (8 hàm — chữ ký ở `env.rs` Task 1, cài đủ cả 8), `Env` (7 trường), `CancelToken::is_cancelled`, `CoreError::{System, Cancelled}`, `io_err`. Crate `windows-sys 0.61` với feature `Win32_Foundation`, `Win32_Storage_FileSystem`, `Win32_System_Diagnostics_ToolHelp`, `Win32_UI_Shell` đã khai báo ở Task 1.
- Produces:
  - `pub struct RealSystem { pub system32: PathBuf }` — `impl SystemOps`.
  - `impl Env { pub fn from_system() -> Result<Env> }`
  - `pub fn disk_free(path: &Path) -> Result<u64>` — byte trống khả dụng cho người gọi.
  - `pub fn split_lines(pending: &mut String, incoming: &str) -> Vec<String>` — tách theo `\r`/`\n`, giữ phần dở dang.

Phần này chạm hệ thống thật nên test tự động chỉ phủ hàm thuần và phép đọc; phần dừng/bật dịch vụ, DISM, Thùng rác, điểm khôi phục, `icacls` được thử tay ở Task 17 trong Windows Sandbox/VM.

- [ ] **Step 1: Viết test hỏng**

Thay dòng khung của `sys_windows.rs` bằng phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_lines_handles_carriage_returns_and_partial_chunks() {
        let mut pending = String::new();
        assert_eq!(split_lines(&mut pending, "[==  10.0%  ]\r[===  2"), vec!["[==  10.0%  ]"]);
        assert_eq!(split_lines(&mut pending, "0.0%  ]\r\nDone.\r\n"), vec!["[===  20.0%  ]", "Done."]);
        assert!(pending.is_empty());
    }

    #[test]
    fn disk_free_of_temp_dir_is_positive() {
        assert!(disk_free(&std::env::temp_dir()).unwrap() > 0);
    }

    #[test]
    fn disk_free_of_missing_drive_is_an_error() {
        assert!(disk_free(Path::new(r"\\?\Volume{00000000-0000-0000-0000-000000000000}\")).is_err());
    }

    #[test]
    fn env_from_system_points_at_existing_folders() {
        let env = Env::from_system().unwrap();
        for p in [&env.temp, &env.windir, &env.local_appdata, &env.program_data, &env.system_drive] {
            assert!(p.exists(), "{}", p.display());
        }
        assert!(env.system_drive.to_string_lossy().ends_with('\\'));
    }

    #[test]
    fn detects_the_current_test_process_but_not_a_made_up_one() {
        let sys = RealSystem { system32: PathBuf::from(r"C:\Windows\System32") };
        let me = std::env::current_exe().unwrap();
        let name = me.file_name().unwrap().to_string_lossy().to_string();
        assert!(sys.is_process_running(&name));
        assert!(!sys.is_process_running("khong-co-tien-trinh-nay-3f9a.exe"));
    }

    #[test]
    fn service_names_are_validated_before_reaching_powershell() {
        assert!(valid_service_name("wuauserv").is_ok());
        assert!(valid_service_name("x'; Remove-Item C:\\ -Recurse; '").is_err());
        assert!(valid_service_name("").is_err());
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test -p winfreeup-core sys_windows`
Expected: FAIL biên dịch — `cannot find function split_lines`, `cannot find struct RealSystem`.

- [ ] **Step 3: Viết mã (trên khối test)**

```rust
//! Cài đặt thật của SystemOps. Mọi tiến trình con chạy với CREATE_NO_WINDOW (không nháy cửa sổ đen).
use std::ffi::OsStr;
use std::io::Read;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc};
use std::time::{Duration, SystemTime};

use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::UI::Shell::{
    SHEmptyRecycleBinW, SHQueryRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHQUERYRBINFO,
};

use crate::env::{Env, SystemOps};
use crate::error::{io_err, CoreError, Result};
use crate::types::CancelToken;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// SHEmptyRecycleBinW trả E_UNEXPECTED khi thùng rác vốn đã trống.
const E_UNEXPECTED: i32 = 0x8000_FFFFu32 as i32;
/// DISM: 3010 = thành công, cần khởi động lại.
const DISM_OK_REBOOT: i32 = 3010;
const ADMINS_SID: &str = "*S-1-5-32-544";

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

pub fn disk_free(path: &Path) -> Result<u64> {
    let w = wide(path.as_os_str());
    let mut avail: u64 = 0;
    let ok = unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, std::ptr::null_mut(), std::ptr::null_mut()) };
    if ok == 0 {
        return Err(io_err(path, std::io::Error::last_os_error()));
    }
    Ok(avail)
}

pub fn split_lines(pending: &mut String, incoming: &str) -> Vec<String> {
    pending.push_str(incoming);
    let mut out = Vec::new();
    while let Some(i) = pending.find(['\r', '\n']) {
        let line: String = pending.drain(..=i).collect();
        let line = line.trim();
        if !line.is_empty() {
            out.push(line.to_string());
        }
    }
    out
}

pub(crate) fn valid_service_name(name: &str) -> Result<()> {
    if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Ok(())
    } else {
        Err(CoreError::System(format!("invalid service name: {name:?}")))
    }
}

fn run_capture(exe: &Path, args: &[&OsStr]) -> Result<String> {
    let out = Command::new(exe)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| CoreError::System(format!("{}: {e}", exe.display())))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        Err(CoreError::System(
            format!("{} exit {}: {} {}", exe.display(), out.status.code().unwrap_or(-1), stdout, stderr)
                .trim()
                .to_string(),
        ))
    }
}

pub struct RealSystem {
    pub system32: PathBuf,
}

impl RealSystem {
    fn powershell(&self, script: &str) -> Result<String> {
        let exe = self.system32.join(r"WindowsPowerShell\v1.0\powershell.exe");
        let args: Vec<&OsStr> = ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script]
            .into_iter()
            .map(OsStr::new)
            .collect();
        run_capture(&exe, &args)
    }
}

impl SystemOps for RealSystem {
    fn is_process_running(&self, exe_name: &str) -> bool {
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return false;
            }
            let mut e: PROCESSENTRY32W = std::mem::zeroed();
            e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut found = false;
            if Process32FirstW(snap, &mut e) != 0 {
                loop {
                    let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                    if String::from_utf16_lossy(&e.szExeFile[..len]).eq_ignore_ascii_case(exe_name) {
                        found = true;
                        break;
                    }
                    if Process32NextW(snap, &mut e) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snap);
            found
        }
    }

    fn stop_service(&self, name: &str) -> Result<bool> {
        valid_service_name(name)?;
        let out = self.powershell(&format!(
            "$s = Get-Service -Name '{name}' -ErrorAction Stop; if ($s.Status -eq 'Running') {{ Stop-Service -Name '{name}' -Force -ErrorAction Stop; 'STOPPED' }} else {{ 'ALREADY' }}"
        ))?;
        Ok(out.ends_with("STOPPED"))
    }

    fn start_service(&self, name: &str) -> Result<()> {
        valid_service_name(name)?;
        self.powershell(&format!("Start-Service -Name '{name}' -ErrorAction Stop")).map(|_| ())
    }

    fn run_dism(&self, args: &[&str], cancel: &CancelToken, on_line: &mut dyn FnMut(&str)) -> Result<()> {
        let exe = self.system32.join("Dism.exe");
        let mut child = Command::new(&exe)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| CoreError::System(format!("{}: {e}", exe.display())))?;
        let mut stdout = child.stdout.take().expect("stdout is piped");
        let (tx, rx) = mpsc::channel::<String>();
        let reader = std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut pending = String::new();
            loop {
                match stdout.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        for line in split_lines(&mut pending, &String::from_utf8_lossy(&buf[..n])) {
                            if tx.send(line).is_err() {
                                return;
                            }
                        }
                    }
                }
            }
            if !pending.trim().is_empty() {
                let _ = tx.send(pending.trim().to_string());
            }
        });
        let mut tail: Vec<String> = Vec::new();
        loop {
            if cancel.is_cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(CoreError::Cancelled);
            }
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(line) => {
                    on_line(&line);
                    tail.push(line);
                    if tail.len() > 5 {
                        tail.remove(0);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        let _ = reader.join();
        let status = child.wait().map_err(|e| CoreError::System(format!("{}: {e}", exe.display())))?;
        match status.code() {
            Some(0) | Some(DISM_OK_REBOOT) => Ok(()),
            code => Err(CoreError::System(format!("DISM exit code {}: {}", code.unwrap_or(-1), tail.join(" | ")))),
        }
    }

    fn recycle_bin_size(&self) -> Result<(u64, u64)> {
        let mut info: SHQUERYRBINFO = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHQUERYRBINFO>() as u32;
        let hr = unsafe { SHQueryRecycleBinW(std::ptr::null(), &mut info) };
        if hr < 0 {
            return Err(CoreError::System(format!("SHQueryRecycleBinW failed: 0x{:08X}", hr as u32)));
        }
        let size = info.i64Size;
        let items = info.i64NumItems;
        Ok((size.max(0) as u64, items.max(0) as u64))
    }

    fn empty_recycle_bin(&self) -> Result<()> {
        let flags = SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND;
        let hr = unsafe { SHEmptyRecycleBinW(std::ptr::null_mut(), std::ptr::null(), flags) };
        if hr >= 0 || hr == E_UNEXPECTED {
            Ok(())
        } else {
            Err(CoreError::System(format!("SHEmptyRecycleBinW failed: 0x{:08X}", hr as u32)))
        }
    }

    fn create_restore_point(&self, description: &str) -> Result<()> {
        let d = description.replace('\'', "''");
        self.powershell(&format!(
            "Checkpoint-Computer -Description '{d}' -RestorePointType 'MODIFY_SETTINGS' -WarningAction SilentlyContinue -WarningVariable w -ErrorAction Stop; if ($w) {{ Write-Output ($w -join ' '); exit 2 }}"
        ))
        .map(|_| ())
    }

    fn take_ownership(&self, path: &Path) -> Result<()> {
        let icacls = self.system32.join("icacls.exe");
        let p = path.as_os_str();
        let own: Vec<&OsStr> = vec![p, OsStr::new("/setowner"), OsStr::new(ADMINS_SID), OsStr::new("/T"), OsStr::new("/C"), OsStr::new("/L"), OsStr::new("/Q")];
        run_capture(&icacls, &own)?;
        let grant = format!("{ADMINS_SID}:F");
        let g: Vec<&OsStr> = vec![p, OsStr::new("/grant"), OsStr::new(&grant), OsStr::new("/T"), OsStr::new("/C"), OsStr::new("/L"), OsStr::new("/Q")];
        run_capture(&icacls, &g).map(|_| ())
    }
}

impl Env {
    pub fn from_system() -> Result<Env> {
        fn var(name: &str) -> Result<PathBuf> {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .ok_or_else(|| CoreError::System(format!("environment variable {name} is not set")))
        }
        let windir = var("SystemRoot").or_else(|_| var("windir"))?;
        let mut drive = var("SystemDrive")?.into_os_string();
        drive.push("\\");
        Ok(Env {
            temp: std::env::temp_dir(),
            windir: windir.clone(),
            local_appdata: var("LOCALAPPDATA")?,
            program_data: var("ProgramData")?,
            system_drive: PathBuf::from(drive),
            now: SystemTime::now(),
            sys: Arc::new(RealSystem { system32: windir.join("System32") }),
        })
    }
}
```

Nếu `cargo build` báo lệch kiểu ở `SHQUERYRBINFO` (một số bản windows-sys đánh dấu `packed`), giữ nguyên cách chép trường ra biến cục bộ (`let size = info.i64Size;`) như trên — đó là cách đọc trường của struct packed hợp lệ.

- [ ] **Step 4: Chạy test, thấy qua**

Run: `cargo test -p winfreeup-core sys_windows`
Expected: `6 passed; 0 failed`.

- [ ] **Step 5: Chạy toàn bộ lõi và clippy**

Run: `cargo test -p winfreeup-core` rồi `cargo clippy -p winfreeup-core --all-targets -- -D warnings`
Expected: `0 failed`; clippy không có lỗi.

- [ ] **Step 6: Commit**

```bash
rtk git add crates/winfreeup-core/src/sys_windows.rs
rtk git commit -m "feat(core): RealSystem — dịch vụ, DISM có hủy, Thùng rác, điểm khôi phục, icacls, disk_free

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 10: Khung giao diện — Vite, Vitest, chuỗi tiếng Việt, kiểu dữ liệu dùng chung

**Files:**
- Create: `package.json` (rồi `npm install` sinh `package-lock.json`)
- Create: `vite.config.ts`
- Create: `tsconfig.json`
- Create: `src/test/setup.ts`
- Create: `src/i18n/vi.json`
- Create: `src/i18n/index.ts`
- Create: `src/catalog.ts`
- Create: `src/format.ts`
- Create: `src/api/types.ts`
- Test: `src/i18n/i18n.test.ts`, `src/format.test.ts`

**Interfaces:**
- Consumes: không có (hình dạng JSON khớp serde của lõi mô tả ở Task 1 và Task 8 — tên trường giữ `snake_case`).
- Produces (mọi task giao diện sau dùng nguyên văn):
  - `src/i18n/index.ts`: `t(key: string, params?: Record<string, string | number>): string` (thiếu khoá ⇒ trả chính khoá và `console.warn`), `hasKey(key: string): boolean`.
  - `src/i18n/vi.json`: ĐỦ mọi khoá của v0.1 (liệt kê ở Step 3). Task sau **không** thêm khoá.
  - `src/catalog.ts`: `GROUP_IDS` (9 id, đúng thứ tự `cleaners::registry::ALL_IDS` của lõi), `type GroupId`, `groupName(id: string): string`, `groupDesc(id: string): string`, `noticeText(code: string): string` (`"browser_running:chrome"` ⇒ câu tiếng Việt).
  - `src/format.ts`: `formatBytes(n: number): string` — `≤0` ⇒ `"0 MB"`; `≥1 GiB` ⇒ `"1,5 GB"` (1 chữ số thập phân, dấu phẩy vi-VN); `≥1 MiB` ⇒ `"5 MB"`; còn lại `"< 1 MB"`.
  - `src/api/types.ts`: `RiskLevel`, `Item`, `ScanResult`, `GroupScan`, `CleanReport`, `GroupClean`, `CleanEvent`, `RestorePointStatus`, `CleanSummary`, `AppInfo`, và interface `Api` (7 hàm — xem code).
  - `npm test` = `vitest run` (jsdom, setup `src/test/setup.ts`); `npm run typecheck` = `tsc --noEmit`; `npm run build` = typecheck + `vite build` ra `dist/`.

- [ ] **Step 1: Tạo cấu hình và cài gói**

`package.json`:

```json
{
  "name": "winfreeup",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc --noEmit && vite build",
    "typecheck": "tsc --noEmit",
    "test": "vitest run",
    "tauri": "tauri"
  },
  "dependencies": {
    "@fluentui/react-components": "^9.74.9",
    "@tauri-apps/api": "^2.11.1",
    "react": "^19.3.0",
    "react-dom": "^19.3.0"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.11.5",
    "@testing-library/dom": "^10.4.2",
    "@testing-library/react": "^16.3.3",
    "@testing-library/user-event": "^14.6.7",
    "@types/react": "^19.3.0",
    "@types/react-dom": "^19.3.0",
    "@vitejs/plugin-react": "^6.1.1",
    "jsdom": "^30.1.1",
    "typescript": "~5.9.3",
    "vite": "^8.3.1",
    "vitest": "^5.0.1"
  }
}
```

`vite.config.ts`:

```ts
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// Cổng 1420 cố định theo tauri.conf.json (Task 17).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: 'es2022', outDir: 'dist', emptyOutDir: true },
  test: { environment: 'jsdom', setupFiles: ['src/test/setup.ts'], css: false },
});
```

`tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noEmit": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "skipLibCheck": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "types": ["vite/client"]
  },
  "include": ["src"]
}
```

`src/test/setup.ts`:

```ts
import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';

afterEach(() => cleanup());

// jsdom không có matchMedia / ResizeObserver mà Fluent UI và App cần.
if (!window.matchMedia) {
  window.matchMedia = (query: string) =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    }) as unknown as MediaQueryList;
}

if (!('ResizeObserver' in window)) {
  class ResizeObserverStub {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  (window as unknown as { ResizeObserver: unknown }).ResizeObserver = ResizeObserverStub;
}
```

Run: `npm install`
Expected: tạo `node_modules/` và `package-lock.json`, không có `ERR!`. (Cảnh báo `EBADENGINE` của jsdom 30 — muốn Node ≥ 22.19 trong khi máy dev đang 22.14 — chỉ là cảnh báo; đã đo: 75/75 test vẫn qua trên Node 22.14. CI dùng Node 22 mới nhất nên không gặp.)

- [ ] **Step 2: Viết test hỏng**

`src/i18n/i18n.test.ts`:

```ts
import { describe, expect, it, vi } from 'vitest';
import { t, hasKey } from './index';
import vi_ from './vi.json';
import { GROUP_IDS, groupDesc, groupName, noticeText } from '../catalog';

describe('t()', () => {
  it('thay tham số {tên}', () => {
    expect(t('preview.clean', { size: '1,5 GB' })).toBe('Dọn 1,5 GB');
  });

  it('thiếu khoá thì trả về chính khoá và cảnh báo console', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    expect(t('khong.co.khoa.nay')).toBe('khong.co.khoa.nay');
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });

  it('mọi chuỗi đều khác rỗng', () => {
    for (const [k, v] of Object.entries(vi_)) {
      expect(typeof v, k).toBe('string');
      expect((v as string).trim().length, k).toBeGreaterThan(0);
    }
  });
});

describe('catalog', () => {
  it('có đúng 9 nhóm theo thứ tự lõi', () => {
    expect(GROUP_IDS).toEqual([
      'user_temp', 'system_temp', 'browser_cache', 'win_caches', 'delivery_opt',
      'wu_download', 'component_store', 'recycle_bin', 'windows_old',
    ]);
  });

  it('mỗi nhóm có tên và dòng giải thích tiếng Việt', () => {
    for (const id of GROUP_IDS) {
      expect(hasKey(`group.${id}.name`), id).toBe(true);
      expect(hasKey(`group.${id}.desc`), id).toBe(true);
      expect(groupName(id)).not.toContain('group.');
      expect(groupDesc(id)).not.toContain('group.');
    }
  });

  it('dịch mã thông báo trình duyệt đang mở', () => {
    expect(noticeText('browser_running:coccoc')).toContain('Cốc Cốc');
    expect(noticeText('ma_la')).toBe('ma_la');
  });
});
```

`src/format.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { formatBytes } from './format';

describe('formatBytes', () => {
  it('GB có một chữ số thập phân, dấu phẩy', () => {
    expect(formatBytes(1.5 * 1024 ** 3)).toBe('1,5 GB');
    expect(formatBytes(1024 ** 3)).toBe('1 GB');
  });
  it('MB là số nguyên', () => {
    expect(formatBytes(5 * 1024 ** 2)).toBe('5 MB');
  });
  it('rất nhỏ, bằng 0 hoặc âm', () => {
    expect(formatBytes(1000)).toBe('< 1 MB');
    expect(formatBytes(0)).toBe('0 MB');
    expect(formatBytes(-5)).toBe('0 MB');
  });
});
```

- [ ] **Step 3: Chạy test, thấy hỏng**

Run: `npm test`
Expected: FAIL — `Failed to resolve import "./index"` / `"./format"`.

- [ ] **Step 4: Viết mã**

`src/i18n/vi.json` (toàn bộ chuỗi v0.1 — task sau không thêm khoá):

```json
{
  "app.title": "WinFreeUp — Dọn ổ đĩa",
  "app.dryRunBadge": "Chạy thử — không xóa gì",

  "welcome.intro": "WinFreeUp tìm file tạm và bộ nhớ đệm mà máy tự tạo lại được. Không có gì bị xóa cho tới khi bạn xem danh sách và bấm đồng ý.",
  "welcome.freeSpace": "Ổ {drive} còn trống {size}",
  "welcome.loadingFree": "Đang đọc dung lượng ổ đĩa…",
  "welcome.freeUnknown": "Chưa đọc được dung lượng trống của ổ đĩa.",
  "welcome.scan": "Quét",

  "scanning.title": "Đang quét…",
  "scanning.pending": "Đang quét",
  "scanning.failed": "Không quét được",
  "scanning.cancel": "Hủy",
  "scanning.cancelling": "Đang hủy…",

  "preview.title": "Có thể lấy lại",
  "preview.clean": "Dọn {size}",
  "preview.rescan": "Quét lại",
  "preview.showItems": "Xem 20 mục lớn nhất",
  "preview.hideItems": "Ẩn danh sách",
  "preview.estimated": "khoảng {size}",
  "preview.groupError": "Không quét được nhóm này: {message}",
  "preview.nothing": "Máy đã gọn — không có gì cần dọn.",

  "risk.safe": "An toàn",
  "risk.caution": "Cân nhắc",
  "risk.risky": "Rủi ro",

  "confirm.title": "Xác nhận trước khi dọn",
  "confirm.cautionIntro": "Các nhóm sau cần bạn cân nhắc. Nội dung đã xóa sẽ không lấy lại được:",
  "confirm.riskyIntro": "Có nhóm rủi ro. Gõ XOA vào ô bên dưới để đồng ý.",
  "confirm.riskyLabel": "Gõ XOA để xác nhận",
  "confirm.ok": "Dọn",
  "confirm.cancel": "Quay lại",

  "restore.creating": "Đang tạo điểm khôi phục hệ thống…",
  "restore.failedTitle": "Không tạo được điểm khôi phục",
  "restore.failedBody": "Nếu dọn tiếp, bạn sẽ không dùng System Restore để quay lại trạng thái trước khi dọn được. Chi tiết: {message}",
  "restore.continue": "Vẫn dọn tiếp",
  "restore.abort": "Dừng lại",

  "cleaning.title": "Đang dọn… Đừng tắt máy.",
  "cleaning.waiting": "Chờ",
  "cleaning.running": "Đang dọn",
  "cleaning.done": "Xong",
  "cleaning.failed": "Lỗi",
  "cleaning.percent": "{percent}%",

  "result.title": "Đã lấy lại",
  "result.titleDryRun": "Chạy thử xong — lẽ ra lấy lại",
  "result.measuring": "Đang đo dung lượng trống…",
  "result.measureFailed": "Không đo được dung lượng trống sau khi dọn.",
  "result.groupOk": "{size} · đã xóa {files} file",
  "result.skipped": "bỏ qua {count} file đang bị khóa",
  "result.groupError": "Lỗi: {message}",
  "result.groupItemErrors": "{count} lỗi, xem nhật ký",
  "result.logPath": "Nhật ký: {path}",
  "result.openLog": "Xem nhật ký",
  "result.opening": "Đang mở…",
  "result.home": "Về đầu",

  "notice.browserRunning": "{name} đang mở — file đang dùng sẽ được bỏ qua. Đóng trình duyệt để dọn sạch hơn.",
  "notice.groupFailed": "{name}: {message}",
  "notice.groupErrors": "{name}: {count} lỗi, ví dụ: {first}",
  "notice.logWriteFailed": "Không ghi được đầy đủ nhật ký lượt dọn này.",
  "notice.dismiss": "Đóng",
  "notice.repeat": "(×{count})",

  "errors.scanFailed": "Không quét được: {message}",
  "errors.cleanFailed": "Không dọn được: {message}",
  "errors.freeSpaceFailed": "Không đọc được dung lượng ổ đĩa: {message}",
  "errors.openLogFailed": "Không mở được thư mục nhật ký: {message}",
  "errors.appInfoFailed": "Không đọc được thông tin ứng dụng: {message}",
  "errors.busy": "WinFreeUp đang bận với thao tác trước, hãy đợi xong rồi thử lại.",
  "errors.crashed": "Giao diện gặp lỗi: {message}",

  "browser.chrome": "Chrome",
  "browser.edge": "Edge",
  "browser.firefox": "Firefox",
  "browser.coccoc": "Cốc Cốc",

  "group.user_temp.name": "File tạm của bạn",
  "group.user_temp.desc": "File các chương trình tạo ra để dùng tạm, đã cũ hơn 1 ngày.",
  "group.system_temp.name": "File tạm của Windows",
  "group.system_temp.desc": "File tạm của hệ thống, đã cũ hơn 1 ngày.",
  "group.browser_cache.name": "Bộ nhớ đệm trình duyệt",
  "group.browser_cache.desc": "Ảnh và dữ liệu trang web lưu tạm của Chrome, Edge, Firefox, Cốc Cốc. Không đụng mật khẩu, lịch sử, cookie.",
  "group.win_caches.name": "Bộ nhớ đệm của Windows",
  "group.win_caches.desc": "Ảnh thu nhỏ, báo cáo lỗi và file ghi lại khi máy gặp sự cố. Windows tự tạo lại khi cần.",
  "group.delivery_opt.name": "Bản cập nhật chia sẻ",
  "group.delivery_opt.desc": "File cập nhật Windows giữ lại để chia sẻ cho máy khác. Windows tự tải lại khi cần.",
  "group.wu_download.name": "File tải về của Windows Update",
  "group.wu_download.desc": "Bản cập nhật đã tải về. Nếu còn bản đang chờ cài, Windows sẽ tải lại.",
  "group.component_store.name": "Kho thành phần Windows",
  "group.component_store.desc": "Dọn bản cũ của các thành phần hệ thống bằng công cụ DISM của Microsoft. Mất 5–15 phút.",
  "group.recycle_bin.name": "Thùng rác",
  "group.recycle_bin.desc": "Xóa hẳn mọi thứ trong Thùng rác. Không lấy lại được.",
  "group.windows_old.name": "Bản Windows cũ",
  "group.windows_old.desc": "Bản Windows trước khi nâng cấp. Xóa đi thì không quay về bản cũ được nữa."
}
```

`src/i18n/index.ts`:

```ts
import vi from './vi.json';

const dict: Record<string, string> = vi;

export type Params = Record<string, string | number>;

export function hasKey(key: string): boolean {
  return Object.prototype.hasOwnProperty.call(dict, key);
}

export function t(key: string, params?: Params): string {
  if (!hasKey(key)) {
    console.warn(`[i18n] thiếu khoá: ${key}`);
    return key;
  }
  return dict[key].replace(/\{(\w+)\}/g, (whole, name: string) =>
    params && name in params ? String(params[name]) : whole,
  );
}
```

`src/catalog.ts`:

```ts
import { t } from './i18n';

/** Trùng thứ tự `cleaners::registry::ALL_IDS` của lõi (spec mục 3). */
export const GROUP_IDS = [
  'user_temp',
  'system_temp',
  'browser_cache',
  'win_caches',
  'delivery_opt',
  'wu_download',
  'component_store',
  'recycle_bin',
  'windows_old',
] as const;

export type GroupId = (typeof GROUP_IDS)[number];

export function groupName(id: string): string {
  return t(`group.${id}.name`);
}

export function groupDesc(id: string): string {
  return t(`group.${id}.desc`);
}

/** Dịch mã thông báo của lõi (vd "browser_running:chrome"). Mã lạ giữ nguyên để vẫn thấy được. */
export function noticeText(code: string): string {
  const [kind, arg] = code.split(':');
  if (kind === 'browser_running' && arg) {
    return t('notice.browserRunning', { name: t(`browser.${arg}`) });
  }
  return code;
}
```

`src/format.ts`:

```ts
const GB = 1024 ** 3;
const MB = 1024 ** 2;
const oneDecimal = new Intl.NumberFormat('vi-VN', { maximumFractionDigits: 1 });
const integer = new Intl.NumberFormat('vi-VN', { maximumFractionDigits: 0 });

export function formatBytes(n: number): string {
  if (!(n > 0)) return '0 MB';
  if (n >= GB) return `${oneDecimal.format(n / GB)} GB`;
  if (n >= MB) return `${integer.format(n / MB)} MB`;
  return '< 1 MB';
}
```

`src/api/types.ts`:

```ts
// Khớp serde của winfreeup-core (tên trường snake_case) — đừng đổi tên.
export type RiskLevel = 'safe' | 'caution' | 'risky';

export interface Item {
  path: string;
  bytes: number;
}

export interface ScanResult {
  total_bytes: number;
  file_count: number;
  top_items: Item[];
  estimated: boolean;
  notices: string[];
}

export interface GroupScan {
  id: string;
  risk: RiskLevel;
  default_selected: boolean;
  result: ScanResult | null;
  error: string | null;
}

export interface CleanReport {
  bytes_freed: number;
  files_deleted: number;
  skipped_locked: number;
  errors: string[];
  dry_run: boolean;
}

export interface GroupClean {
  id: string;
  report: CleanReport | null;
  error: string | null;
}

export type CleanEvent =
  | { kind: 'started'; id: string }
  | { kind: 'percent'; id: string; percent: number }
  | { kind: 'finished'; result: GroupClean };

export type RestorePointStatus =
  | { status: 'not_needed' }
  | { status: 'created' }
  | { status: 'skipped' }
  | { status: 'failed'; message: string };

export interface CleanSummary {
  groups: GroupClean[];
  log_path: string;
  dry_run: boolean;
  log_write_failed: boolean;
}

export interface AppInfo {
  version: string;
  dry_run: boolean;
  /** Dạng "C:\\". */
  system_drive: string;
}

/** Cầu nối tới vỏ Tauri. Bản thật ở `api/tauri.ts` (Task 13); test dùng bản giả. */
export interface Api {
  appInfo(): Promise<AppInfo>;
  diskFree(): Promise<number>;
  scanAll(onGroup: (g: GroupScan) => void): Promise<GroupScan[]>;
  cancelScan(): Promise<void>;
  prepareRestorePoint(ids: string[], dryRun: boolean): Promise<RestorePointStatus>;
  clean(ids: string[], dryRun: boolean, onEvent: (e: CleanEvent) => void): Promise<CleanSummary>;
  openLogFolder(): Promise<void>;
}
```

- [ ] **Step 5: Chạy test, thấy qua**

Run: `npm test` rồi `npm run typecheck`
Expected: `Test Files  2 passed (2)`, `Tests  9 passed (9)`; typecheck không lỗi.

- [ ] **Step 6: Commit**

```bash
rtk git add package.json
rtk git add package-lock.json
rtk git add vite.config.ts
rtk git add tsconfig.json
rtk git add src/test/setup.ts
rtk git add src/i18n/vi.json
rtk git add src/i18n/index.ts
rtk git add src/i18n/i18n.test.ts
rtk git add src/catalog.ts
rtk git add src/format.ts
rtk git add src/format.test.ts
rtk git add src/api/types.ts
rtk git commit -m "feat(ui): khung Vite/Vitest, chuỗi tiếng Việt, danh mục nhóm, kiểu dữ liệu dùng chung

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Máy trạng thái và bộ chọn

**Files:**
- Create: `src/state/machine.ts`
- Create: `src/state/selectors.ts`
- Test: `src/state/machine.test.ts`

**Interfaces:**
- Consumes (Task 10): kiểu `GroupScan`, `ScanResult`, `GroupClean`, `CleanEvent`, `RestorePointStatus`, `CleanSummary`, `RiskLevel` từ `src/api/types.ts`; `GROUP_IDS` từ `src/catalog.ts`.
- Produces:
  - `type Phase = 'welcome' | 'scanning' | 'preview' | 'confirm' | 'restorePoint' | 'cleaning' | 'result'`
  - `interface GroupState { id: string; status: 'pending' | 'done' | 'error'; risk: RiskLevel | null; defaultSelected: boolean; result: ScanResult | null; error: string | null }`
  - `interface CleanRow { id: string; status: 'waiting' | 'running' | 'done'; percent: number | null; result: GroupClean | null }`
  - `interface State { phase; dryRun: boolean; systemDrive: string; freeBefore: number | null; freeFailed: boolean; freeAfter: number | null; freeAfterFailed: boolean; groups: GroupState[]; selected: string[]; cancelling: boolean; restore: RestorePointStatus | null; cleaning: CleanRow[]; summary: CleanSummary | null }`, `initialState: State`
  - `type Action` (19 loại — xem code), `reducer(s: State, a: Action): State` — hành động không hợp với phase hiện tại thì trả nguyên `s` (cùng tham chiếu).
  - `selectors.ts`: `selectable(g: GroupState): boolean`, `selectedBytes(s: State): number`, `type ConfirmKind = 'none' | 'caution' | 'risky'`, `confirmKind(groups: GroupState[], selected: string[]): ConfirmKind`, `isConfirmWord(input: string): boolean`, `reclaimedBytes(before: number | null, after: number | null): number | null` (không bao giờ âm), `dryRunBytes(summary: CleanSummary): number`.
  - Luồng: `REQUEST_CLEAN` chỉ-An-toàn ⇒ `cleaning` luôn; có Cân nhắc/Rủi ro ⇒ `confirm` ⇒ `CONFIRM_ACCEPTED` ⇒ `restorePoint` (restore = null, đang tạo) ⇒ `RESTORE_RESULT` (failed ⇒ đứng lại chờ `RESTORE_CONTINUE`/`RESTORE_ABORT`; khác ⇒ `cleaning`) ⇒ `CLEAN_DONE` ⇒ `result`.

- [ ] **Step 1: Viết test hỏng**

`src/state/machine.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { initialState, reducer, type Action, type State } from './machine';
import { confirmKind, dryRunBytes, isConfirmWord, reclaimedBytes, selectedBytes } from './selectors';
import type { GroupScan } from '../api/types';

const GB = 1024 ** 3;

function scan(id: string, risk: GroupScan['risk'], bytes: number, def = risk === 'safe', error: string | null = null): GroupScan {
  return {
    id,
    risk,
    default_selected: def,
    result: error ? null : { total_bytes: bytes, file_count: 1, top_items: [], estimated: false, notices: [] },
    error,
  };
}

function run(s: State, ...actions: Action[]): State {
  return actions.reduce(reducer, s);
}

const SCANS = [
  scan('user_temp', 'safe', 1 * GB),
  scan('system_temp', 'safe', 0),
  scan('browser_cache', 'safe', 0, true, 'disk error'),
  scan('recycle_bin', 'caution', GB / 2),
  scan('windows_old', 'risky', 10 * GB),
];

function preview(): State {
  return run(initialState, { type: 'SCAN_STARTED' }, { type: 'SCAN_FINISHED', groups: SCANS });
}

describe('quét', () => {
  it('bắt đầu quét thì mọi nhóm của catalog đang chờ', () => {
    const s = run(initialState, { type: 'SCAN_STARTED' });
    expect(s.phase).toBe('scanning');
    expect(s.groups).toHaveLength(9);
    expect(s.groups.every((g) => g.status === 'pending')).toBe(true);
  });

  it('nhóm xong nào hiện nhóm đó', () => {
    const s = run(initialState, { type: 'SCAN_STARTED' }, { type: 'SCAN_GROUP_DONE', group: SCANS[0] });
    expect(s.groups.find((g) => g.id === 'user_temp')?.status).toBe('done');
    expect(s.groups.find((g) => g.id === 'system_temp')?.status).toBe('pending');
  });

  it('kết thúc quét: tích sẵn nhóm mặc định có dung lượng, bỏ nhóm 0 byte và nhóm lỗi', () => {
    const s = preview();
    expect(s.phase).toBe('preview');
    expect(s.selected).toEqual(['user_temp']);
    expect(s.groups.find((g) => g.id === 'browser_cache')?.status).toBe('error');
  });

  it('sự kiện đến muộn sau khi hủy bị bỏ qua', () => {
    const aborted = run(initialState, { type: 'SCAN_STARTED' }, { type: 'SCAN_CANCEL_REQUESTED' }, { type: 'SCAN_ABORTED' });
    expect(aborted.phase).toBe('welcome');
    expect(reducer(aborted, { type: 'SCAN_GROUP_DONE', group: SCANS[0] })).toBe(aborted);
    expect(reducer(aborted, { type: 'SCAN_FINISHED', groups: SCANS })).toBe(aborted);
  });
});

describe('chọn nhóm', () => {
  it('tích/bỏ tích giữ thứ tự nhóm và tổng luôn đúng', () => {
    let s = preview();
    expect(selectedBytes(s)).toBe(GB);
    s = reducer(s, { type: 'TOGGLE', id: 'recycle_bin' });
    expect(s.selected).toEqual(['user_temp', 'recycle_bin']);
    expect(selectedBytes(s)).toBe(1.5 * GB);
    s = reducer(s, { type: 'TOGGLE', id: 'user_temp' });
    expect(s.selected).toEqual(['recycle_bin']);
  });

  it('không tích được nhóm lỗi hoặc 0 byte', () => {
    const s = preview();
    expect(reducer(s, { type: 'TOGGLE', id: 'browser_cache' })).toBe(s);
    expect(reducer(s, { type: 'TOGGLE', id: 'system_temp' })).toBe(s);
  });
});

describe('xác nhận theo mức rủi ro', () => {
  it('chỉ nhóm An toàn thì vào dọn ngay, không qua xác nhận', () => {
    const s = reducer(preview(), { type: 'REQUEST_CLEAN' });
    expect(s.phase).toBe('cleaning');
    expect(s.cleaning.map((r) => r.id)).toEqual(['user_temp']);
  });

  it('có Cân nhắc thì hỏi; đồng ý thì tạo điểm khôi phục rồi dọn', () => {
    let s = run(preview(), { type: 'TOGGLE', id: 'recycle_bin' }, { type: 'REQUEST_CLEAN' });
    expect(s.phase).toBe('confirm');
    expect(confirmKind(s.groups, s.selected)).toBe('caution');
    s = reducer(s, { type: 'CONFIRM_ACCEPTED' });
    expect(s.phase).toBe('restorePoint');
    expect(s.restore).toBeNull();
    s = reducer(s, { type: 'RESTORE_RESULT', status: { status: 'created' } });
    expect(s.phase).toBe('cleaning');
  });

  it('có Rủi ro thì confirmKind là risky; quay lại thì về xem trước', () => {
    const s = run(preview(), { type: 'TOGGLE', id: 'windows_old' }, { type: 'REQUEST_CLEAN' });
    expect(confirmKind(s.groups, s.selected)).toBe('risky');
    expect(reducer(s, { type: 'CONFIRM_CANCELLED' }).phase).toBe('preview');
  });

  it('không tạo được điểm khôi phục thì đứng lại cho người dùng chọn', () => {
    const failed = run(
      preview(),
      { type: 'TOGGLE', id: 'recycle_bin' },
      { type: 'REQUEST_CLEAN' },
      { type: 'CONFIRM_ACCEPTED' },
      { type: 'RESTORE_RESULT', status: { status: 'failed', message: 'off' } },
    );
    expect(failed.phase).toBe('restorePoint');
    expect(reducer(failed, { type: 'RESTORE_CONTINUE' }).phase).toBe('cleaning');
    expect(reducer(failed, { type: 'RESTORE_ABORT' }).phase).toBe('preview');
  });

  it('không có gì được chọn thì bấm Dọn không làm gì', () => {
    const s = reducer(preview(), { type: 'TOGGLE', id: 'user_temp' });
    expect(reducer(s, { type: 'REQUEST_CLEAN' })).toBe(s);
  });
});

describe('dọn và kết quả', () => {
  it('sự kiện tiến độ cập nhật từng dòng; xong thì sang kết quả', () => {
    let s = reducer(preview(), { type: 'REQUEST_CLEAN' });
    s = reducer(s, { type: 'CLEAN_EVENT', event: { kind: 'started', id: 'user_temp' } });
    expect(s.cleaning[0].status).toBe('running');
    s = reducer(s, { type: 'CLEAN_EVENT', event: { kind: 'percent', id: 'user_temp', percent: 40 } });
    expect(s.cleaning[0].percent).toBe(40);
    const result = { id: 'user_temp', report: { bytes_freed: GB, files_deleted: 3, skipped_locked: 1, errors: [], dry_run: false }, error: null };
    s = reducer(s, { type: 'CLEAN_EVENT', event: { kind: 'finished', result } });
    expect(s.cleaning[0].status).toBe('done');
    s = reducer(s, { type: 'CLEAN_DONE', summary: { groups: [result], log_path: 'x.log', dry_run: false, log_write_failed: false } });
    expect(s.phase).toBe('result');
    expect(s.freeAfter).toBeNull();
    s = reducer(s, { type: 'FREE_AFTER', bytes: 123 });
    expect(s.freeAfter).toBe(123);
  });

  it('dọn hỏng thì quay về xem trước', () => {
    const s = run(preview(), { type: 'REQUEST_CLEAN' }, { type: 'CLEAN_FAILED' });
    expect(s.phase).toBe('preview');
  });

  it('về đầu giữ cờ chạy thử và ổ hệ thống', () => {
    const s = run(initialState, { type: 'APP_INFO', dryRun: true, systemDrive: 'D:\\' }, { type: 'SCAN_STARTED' }, { type: 'RESET' });
    expect(s.phase).toBe('welcome');
    expect(s.dryRun).toBe(true);
    expect(s.systemDrive).toBe('D:\\');
  });
});

describe('selectors', () => {
  // Review Focus 1
  it('chấp nhận XOA gõ theo thói quen tiếng Việt', () => {
    for (const ok of ['XOA', 'xoa', 'xóa', 'Xóa', 'XÓA', '  XOA  ', 'xoá']) {
      expect(isConfirmWord(ok), ok).toBe(true);
    }
    for (const bad of ['', 'XO', 'XOAA', 'X O A', 'xóa đi']) {
      expect(isConfirmWord(bad), bad).toBe(false);
    }
  });

  // Review Focus 5
  it('dung lượng lấy lại không bao giờ âm', () => {
    expect(reclaimedBytes(100, 50)).toBe(0);
    expect(reclaimedBytes(100, 300)).toBe(200);
    expect(reclaimedBytes(null, 300)).toBeNull();
  });

  it('chạy thử cộng số byte lẽ ra xóa', () => {
    const r = (b: number) => ({ id: 'a', report: { bytes_freed: b, files_deleted: 1, skipped_locked: 0, errors: [], dry_run: true }, error: null });
    expect(dryRunBytes({ groups: [r(5), r(7), { id: 'x', report: null, error: 'e' }], log_path: '', dry_run: true, log_write_failed: false })).toBe(12);
  });
});
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `npx vitest run src/state`
Expected: FAIL — `Failed to resolve import "./machine"`.

- [ ] **Step 3: Viết mã**

`src/state/machine.ts`:

```ts
import type { CleanEvent, CleanSummary, GroupClean, GroupScan, RestorePointStatus, RiskLevel, ScanResult } from '../api/types';
import { GROUP_IDS } from '../catalog';
import { confirmKind, selectable } from './selectors';

export type Phase = 'welcome' | 'scanning' | 'preview' | 'confirm' | 'restorePoint' | 'cleaning' | 'result';

export interface GroupState {
  id: string;
  status: 'pending' | 'done' | 'error';
  risk: RiskLevel | null;
  defaultSelected: boolean;
  result: ScanResult | null;
  error: string | null;
}

export interface CleanRow {
  id: string;
  status: 'waiting' | 'running' | 'done';
  percent: number | null;
  result: GroupClean | null;
}

export interface State {
  phase: Phase;
  dryRun: boolean;
  systemDrive: string;
  freeBefore: number | null;
  freeFailed: boolean;
  freeAfter: number | null;
  freeAfterFailed: boolean;
  groups: GroupState[];
  selected: string[];
  cancelling: boolean;
  restore: RestorePointStatus | null;
  cleaning: CleanRow[];
  summary: CleanSummary | null;
}

export const initialState: State = {
  phase: 'welcome',
  dryRun: false,
  systemDrive: 'C:\\',
  freeBefore: null,
  freeFailed: false,
  freeAfter: null,
  freeAfterFailed: false,
  groups: [],
  selected: [],
  cancelling: false,
  restore: null,
  cleaning: [],
  summary: null,
};

export type Action =
  | { type: 'APP_INFO'; dryRun: boolean; systemDrive: string }
  | { type: 'FREE_SPACE'; bytes: number }
  | { type: 'FREE_SPACE_FAILED' }
  | { type: 'SCAN_STARTED' }
  | { type: 'SCAN_GROUP_DONE'; group: GroupScan }
  | { type: 'SCAN_FINISHED'; groups: GroupScan[] }
  | { type: 'SCAN_CANCEL_REQUESTED' }
  | { type: 'SCAN_ABORTED' }
  | { type: 'TOGGLE'; id: string }
  | { type: 'REQUEST_CLEAN' }
  | { type: 'CONFIRM_ACCEPTED' }
  | { type: 'CONFIRM_CANCELLED' }
  | { type: 'RESTORE_RESULT'; status: RestorePointStatus }
  | { type: 'RESTORE_CONTINUE' }
  | { type: 'RESTORE_ABORT' }
  | { type: 'CLEAN_EVENT'; event: CleanEvent }
  | { type: 'CLEAN_DONE'; summary: CleanSummary }
  | { type: 'CLEAN_FAILED' }
  | { type: 'FREE_AFTER'; bytes: number }
  | { type: 'FREE_AFTER_FAILED' }
  | { type: 'RESET' };

function fromScan(g: GroupScan): GroupState {
  return {
    id: g.id,
    status: g.error ? 'error' : 'done',
    risk: g.risk,
    defaultSelected: g.default_selected,
    result: g.error ? null : g.result,
    error: g.error,
  };
}

function pendingGroups(): GroupState[] {
  return GROUP_IDS.map((id) => ({ id, status: 'pending', risk: null, defaultSelected: false, result: null, error: null }));
}

function mergeGroup(groups: GroupState[], g: GroupScan): GroupState[] {
  return groups.some((x) => x.id === g.id)
    ? groups.map((x) => (x.id === g.id ? fromScan(g) : x))
    : [...groups, fromScan(g)];
}

function startCleaning(s: State): State {
  return {
    ...s,
    phase: 'cleaning',
    cleaning: s.selected.map((id) => ({ id, status: 'waiting', percent: null, result: null })),
    summary: null,
    freeAfter: null,
    freeAfterFailed: false,
  };
}

function applyCleanEvent(rows: CleanRow[], e: CleanEvent): CleanRow[] {
  switch (e.kind) {
    case 'started':
      return rows.map((r) => (r.id === e.id ? { ...r, status: 'running' } : r));
    case 'percent':
      return rows.map((r) => (r.id === e.id ? { ...r, percent: e.percent } : r));
    case 'finished':
      return rows.map((r) => (r.id === e.result.id ? { ...r, status: 'done', result: e.result } : r));
  }
}

export function reducer(s: State, a: Action): State {
  switch (a.type) {
    case 'APP_INFO':
      return { ...s, dryRun: a.dryRun, systemDrive: a.systemDrive };
    case 'FREE_SPACE':
      return { ...s, freeBefore: a.bytes, freeFailed: false };
    case 'FREE_SPACE_FAILED':
      return { ...s, freeFailed: true };
    case 'SCAN_STARTED':
      if (s.phase !== 'welcome' && s.phase !== 'preview') return s;
      return { ...s, phase: 'scanning', groups: pendingGroups(), selected: [], cancelling: false };
    case 'SCAN_GROUP_DONE':
      if (s.phase !== 'scanning') return s;
      return { ...s, groups: mergeGroup(s.groups, a.group) };
    case 'SCAN_FINISHED': {
      if (s.phase !== 'scanning') return s;
      const groups = a.groups.reduce(mergeGroup, s.groups);
      const selected = groups.filter((g) => g.defaultSelected && selectable(g)).map((g) => g.id);
      return { ...s, phase: 'preview', groups, selected, cancelling: false };
    }
    case 'SCAN_CANCEL_REQUESTED':
      if (s.phase !== 'scanning') return s;
      return { ...s, cancelling: true };
    case 'SCAN_ABORTED':
      if (s.phase !== 'scanning') return s;
      return { ...s, phase: 'welcome', groups: [], selected: [], cancelling: false };
    case 'TOGGLE': {
      if (s.phase !== 'preview') return s;
      const g = s.groups.find((x) => x.id === a.id);
      if (!g || !selectable(g)) return s;
      const on = !s.selected.includes(a.id);
      const selected = s.groups
        .filter((x) => (x.id === a.id ? on : s.selected.includes(x.id)))
        .map((x) => x.id);
      return { ...s, selected };
    }
    case 'REQUEST_CLEAN':
      if (s.phase !== 'preview' || s.selected.length === 0) return s;
      return confirmKind(s.groups, s.selected) === 'none'
        ? startCleaning({ ...s, restore: null })
        : { ...s, phase: 'confirm' };
    case 'CONFIRM_ACCEPTED':
      if (s.phase !== 'confirm') return s;
      return { ...s, phase: 'restorePoint', restore: null };
    case 'CONFIRM_CANCELLED':
      if (s.phase !== 'confirm') return s;
      return { ...s, phase: 'preview' };
    case 'RESTORE_RESULT':
      if (s.phase !== 'restorePoint' || s.restore !== null) return s;
      return a.status.status === 'failed' ? { ...s, restore: a.status } : startCleaning({ ...s, restore: a.status });
    case 'RESTORE_CONTINUE':
      if (s.phase !== 'restorePoint' || s.restore?.status !== 'failed') return s;
      return startCleaning(s);
    case 'RESTORE_ABORT':
      if (s.phase !== 'restorePoint' || s.restore?.status !== 'failed') return s;
      return { ...s, phase: 'preview', restore: null };
    case 'CLEAN_EVENT':
      if (s.phase !== 'cleaning') return s;
      return { ...s, cleaning: applyCleanEvent(s.cleaning, a.event) };
    case 'CLEAN_DONE':
      if (s.phase !== 'cleaning') return s;
      return { ...s, phase: 'result', summary: a.summary, freeAfter: null, freeAfterFailed: false };
    case 'CLEAN_FAILED':
      if (s.phase !== 'cleaning') return s;
      return { ...s, phase: 'preview', cleaning: [] };
    case 'FREE_AFTER':
      if (s.phase !== 'result') return s;
      return { ...s, freeAfter: a.bytes };
    case 'FREE_AFTER_FAILED':
      if (s.phase !== 'result') return s;
      return { ...s, freeAfterFailed: true };
    case 'RESET':
      return { ...initialState, dryRun: s.dryRun, systemDrive: s.systemDrive };
  }
}
```

`src/state/selectors.ts`:

```ts
import type { CleanSummary } from '../api/types';
import type { GroupState, State } from './machine';

export type ConfirmKind = 'none' | 'caution' | 'risky';

export function selectable(g: GroupState): boolean {
  return g.status === 'done' && (g.result?.total_bytes ?? 0) > 0;
}

export function selectedBytes(s: State): number {
  return s.groups
    .filter((g) => s.selected.includes(g.id) && g.result)
    .reduce((sum, g) => sum + (g.result?.total_bytes ?? 0), 0);
}

export function confirmKind(groups: GroupState[], selected: string[]): ConfirmKind {
  const risks = groups.filter((g) => selected.includes(g.id)).map((g) => g.risk);
  if (risks.includes('risky')) return 'risky';
  if (risks.includes('caution')) return 'caution';
  return 'none';
}

/** "XOA" sau khi bỏ dấu tiếng Việt, khoảng trắng hai đầu và phân biệt hoa/thường. */
export function isConfirmWord(input: string): boolean {
  const plain = input
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .replace(/[đĐ]/g, 'd')
    .trim()
    .toUpperCase();
  return plain === 'XOA';
}

export function reclaimedBytes(before: number | null, after: number | null): number | null {
  if (before === null || after === null) return null;
  return Math.max(0, after - before);
}

export function dryRunBytes(summary: CleanSummary): number {
  return summary.groups.reduce((sum, g) => sum + (g.report?.bytes_freed ?? 0), 0);
}
```

- [ ] **Step 4: Chạy test, thấy qua**

Run: `npx vitest run src/state` rồi `npm run typecheck`
Expected: `Tests  17 passed (17)`; typecheck không lỗi.

- [ ] **Step 5: Commit**

```bash
rtk git add src/state/machine.ts
rtk git add src/state/selectors.ts
rtk git add src/state/machine.test.ts
rtk git commit -m "feat(ui): máy trạng thái Chào→Quét→Xem trước→Xác nhận→Điểm khôi phục→Dọn→Kết quả

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Báo lỗi lên màn, vòng quay, CSS

**Files:**
- Create: `src/errors/errors.ts`
- Create: `src/components/Busy.tsx`
- Create: `src/components/NoticeBar.tsx`
- Create: `src/components/ErrorBoundary.tsx`
- Create: `src/styles.css`
- Test: `src/errors/errors.test.ts`, `src/components/components.test.tsx`

**Interfaces:**
- Consumes (Task 10): `t(key, params?)` từ `src/i18n`; khoá `notice.dismiss`, `notice.repeat`, `errors.crashed`, `result.home` có sẵn trong `vi.json`.
- Produces:
  - `errors.ts`: `type Severity = 'error' | 'warning'`, `interface Notice { id: number; severity: Severity; message: string; count: number }`, `MAX_NOTICES = 5`, `pushNotice(list: Notice[], severity: Severity, message: string): Notice[]` (gộp trùng ⇒ tăng `count`; quá 5 ⇒ bỏ cảnh báo cũ nhất trước, chỉ bỏ lỗi đỏ khi không còn cảnh báo), `dismissNotice(list: Notice[], id: number): Notice[]`, `messageOf(e: unknown): string`, `describeErrorEvent(ev: { message?: string; filename?: string; lineno?: number; error?: unknown }): string` (`"msg (file.js:12)"`), `installGlobalHooks(target: Window, report: (severity: Severity, message: string) => void): () => void` (móc `error` + `unhandledrejection`, trả hàm gỡ).
  - `Busy.tsx`: `Busy({ label?: string; size?: 'tiny' | 'small' | 'medium' })` — Fluent `Spinner` trong `<span class="wfu-busy">`.
  - `NoticeBar.tsx`: `NoticeBar({ notices: Notice[]; onDismiss: (id: number) => void })` — đỏ (`intent="error"`, lớp `wfu-loi`) / hổ phách (`intent="warning"`, lớp `wfu-canhbao`).
  - `ErrorBoundary.tsx`: `class ErrorBoundary` props `{ children: ReactNode; onReset: () => void }` — lỗi render ⇒ băng đỏ nguyên văn + nút «Về đầu».
  - `styles.css` — ĐỦ mọi lớp các màn dùng: `wfu-root, wfu-app, wfu-header, wfu-notices, wfu-busy, wfu-hero, wfu-hero-number, wfu-actions, wfu-groups, wfu-group, wfu-group-main, wfu-group-title, wfu-group-desc, wfu-group-size, wfu-items, wfu-item, wfu-item-path, wfu-muted, wfu-loi, wfu-canhbao`; `prefers-reduced-motion` đổi vòng quay thành nhịp mờ tỏ.

- [ ] **Step 1: Viết test hỏng**

`src/errors/errors.test.ts`:

```ts
import { describe, expect, it, vi } from 'vitest';
import { describeErrorEvent, dismissNotice, installGlobalHooks, MAX_NOTICES, messageOf, pushNotice, type Notice } from './errors';

describe('pushNotice', () => {
  it('gộp trùng và đếm số lần', () => {
    let l: Notice[] = [];
    l = pushNotice(l, 'warning', 'A');
    l = pushNotice(l, 'warning', 'A');
    expect(l).toHaveLength(1);
    expect(l[0].count).toBe(2);
  });

  it('tối đa 5 dòng, bỏ cảnh báo cũ trước, giữ lỗi đỏ', () => {
    let l: Notice[] = pushNotice([], 'error', 'ĐỎ');
    for (let i = 0; i < 7; i++) l = pushNotice(l, 'warning', `w${i}`);
    expect(l).toHaveLength(MAX_NOTICES);
    expect(l[0].message).toBe('ĐỎ');
    expect(l.map((n) => n.message)).toEqual(['ĐỎ', 'w3', 'w4', 'w5', 'w6']);
  });

  it('đóng một dòng', () => {
    const l = pushNotice(pushNotice([], 'warning', 'A'), 'error', 'B');
    expect(dismissNotice(l, l[0].id).map((n) => n.message)).toEqual(['B']);
  });
});

describe('thông điệp lỗi', () => {
  it('lấy nguyên văn từ Error, chuỗi hoặc đối tượng', () => {
    expect(messageOf(new Error('hỏng ổ'))).toBe('hỏng ổ');
    expect(messageOf('busy')).toBe('busy');
    expect(messageOf({ code: 5 })).toBe('{"code":5}');
  });

  it('kèm tên file và số dòng', () => {
    expect(describeErrorEvent({ message: 'x is undefined', filename: 'http://localhost/assets/index-ab.js', lineno: 12 })).toBe('x is undefined (index-ab.js:12)');
    expect(describeErrorEvent({ error: new Error('không file') })).toBe('không file');
  });
});

describe('installGlobalHooks', () => {
  it('lỗi ngoài luồng và promise bị bỏ rơi đều lên màn', () => {
    const report = vi.fn();
    const err = vi.spyOn(console, 'error').mockImplementation(() => {});
    const off = installGlobalHooks(window, report);
    window.dispatchEvent(new ErrorEvent('error', { message: 'vẽ hỏng', filename: 'a.js', lineno: 3 }));
    const rej = new Event('unhandledrejection') as Event & { reason?: unknown };
    rej.reason = new Error('promise hỏng');
    window.dispatchEvent(rej);
    expect(report).toHaveBeenCalledWith('warning', 'vẽ hỏng (a.js:3)');
    expect(report).toHaveBeenCalledWith('warning', 'promise hỏng');
    expect(err).toHaveBeenCalled();
    off();
    window.dispatchEvent(new ErrorEvent('error', { message: 'sau khi gỡ' }));
    expect(report).toHaveBeenCalledTimes(2);
    err.mockRestore();
  });
});
```

`src/components/components.test.tsx`:

```tsx
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import type { ReactNode } from 'react';
import { NoticeBar } from './NoticeBar';
import { ErrorBoundary } from './ErrorBoundary';
import { Busy } from './Busy';
import { pushNotice } from '../errors/errors';

const wrap = (ui: ReactNode) => render(<FluentProvider theme={webLightTheme}>{ui}</FluentProvider>);

describe('NoticeBar', () => {
  it('hiện nguyên văn, số lần lặp và nút đóng', () => {
    const notices = pushNotice(pushNotice([], 'warning', 'Chrome đang mở'), 'warning', 'Chrome đang mở');
    const onDismiss = vi.fn();
    wrap(<NoticeBar notices={notices} onDismiss={onDismiss} />);
    expect(screen.getByText(/Chrome đang mở/).textContent).toContain('(×2)');
    fireEvent.click(screen.getByRole('button', { name: 'Đóng' }));
    expect(onDismiss).toHaveBeenCalledWith(notices[0].id);
  });

  it('không có gì thì không vẽ', () => {
    const { container } = wrap(<NoticeBar notices={[]} onDismiss={() => {}} />);
    expect(container.querySelector('.wfu-notices')).toBeNull();
  });
});

describe('ErrorBoundary', () => {
  it('lỗi khi vẽ thì hiện băng đỏ nguyên văn và nút Về đầu', () => {
    const err = vi.spyOn(console, 'error').mockImplementation(() => {});
    const onReset = vi.fn();
    function Boom(): ReactNode {
      throw new Error('không đọc được top_items');
    }
    wrap(
      <ErrorBoundary onReset={onReset}>
        <Boom />
      </ErrorBoundary>,
    );
    expect(screen.getByText('Giao diện gặp lỗi: không đọc được top_items')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Về đầu' }));
    expect(onReset).toHaveBeenCalled();
    err.mockRestore();
  });
});

describe('Busy', () => {
  it('có vòng quay và nhãn', () => {
    wrap(<Busy label="Đang quét" />);
    expect(screen.getByRole('progressbar')).toBeTruthy();
    expect(screen.getByText('Đang quét')).toBeTruthy();
  });
});
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `npx vitest run src/errors src/components/components.test.tsx`
Expected: FAIL — `Failed to resolve import "./errors"`, `"./NoticeBar"`.

- [ ] **Step 3: Viết mã**

`src/errors/errors.ts`:

```ts
export type Severity = 'error' | 'warning';

export interface Notice {
  id: number;
  severity: Severity;
  message: string;
  count: number;
}

export const MAX_NOTICES = 5;

let nextId = 1;

export function pushNotice(list: Notice[], severity: Severity, message: string): Notice[] {
  const i = list.findIndex((n) => n.severity === severity && n.message === message);
  if (i >= 0) return list.map((n, j) => (j === i ? { ...n, count: n.count + 1 } : n));
  let next = [...list, { id: nextId++, severity, message, count: 1 }];
  while (next.length > MAX_NOTICES) {
    const w = next.findIndex((n) => n.severity === 'warning');
    next = next.filter((_, j) => j !== (w >= 0 ? w : 0));
  }
  return next;
}

export function dismissNotice(list: Notice[], id: number): Notice[] {
  return list.filter((n) => n.id !== id);
}

export function messageOf(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === 'string') return e;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

export function describeErrorEvent(ev: { message?: string; filename?: string; lineno?: number; error?: unknown }): string {
  const msg = ev.message || messageOf(ev.error);
  const file = ev.filename ? ev.filename.split('/').pop() : '';
  return file ? `${msg} (${file}:${ev.lineno ?? 0})` : msg;
}

/** Lỗi ngoài luồng lên màn dạng hổ phách (màn vẫn dùng được); vẫn ghi console đủ ngăn xếp. */
export function installGlobalHooks(target: Window, report: (severity: Severity, message: string) => void): () => void {
  const onError = (ev: ErrorEvent) => {
    console.error(ev.error ?? ev.message);
    report('warning', describeErrorEvent(ev));
  };
  const onRejection = (ev: Event) => {
    const reason = (ev as PromiseRejectionEvent).reason;
    console.error(reason);
    report('warning', messageOf(reason));
  };
  target.addEventListener('error', onError);
  target.addEventListener('unhandledrejection', onRejection);
  return () => {
    target.removeEventListener('error', onError);
    target.removeEventListener('unhandledrejection', onRejection);
  };
}
```

`src/components/Busy.tsx`:

```tsx
import { Spinner } from '@fluentui/react-components';

export function Busy({ label, size = 'tiny' }: { label?: string; size?: 'tiny' | 'small' | 'medium' }) {
  return (
    <span className="wfu-busy">
      <Spinner size={size} label={label} labelPosition="after" />
    </span>
  );
}
```

`src/components/NoticeBar.tsx`:

```tsx
import { Button, MessageBar, MessageBarActions, MessageBarBody } from '@fluentui/react-components';
import type { Notice } from '../errors/errors';
import { t } from '../i18n';

export function NoticeBar({ notices, onDismiss }: { notices: Notice[]; onDismiss: (id: number) => void }) {
  if (notices.length === 0) return null;
  return (
    <div className="wfu-notices" aria-live="polite">
      {notices.map((n) => (
        <MessageBar key={n.id} intent={n.severity === 'error' ? 'error' : 'warning'} className={n.severity === 'error' ? 'wfu-loi' : 'wfu-canhbao'}>
          <MessageBarBody>
            {n.message}
            {n.count > 1 ? ` ${t('notice.repeat', { count: n.count })}` : ''}
          </MessageBarBody>
          <MessageBarActions
            containerAction={
              <Button appearance="transparent" size="small" aria-label={t('notice.dismiss')} onClick={() => onDismiss(n.id)}>
                ×
              </Button>
            }
          />
        </MessageBar>
      ))}
    </div>
  );
}
```

`src/components/ErrorBoundary.tsx`:

```tsx
import { Component, type ErrorInfo, type ReactNode } from 'react';
import { Button, MessageBar, MessageBarActions, MessageBarBody } from '@fluentui/react-components';
import { t } from '../i18n';

interface Props {
  children: ReactNode;
  onReset: () => void;
}

export class ErrorBoundary extends Component<Props, { error: Error | null }> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <MessageBar intent="error" className="wfu-loi">
          <MessageBarBody>{t('errors.crashed', { message: this.state.error.message })}</MessageBarBody>
          <MessageBarActions>
            <Button
              onClick={() => {
                this.setState({ error: null });
                this.props.onReset();
              }}
            >
              {t('result.home')}
            </Button>
          </MessageBarActions>
        </MessageBar>
      );
    }
    return this.props.children;
  }
}
```

`src/styles.css`:

```css
:root {
  color-scheme: light dark;
}

html,
body {
  margin: 0;
  font-family: 'Segoe UI Variable Text', 'Segoe UI', system-ui, sans-serif;
}

.wfu-root {
  min-height: 100vh;
}

.wfu-app {
  max-width: 880px;
  margin: 0 auto;
  padding: 16px 24px 32px;
  display: grid;
  gap: 16px;
}

.wfu-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.wfu-notices {
  display: grid;
  gap: 8px;
}

.wfu-busy {
  display: inline-flex;
  align-items: center;
}

.wfu-hero {
  display: grid;
  gap: 4px;
}

.wfu-hero-number {
  font-size: 40px;
  font-weight: 600;
  line-height: 1.1;
}

.wfu-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.wfu-groups {
  display: grid;
  gap: 8px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.wfu-group {
  display: grid;
  grid-template-columns: auto 1fr auto;
  align-items: start;
  gap: 4px 12px;
  padding: 10px 12px;
  border-radius: 6px;
  border: 1px solid color-mix(in srgb, currentColor 14%, transparent);
}

.wfu-group-main {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.wfu-group-title {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  font-weight: 600;
}

.wfu-group-desc,
.wfu-muted {
  opacity: 0.75;
  font-size: 13px;
}

.wfu-group-size {
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  text-align: right;
}

.wfu-items {
  grid-column: 2 / 4;
  margin: 4px 0 0;
  padding: 0;
  list-style: none;
  display: grid;
  gap: 2px;
  font-size: 12px;
}

.wfu-item {
  display: flex;
  justify-content: space-between;
  gap: 12px;
}

.wfu-item-path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  direction: rtl;
  text-align: left;
}

.wfu-loi,
.wfu-canhbao {
  white-space: pre-wrap;
}

/* Giảm chuyển động: dừng quay nhưng KHÔNG bỏ tín hiệu — đổi sang nhịp mờ tỏ. */
@media (prefers-reduced-motion: reduce) {
  .wfu-busy .fui-Spinner__spinner,
  .wfu-busy .fui-Spinner__spinnerTail {
    animation: none !important;
  }
  .wfu-busy {
    animation: wfu-pulse 1.4s ease-in-out infinite;
  }
}

@keyframes wfu-pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.35;
  }
}
```

- [ ] **Step 4: Chạy test, thấy qua**

Run: `npx vitest run src/errors src/components/components.test.tsx` rồi `npm run typecheck`
Expected: `Tests  10 passed (10)` (6 errors + 4 components); typecheck không lỗi.

- [ ] **Step 5: Commit**

```bash
rtk git add src/errors/errors.ts
rtk git add src/errors/errors.test.ts
rtk git add src/components/Busy.tsx
rtk git add src/components/NoticeBar.tsx
rtk git add src/components/ErrorBoundary.tsx
rtk git add src/components/components.test.tsx
rtk git add src/styles.css
rtk git commit -m "feat(ui): băng đỏ/hổ phách gộp trùng tối đa 5, móc lỗi ngoài luồng, vòng quay tôn trọng reduced-motion

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 13: Cầu nối Tauri, kho trạng thái và bộ điều khiển luồng

**Files:**
- Create: `src/api/tauri.ts`
- Create: `src/state/store.ts`
- Create: `src/state/controller.ts`
- Test: `src/api/tauri.test.ts`, `src/state/controller.test.ts`

**Interfaces:**
- Consumes:
  - Task 10: `Api`, `AppInfo`, `GroupScan`, `CleanEvent`, `CleanSummary`, `RestorePointStatus` (`src/api/types.ts`); `t`; `groupName(id)`, `noticeText(code)` (`src/catalog.ts`).
  - Task 11: `State`, `Action`, `initialState`, `reducer` (`src/state/machine.ts`) — các action dùng: `APP_INFO{dryRun, systemDrive}`, `FREE_SPACE{bytes}`, `FREE_SPACE_FAILED`, `SCAN_STARTED`, `SCAN_GROUP_DONE{group}`, `SCAN_FINISHED{groups}`, `SCAN_CANCEL_REQUESTED`, `SCAN_ABORTED`, `TOGGLE{id}`, `REQUEST_CLEAN`, `CONFIRM_ACCEPTED`, `CONFIRM_CANCELLED`, `RESTORE_RESULT{status}`, `RESTORE_CONTINUE`, `RESTORE_ABORT`, `CLEAN_EVENT{event}`, `CLEAN_DONE{summary}`, `CLEAN_FAILED`, `FREE_AFTER{bytes}`, `FREE_AFTER_FAILED`, `RESET`.
  - Task 12: `Severity`, `messageOf(e: unknown): string` (`src/errors/errors.ts`).
  - Vỏ Tauri (Task 17) sẽ cung cấp đúng các lệnh/sự kiện: `app_info`, `disk_free {drive: string | null}`, `scan_all`, `cancel_scan`, `prepare_restore_point {ids, dryRun}`, `clean {ids, dryRun}`, `open_log_folder`; sự kiện `scan-progress` (payload `GroupScan`), `clean-progress` (payload `CleanEvent`). Lỗi của lệnh là **chuỗi**; chuỗi `"busy"` nghĩa là đang có thao tác khác.
- Produces:
  - `src/api/tauri.ts`: `tauriApi: Api`.
  - `src/state/store.ts`: `interface Store<S, A> { getState(): S; dispatch(a: A): void; subscribe(l: () => void): () => void }`, `createStore<S, A>(reducer: (s: S, a: A) => S, initial: S): Store<S, A>` (chỉ báo khi trạng thái đổi tham chiếu).
  - `src/state/controller.ts`: `type Notify = (severity: Severity, message: string) => void`, `interface Deps { api: Api; store: Store<State, Action>; notify: Notify; session: { scanGen: number; scanRun: Promise<unknown> | null; cleaning: boolean } }`, `createDeps(api, store, notify): Deps`, `friendly(e: unknown): string`, và các hàm: `loadInitial(d): Promise<void>`, `refreshFree(d): Promise<void>`, `startScan(d): Promise<void>`, `cancelScan(d): Promise<void>`, `toggle(d, id: string): void`, `requestClean(d): Promise<void>`, `acceptConfirm(d): Promise<void>`, `cancelConfirm(d): void`, `continueAfterRestoreFailure(d): Promise<void>`, `abortAfterRestoreFailure(d): void`, `openLog(d): Promise<void>`, `goHome(d): Promise<void>`.

- [ ] **Step 1: Viết test hỏng**

`src/api/tauri.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invoke, listen, unlisten } = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn(), unlisten: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

import { tauriApi } from './tauri';

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  unlisten.mockReset();
});

describe('tauriApi', () => {
  it('clean gửi tham số camelCase, chuyển tiếp sự kiện và luôn gỡ lắng nghe kể cả khi lỗi', async () => {
    let handler: (e: { payload: unknown }) => void = () => {};
    listen.mockImplementation(async (_name: string, h: typeof handler) => {
      handler = h;
      return unlisten;
    });
    invoke.mockImplementation(async () => {
      handler({ payload: { kind: 'started', id: 'user_temp' } });
      throw 'busy';
    });
    const events: unknown[] = [];
    await expect(tauriApi.clean(['user_temp'], true, (e) => events.push(e))).rejects.toBe('busy');
    expect(listen).toHaveBeenCalledWith('clean-progress', expect.any(Function));
    expect(invoke).toHaveBeenCalledWith('clean', { ids: ['user_temp'], dryRun: true });
    expect(events).toEqual([{ kind: 'started', id: 'user_temp' }]);
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('scanAll lắng nghe scan-progress và trả kết quả cuối', async () => {
    listen.mockResolvedValue(unlisten);
    invoke.mockResolvedValue([{ id: 'a' }]);
    await expect(tauriApi.scanAll(() => {})).resolves.toEqual([{ id: 'a' }]);
    expect(listen).toHaveBeenCalledWith('scan-progress', expect.any(Function));
    expect(invoke).toHaveBeenCalledWith('scan_all');
    expect(unlisten).toHaveBeenCalled();
  });

  it('các lệnh đơn giản', async () => {
    invoke.mockResolvedValue(0);
    await tauriApi.diskFree();
    await tauriApi.prepareRestorePoint(['wu_download'], false);
    await tauriApi.cancelScan();
    await tauriApi.openLogFolder();
    await tauriApi.appInfo();
    expect(invoke.mock.calls).toEqual([
      ['disk_free', { drive: null }],
      ['prepare_restore_point', { ids: ['wu_download'], dryRun: false }],
      ['cancel_scan'],
      ['open_log_folder'],
      ['app_info'],
    ]);
  });
});
```

`src/state/controller.test.ts`:

```ts
import { describe, expect, it, vi } from 'vitest';
import type { Api, CleanSummary, GroupScan } from '../api/types';
import { initialState, reducer } from './machine';
import { createStore } from './store';
import * as c from './controller';

const GB = 1024 ** 3;

function scan(id: string, risk: GroupScan['risk'], bytes: number, extra: Partial<GroupScan> = {}): GroupScan {
  return {
    id,
    risk,
    default_selected: risk === 'safe',
    result: { total_bytes: bytes, file_count: 1, top_items: [], estimated: false, notices: [] },
    error: null,
    ...extra,
  };
}

const SCANS: GroupScan[] = [
  scan('user_temp', 'safe', GB),
  scan('browser_cache', 'safe', GB, { result: { total_bytes: GB, file_count: 1, top_items: [], estimated: false, notices: ['browser_running:chrome'] } }),
  scan('delivery_opt', 'safe', 0, { result: null, error: 'Access is denied. (os error 5)' }),
  scan('recycle_bin', 'caution', GB),
];

function summary(ids: string[], extra: Partial<CleanSummary> = {}): CleanSummary {
  return {
    groups: ids.map((id) => ({ id, report: { bytes_freed: GB, files_deleted: 2, skipped_locked: 0, errors: [], dry_run: false }, error: null })),
    log_path: 'C:\\logs\\x.log',
    dry_run: false,
    log_write_failed: false,
    ...extra,
  };
}

function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

function setup(over: Partial<Api> = {}) {
  const api: Api = {
    appInfo: vi.fn(async () => ({ version: '0.1.0', dry_run: false, system_drive: 'C:\\' })),
    diskFree: vi.fn(async () => 50 * GB),
    scanAll: vi.fn(async (onGroup: (g: GroupScan) => void) => {
      SCANS.forEach(onGroup);
      return SCANS;
    }),
    cancelScan: vi.fn(async () => {}),
    prepareRestorePoint: vi.fn(async () => ({ status: 'created' as const })),
    clean: vi.fn(async (ids: string[]) => summary(ids)),
    openLogFolder: vi.fn(async () => {}),
    ...over,
  };
  const store = createStore(reducer, initialState);
  const notify = vi.fn();
  const d = c.createDeps(api, store, notify);
  return { api, store, notify, d };
}

describe('khởi động', () => {
  it('đọc thông tin ứng dụng và dung lượng trống', async () => {
    const { d, store } = setup();
    await c.loadInitial(d);
    expect(store.getState().freeBefore).toBe(50 * GB);
    expect(store.getState().systemDrive).toBe('C:\\');
  });

  it('không đọc được dung lượng thì báo hổ phách, không treo vòng quay', async () => {
    const { d, store, notify } = setup({ diskFree: vi.fn(async () => Promise.reject('The device is not ready.')) });
    await c.loadInitial(d);
    expect(store.getState().freeFailed).toBe(true);
    expect(notify).toHaveBeenCalledWith('warning', 'Không đọc được dung lượng ổ đĩa: The device is not ready.');
  });
});

describe('quét', () => {
  it('xong thì sang xem trước; nhóm lỗi và trình duyệt đang mở lên băng hổ phách', async () => {
    const { d, store, notify } = setup();
    await c.startScan(d);
    expect(store.getState().phase).toBe('preview');
    expect(notify).toHaveBeenCalledWith('warning', 'Bản cập nhật chia sẻ: Access is denied. (os error 5)');
    expect(notify).toHaveBeenCalledWith('warning', expect.stringContaining('Chrome đang mở'));
  });

  it('quét hỏng hẳn thì băng đỏ và về màn Chào', async () => {
    const { d, store, notify } = setup({ scanAll: vi.fn(async () => Promise.reject('core crashed')) });
    await c.startScan(d);
    expect(store.getState().phase).toBe('welcome');
    expect(notify).toHaveBeenCalledWith('error', 'Không quét được: core crashed');
  });

  it('hủy: chờ lõi dừng hẳn rồi về Chào; kết quả và sự kiện đến muộn bị bỏ qua', async () => {
    const run = deferred<GroupScan[]>();
    let onGroup: (g: GroupScan) => void = () => {};
    const { d, store, api } = setup({
      scanAll: vi.fn((cb: (g: GroupScan) => void) => {
        onGroup = cb;
        return run.promise;
      }),
    });
    const scanning = c.startScan(d);
    expect(store.getState().phase).toBe('scanning');
    const cancelling = c.cancelScan(d);
    expect(store.getState().cancelling).toBe(true);
    onGroup(SCANS[0]);
    run.resolve(SCANS);
    await cancelling;
    await scanning;
    expect(api.cancelScan).toHaveBeenCalled();
    expect(store.getState().phase).toBe('welcome');
    expect(store.getState().groups).toEqual([]);
  });
});

describe('dọn', () => {
  it('chỉ nhóm An toàn: không hỏi, không tạo điểm khôi phục, đo dung lượng trước và sau', async () => {
    const free = [50 * GB, 50 * GB, 52 * GB];
    const { d, store, api } = setup({ diskFree: vi.fn(async () => free.shift() ?? 0) });
    await c.loadInitial(d);
    await c.startScan(d);
    await c.requestClean(d);
    expect(api.prepareRestorePoint).not.toHaveBeenCalled();
    expect(api.clean).toHaveBeenCalledWith(['user_temp', 'browser_cache'], false, expect.any(Function));
    expect(store.getState().phase).toBe('result');
    expect(store.getState().freeBefore).toBe(50 * GB);
    expect(store.getState().freeAfter).toBe(52 * GB);
  });

  it('có nhóm Cân nhắc: hỏi, đồng ý thì tạo điểm khôi phục rồi dọn', async () => {
    const { d, store, api } = setup();
    await c.startScan(d);
    c.toggle(d, 'recycle_bin');
    await c.requestClean(d);
    expect(store.getState().phase).toBe('confirm');
    expect(api.clean).not.toHaveBeenCalled();
    await c.acceptConfirm(d);
    expect(api.prepareRestorePoint).toHaveBeenCalledWith(['user_temp', 'browser_cache', 'recycle_bin'], false);
    expect(api.clean).toHaveBeenCalledTimes(1);
    expect(store.getState().phase).toBe('result');
  });

  it('không tạo được điểm khôi phục: chờ người dùng; dừng thì không dọn', async () => {
    const { d, store, api } = setup({ prepareRestorePoint: vi.fn(async () => ({ status: 'failed' as const, message: 'System Protection is off' })) });
    await c.startScan(d);
    c.toggle(d, 'recycle_bin');
    await c.requestClean(d);
    await c.acceptConfirm(d);
    expect(store.getState().phase).toBe('restorePoint');
    expect(api.clean).not.toHaveBeenCalled();
    c.abortAfterRestoreFailure(d);
    expect(store.getState().phase).toBe('preview');
    expect(api.clean).not.toHaveBeenCalled();
  });

  it('không tạo được điểm khôi phục: chọn dọn tiếp thì dọn', async () => {
    const { d, store, api } = setup({ prepareRestorePoint: vi.fn(async () => ({ status: 'failed' as const, message: 'x' })) });
    await c.startScan(d);
    c.toggle(d, 'recycle_bin');
    await c.requestClean(d);
    await c.acceptConfirm(d);
    await c.continueAfterRestoreFailure(d);
    expect(api.clean).toHaveBeenCalledTimes(1);
    expect(store.getState().phase).toBe('result');
  });

  it('lõi báo bận thì băng đỏ câu dễ hiểu và quay về xem trước', async () => {
    const { d, store, notify } = setup({ clean: vi.fn(async () => Promise.reject('busy')) });
    await c.startScan(d);
    await c.requestClean(d);
    expect(store.getState().phase).toBe('preview');
    expect(notify).toHaveBeenCalledWith('error', 'Không dọn được: WinFreeUp đang bận với thao tác trước, hãy đợi xong rồi thử lại.');
  });

  it('lỗi từng nhóm, lỗi từng file và nhật ký hỏng đều lên băng hổ phách', async () => {
    const s: CleanSummary = {
      groups: [
        { id: 'user_temp', report: { bytes_freed: 1, files_deleted: 1, skipped_locked: 0, errors: ['C:\\x: denied'], dry_run: false }, error: null },
        { id: 'browser_cache', report: null, error: 'panic: boom' },
      ],
      log_path: 'x',
      dry_run: false,
      log_write_failed: true,
    };
    const { d, notify } = setup({ clean: vi.fn(async () => s) });
    await c.startScan(d);
    await c.requestClean(d);
    expect(notify).toHaveBeenCalledWith('warning', 'File tạm của bạn: 1 lỗi, ví dụ: C:\\x: denied');
    expect(notify).toHaveBeenCalledWith('warning', 'Bộ nhớ đệm trình duyệt: panic: boom');
    expect(notify).toHaveBeenCalledWith('warning', 'Không ghi được đầy đủ nhật ký lượt dọn này.');
  });

  it('chế độ chạy thử truyền dryRun=true', async () => {
    const { d, api } = setup({ appInfo: vi.fn(async () => ({ version: '0.1.0', dry_run: true, system_drive: 'C:\\' })) });
    await c.loadInitial(d);
    await c.startScan(d);
    await c.requestClean(d);
    expect(api.clean).toHaveBeenCalledWith(expect.any(Array), true, expect.any(Function));
  });

  it('mở nhật ký hỏng thì báo hổ phách', async () => {
    const { d, notify } = setup({ openLogFolder: vi.fn(async () => Promise.reject('explorer.exe not found')) });
    await c.openLog(d);
    expect(notify).toHaveBeenCalledWith('warning', 'Không mở được thư mục nhật ký: explorer.exe not found');
  });
});
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `npx vitest run src/api src/state/controller.test.ts`
Expected: FAIL — `Failed to resolve import "./tauri"`, `"./store"`, `"./controller"`.

- [ ] **Step 3: Viết mã**

`src/api/tauri.ts`:

```ts
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Api, AppInfo, CleanEvent, CleanSummary, GroupScan, RestorePointStatus } from './types';

// Tauri đổi tham số camelCase ở JS sang snake_case ở Rust (dryRun → dry_run).
export const tauriApi: Api = {
  appInfo: () => invoke<AppInfo>('app_info'),
  diskFree: () => invoke<number>('disk_free', { drive: null }),
  async scanAll(onGroup) {
    const unlisten = await listen<GroupScan>('scan-progress', (e) => onGroup(e.payload));
    try {
      return await invoke<GroupScan[]>('scan_all');
    } finally {
      unlisten();
    }
  },
  cancelScan: () => invoke<void>('cancel_scan'),
  prepareRestorePoint: (ids, dryRun) => invoke<RestorePointStatus>('prepare_restore_point', { ids, dryRun }),
  async clean(ids, dryRun, onEvent) {
    const unlisten = await listen<CleanEvent>('clean-progress', (e) => onEvent(e.payload));
    try {
      return await invoke<CleanSummary>('clean', { ids, dryRun });
    } finally {
      unlisten();
    }
  },
  openLogFolder: () => invoke<void>('open_log_folder'),
};
```

`src/state/store.ts`:

```ts
export interface Store<S, A> {
  getState(): S;
  dispatch(a: A): void;
  subscribe(listener: () => void): () => void;
}

export function createStore<S, A>(reducer: (s: S, a: A) => S, initial: S): Store<S, A> {
  let state = initial;
  const listeners = new Set<() => void>();
  return {
    getState: () => state,
    dispatch(a) {
      const next = reducer(state, a);
      if (next !== state) {
        state = next;
        listeners.forEach((l) => l());
      }
    },
    subscribe(l) {
      listeners.add(l);
      return () => {
        listeners.delete(l);
      };
    },
  };
}
```

`src/state/controller.ts`:

```ts
import type { Api, CleanEvent, CleanSummary, GroupScan, RestorePointStatus } from '../api/types';
import { groupName, noticeText } from '../catalog';
import { messageOf, type Severity } from '../errors/errors';
import { t } from '../i18n';
import type { Action, State } from './machine';
import type { Store } from './store';

export type Notify = (severity: Severity, message: string) => void;

export interface Deps {
  api: Api;
  store: Store<State, Action>;
  notify: Notify;
  session: { scanGen: number; scanRun: Promise<unknown> | null; cleaning: boolean };
}

export function createDeps(api: Api, store: Store<State, Action>, notify: Notify): Deps {
  return { api, store, notify, session: { scanGen: 0, scanRun: null, cleaning: false } };
}

/** Lỗi lệnh Tauri là chuỗi; "busy" đổi sang câu dễ hiểu, còn lại giữ nguyên văn. */
export function friendly(e: unknown): string {
  const m = messageOf(e);
  return m === 'busy' ? t('errors.busy') : m;
}

export async function loadInitial(d: Deps): Promise<void> {
  try {
    const info = await d.api.appInfo();
    d.store.dispatch({ type: 'APP_INFO', dryRun: info.dry_run, systemDrive: info.system_drive });
  } catch (e) {
    d.notify('warning', t('errors.appInfoFailed', { message: friendly(e) }));
  }
  await refreshFree(d);
}

export async function refreshFree(d: Deps): Promise<void> {
  try {
    d.store.dispatch({ type: 'FREE_SPACE', bytes: await d.api.diskFree() });
  } catch (e) {
    d.store.dispatch({ type: 'FREE_SPACE_FAILED' });
    d.notify('warning', t('errors.freeSpaceFailed', { message: friendly(e) }));
  }
}

function reportScanIssues(d: Deps, groups: GroupScan[]): void {
  for (const g of groups) {
    if (g.error && g.error !== 'cancelled') {
      d.notify('warning', t('notice.groupFailed', { name: groupName(g.id), message: g.error }));
    }
    for (const n of g.result?.notices ?? []) d.notify('warning', noticeText(n));
  }
}

export async function startScan(d: Deps): Promise<void> {
  d.store.dispatch({ type: 'SCAN_STARTED' });
  if (d.store.getState().phase !== 'scanning') return;
  const gen = ++d.session.scanGen;
  const run = d.api.scanAll((g) => {
    if (gen === d.session.scanGen) d.store.dispatch({ type: 'SCAN_GROUP_DONE', group: g });
  });
  d.session.scanRun = run;
  try {
    const groups = await run;
    if (gen !== d.session.scanGen) return;
    d.store.dispatch({ type: 'SCAN_FINISHED', groups });
    reportScanIssues(d, groups);
  } catch (e) {
    if (gen !== d.session.scanGen) return;
    d.store.dispatch({ type: 'SCAN_ABORTED' });
    d.notify('error', t('errors.scanFailed', { message: friendly(e) }));
  } finally {
    if (d.session.scanRun === run) d.session.scanRun = null;
  }
}

/** Giữ vòng quay «Đang hủy…» cho tới khi lõi dừng hẳn, để lần Quét kế tiếp không bị «busy». */
export async function cancelScan(d: Deps): Promise<void> {
  if (d.store.getState().phase !== 'scanning') return;
  d.store.dispatch({ type: 'SCAN_CANCEL_REQUESTED' });
  d.session.scanGen++;
  const run = d.session.scanRun;
  try {
    await d.api.cancelScan();
  } catch (e) {
    d.notify('warning', friendly(e));
  }
  if (run) await run.catch(() => undefined);
  d.store.dispatch({ type: 'SCAN_ABORTED' });
}

export function toggle(d: Deps, id: string): void {
  d.store.dispatch({ type: 'TOGGLE', id });
}

export async function requestClean(d: Deps): Promise<void> {
  d.store.dispatch({ type: 'REQUEST_CLEAN' });
  await advance(d);
}

export async function acceptConfirm(d: Deps): Promise<void> {
  d.store.dispatch({ type: 'CONFIRM_ACCEPTED' });
  await advance(d);
}

export function cancelConfirm(d: Deps): void {
  d.store.dispatch({ type: 'CONFIRM_CANCELLED' });
}

export async function continueAfterRestoreFailure(d: Deps): Promise<void> {
  d.store.dispatch({ type: 'RESTORE_CONTINUE' });
  await advance(d);
}

export function abortAfterRestoreFailure(d: Deps): void {
  d.store.dispatch({ type: 'RESTORE_ABORT' });
}

async function advance(d: Deps): Promise<void> {
  const s = d.store.getState();
  if (s.phase === 'restorePoint' && s.restore === null) return runRestore(d);
  if (s.phase === 'cleaning' && s.summary === null && !d.session.cleaning) return runClean(d);
}

async function runRestore(d: Deps): Promise<void> {
  const s = d.store.getState();
  let status: RestorePointStatus;
  try {
    status = await d.api.prepareRestorePoint(s.selected, s.dryRun);
  } catch (e) {
    status = { status: 'failed', message: friendly(e) };
  }
  d.store.dispatch({ type: 'RESTORE_RESULT', status });
  await advance(d);
}

async function runClean(d: Deps): Promise<void> {
  d.session.cleaning = true;
  try {
    const s = d.store.getState();
    try {
      d.store.dispatch({ type: 'FREE_SPACE', bytes: await d.api.diskFree() });
    } catch (e) {
      d.notify('warning', t('errors.freeSpaceFailed', { message: friendly(e) }));
    }
    let summary: CleanSummary;
    try {
      summary = await d.api.clean(s.selected, s.dryRun, (ev: CleanEvent) => d.store.dispatch({ type: 'CLEAN_EVENT', event: ev }));
    } catch (e) {
      d.store.dispatch({ type: 'CLEAN_FAILED' });
      d.notify('error', t('errors.cleanFailed', { message: friendly(e) }));
      return;
    }
    d.store.dispatch({ type: 'CLEAN_DONE', summary });
    for (const g of summary.groups) {
      if (g.error) {
        d.notify('warning', t('notice.groupFailed', { name: groupName(g.id), message: g.error }));
      } else if (g.report && g.report.errors.length > 0) {
        d.notify('warning', t('notice.groupErrors', { name: groupName(g.id), count: g.report.errors.length, first: g.report.errors[0] }));
      }
    }
    if (summary.log_write_failed) d.notify('warning', t('notice.logWriteFailed'));
    try {
      d.store.dispatch({ type: 'FREE_AFTER', bytes: await d.api.diskFree() });
    } catch (e) {
      d.store.dispatch({ type: 'FREE_AFTER_FAILED' });
      d.notify('warning', t('errors.freeSpaceFailed', { message: friendly(e) }));
    }
  } finally {
    d.session.cleaning = false;
  }
}

export async function openLog(d: Deps): Promise<void> {
  try {
    await d.api.openLogFolder();
  } catch (e) {
    d.notify('warning', t('errors.openLogFailed', { message: friendly(e) }));
  }
}

export async function goHome(d: Deps): Promise<void> {
  d.store.dispatch({ type: 'RESET' });
  await refreshFree(d);
}
```

- [ ] **Step 4: Chạy test, thấy qua**

Run: `npx vitest run src/api src/state/controller.test.ts` rồi `npm run typecheck`
Expected: `Tests  16 passed (16)`; typecheck không lỗi.

- [ ] **Step 5: Commit**

```bash
rtk git add src/api/tauri.ts
rtk git add src/api/tauri.test.ts
rtk git add src/state/store.ts
rtk git add src/state/controller.ts
rtk git add src/state/controller.test.ts
rtk git commit -m "feat(ui): cầu nối Tauri, kho trạng thái, bộ điều khiển luồng quét/xác nhận/điểm khôi phục/dọn

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 14: Màn Chào, Đang quét, Xem trước, hộp Xác nhận

**Files:**
- Create: `src/components/WelcomeView.tsx`
- Create: `src/components/ScanningView.tsx`
- Create: `src/components/PreviewView.tsx`
- Create: `src/components/ConfirmDialog.tsx`
- Test: `src/components/views1.test.tsx`

**Interfaces:**
- Consumes:
  - Task 10: `t`, `groupName`, `groupDesc`, `formatBytes`, kiểu `GroupScan`, `RiskLevel`.
  - Task 11: `State`, `GroupState`, `initialState`, `reducer` (test dựng trạng thái bằng reducer); `selectable(g)`, `selectedBytes(s)`, `confirmKind(groups, selected)`, `isConfirmWord(input)`.
  - Task 12: `Busy({ label?, size? })`; lớp CSS `wfu-hero, wfu-actions, wfu-groups, wfu-group, wfu-group-main, wfu-group-title, wfu-group-desc, wfu-group-size, wfu-items, wfu-item, wfu-item-path, wfu-muted`.
- Produces (App ở Task 16 dùng nguyên văn):
  - `WelcomeView({ state: State; onScan: () => void })`
  - `ScanningView({ state: State; onCancel: () => void })`
  - `PreviewView({ state: State; onToggle: (id: string) => void; onClean: () => void; onRescan: () => void })`
  - `ConfirmDialog({ state: State; onAccept: () => void; onCancel: () => void })` — chỉ được vẽ khi `phase === 'confirm'` (ô gõ XOA tự xoá khi đóng).

- [ ] **Step 1: Viết test hỏng**

`src/components/views1.test.tsx`:

```tsx
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import type { ReactNode } from 'react';
import type { GroupScan } from '../api/types';
import { initialState, reducer, type Action, type State } from '../state/machine';
import { WelcomeView } from './WelcomeView';
import { ScanningView } from './ScanningView';
import { PreviewView } from './PreviewView';
import { ConfirmDialog } from './ConfirmDialog';

const GB = 1024 ** 3;
const wrap = (ui: ReactNode) => render(<FluentProvider theme={webLightTheme}>{ui}</FluentProvider>);
const run = (s: State, ...a: Action[]) => a.reduce(reducer, s);

function scan(id: string, risk: GroupScan['risk'], bytes: number, extra: Partial<GroupScan> = {}): GroupScan {
  return {
    id,
    risk,
    default_selected: risk === 'safe',
    result: {
      total_bytes: bytes,
      file_count: 2,
      top_items: [
        { path: 'C:\\Users\\a\\AppData\\Local\\Temp\\setup-lon.exe', bytes: bytes * 0.75 },
        { path: 'C:\\Users\\a\\AppData\\Local\\Temp\\nho.tmp', bytes: bytes * 0.25 },
      ],
      estimated: false,
      notices: [],
    },
    error: null,
    ...extra,
  };
}

const SCANS = [
  scan('user_temp', 'safe', GB),
  scan('browser_cache', 'safe', 0, { result: null, error: 'Access is denied. (os error 5)' }),
  scan('component_store', 'caution', 2 * GB, { result: { total_bytes: 2 * GB, file_count: 0, top_items: [], estimated: true, notices: [] } }),
  scan('recycle_bin', 'caution', GB / 2),
  scan('windows_old', 'risky', 10 * GB),
];
const preview = () => run(initialState, { type: 'SCAN_STARTED' }, { type: 'SCAN_FINISHED', groups: SCANS });

describe('WelcomeView', () => {
  it('đang đọc dung lượng thì có vòng quay; đọc xong thì hiện số', () => {
    const { rerender } = wrap(<WelcomeView state={initialState} onScan={() => {}} />);
    expect(screen.getByRole('progressbar')).toBeTruthy();
    rerender(
      <FluentProvider theme={webLightTheme}>
        <WelcomeView state={reducer(initialState, { type: 'FREE_SPACE', bytes: 1.5 * GB })} onScan={() => {}} />
      </FluentProvider>,
    );
    expect(screen.getByText('Ổ C: còn trống 1,5 GB')).toBeTruthy();
  });

  it('không đọc được thì nói rõ, không quay mãi', () => {
    wrap(<WelcomeView state={reducer(initialState, { type: 'FREE_SPACE_FAILED' })} onScan={() => {}} />);
    expect(screen.queryByRole('progressbar')).toBeNull();
    expect(screen.getByText('Chưa đọc được dung lượng trống của ổ đĩa.')).toBeTruthy();
  });

  it('bấm Quét', () => {
    const onScan = vi.fn();
    wrap(<WelcomeView state={initialState} onScan={onScan} />);
    fireEvent.click(screen.getByRole('button', { name: 'Quét' }));
    expect(onScan).toHaveBeenCalled();
  });
});

describe('ScanningView', () => {
  it('mỗi nhóm đang chờ có vòng quay riêng; nhóm xong hiện dung lượng; nút Hủy', () => {
    const s = run(initialState, { type: 'SCAN_STARTED' }, { type: 'SCAN_GROUP_DONE', group: SCANS[0] });
    const onCancel = vi.fn();
    wrap(<ScanningView state={s} onCancel={onCancel} />);
    expect(screen.getAllByRole('progressbar')).toHaveLength(8);
    expect(screen.getByText('1 GB')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Hủy' }));
    expect(onCancel).toHaveBeenCalled();
  });

  it('đang hủy thì thay nút bằng vòng quay «Đang hủy…»', () => {
    const s = run(initialState, { type: 'SCAN_STARTED' }, { type: 'SCAN_CANCEL_REQUESTED' });
    wrap(<ScanningView state={s} onCancel={() => {}} />);
    expect(screen.queryByRole('button', { name: 'Hủy' })).toBeNull();
    expect(screen.getByText('Đang hủy…')).toBeTruthy();
  });
});

describe('PreviewView', () => {
  it('nút Dọn luôn đúng số đang chọn', () => {
    const s = preview();
    const { rerender } = wrap(<PreviewView state={s} onToggle={() => {}} onClean={() => {}} onRescan={() => {}} />);
    expect(screen.getByRole('button', { name: 'Dọn 1 GB' })).toBeTruthy();
    rerender(
      <FluentProvider theme={webLightTheme}>
        <PreviewView state={reducer(s, { type: 'TOGGLE', id: 'recycle_bin' })} onToggle={() => {}} onClean={() => {}} onRescan={() => {}} />
      </FluentProvider>,
    );
    expect(screen.getByRole('button', { name: 'Dọn 1,5 GB' })).toBeTruthy();
  });

  it('tích ô gọi onToggle; nhóm lỗi không tích được và hiện lỗi nguyên văn', () => {
    const onToggle = vi.fn();
    wrap(<PreviewView state={preview()} onToggle={onToggle} onClean={() => {}} onRescan={() => {}} />);
    fireEvent.click(screen.getByRole('checkbox', { name: 'Thùng rác' }));
    expect(onToggle).toHaveBeenCalledWith('recycle_bin');
    expect((screen.getByRole('checkbox', { name: 'Bộ nhớ đệm trình duyệt' }) as HTMLInputElement).disabled).toBe(true);
    expect(screen.getByText('Không quét được nhóm này: Access is denied. (os error 5)')).toBeTruthy();
  });

  it('nhãn mức rủi ro, số ước tính và danh sách 20 mục lớn nhất', () => {
    wrap(<PreviewView state={preview()} onToggle={() => {}} onClean={() => {}} onRescan={() => {}} />);
    expect(screen.getByText('Rủi ro')).toBeTruthy();
    expect(screen.getAllByText('Cân nhắc')).toHaveLength(2);
    expect(screen.getByText('khoảng 2 GB')).toBeTruthy();
    fireEvent.click(screen.getAllByRole('button', { name: 'Xem 20 mục lớn nhất' })[0]);
    expect(screen.getByText('C:\\Users\\a\\AppData\\Local\\Temp\\setup-lon.exe')).toBeTruthy();
  });

  it('bỏ hết lựa chọn thì nút Dọn bị khoá', () => {
    const s = reducer(preview(), { type: 'TOGGLE', id: 'user_temp' });
    wrap(<PreviewView state={s} onToggle={() => {}} onClean={() => {}} onRescan={() => {}} />);
    expect((screen.getByRole('button', { name: 'Dọn 0 MB' }) as HTMLButtonElement).disabled).toBe(true);
  });
});

describe('ConfirmDialog', () => {
  it('nhóm Cân nhắc: liệt kê nhóm, nút Dọn bấm được ngay', () => {
    const s = run(preview(), { type: 'TOGGLE', id: 'recycle_bin' }, { type: 'REQUEST_CLEAN' });
    const onAccept = vi.fn();
    wrap(<ConfirmDialog state={s} onAccept={onAccept} onCancel={() => {}} />);
    expect(screen.getByText('Thùng rác')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Dọn' }));
    expect(onAccept).toHaveBeenCalled();
  });

  // Review Focus 1
  it('nhóm Rủi ro: phải gõ XOA — chấp nhận «xóa», không chấp nhận «XO»', async () => {
    const user = userEvent.setup();
    const s = run(preview(), { type: 'TOGGLE', id: 'windows_old' }, { type: 'REQUEST_CLEAN' });
    wrap(<ConfirmDialog state={s} onAccept={() => {}} onCancel={() => {}} />);
    const ok = () => screen.getByRole('button', { name: 'Dọn' }) as HTMLButtonElement;
    const box = screen.getByRole('textbox', { name: 'Gõ XOA để xác nhận' });
    expect(ok().disabled).toBe(true);
    await user.type(box, 'XO');
    expect(ok().disabled).toBe(true);
    await user.clear(box);
    await user.type(box, 'xóa');
    expect(ok().disabled).toBe(false);
  });

  it('Quay lại gọi onCancel', () => {
    const s = run(preview(), { type: 'TOGGLE', id: 'recycle_bin' }, { type: 'REQUEST_CLEAN' });
    const onCancel = vi.fn();
    wrap(<ConfirmDialog state={s} onAccept={() => {}} onCancel={onCancel} />);
    fireEvent.click(screen.getByRole('button', { name: 'Quay lại' }));
    expect(onCancel).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `npx vitest run src/components/views1.test.tsx`
Expected: FAIL — `Failed to resolve import "./WelcomeView"`.

- [ ] **Step 3: Viết mã**

`src/components/WelcomeView.tsx`:

```tsx
import { Body1, Button, Title3 } from '@fluentui/react-components';
import { formatBytes } from '../format';
import { t } from '../i18n';
import type { State } from '../state/machine';
import { Busy } from './Busy';

export function WelcomeView({ state, onScan }: { state: State; onScan: () => void }) {
  const drive = state.systemDrive.replace(/\\$/, '');
  return (
    <section className="wfu-hero">
      <Body1>{t('welcome.intro')}</Body1>
      <div>
        {state.freeBefore !== null ? (
          <Title3>{t('welcome.freeSpace', { drive, size: formatBytes(state.freeBefore) })}</Title3>
        ) : state.freeFailed ? (
          <Body1 className="wfu-muted">{t('welcome.freeUnknown')}</Body1>
        ) : (
          <Busy label={t('welcome.loadingFree')} />
        )}
      </div>
      <div className="wfu-actions">
        <Button appearance="primary" size="large" onClick={onScan}>
          {t('welcome.scan')}
        </Button>
      </div>
    </section>
  );
}
```

`src/components/ScanningView.tsx`:

```tsx
import { Button, Title3 } from '@fluentui/react-components';
import { groupName } from '../catalog';
import { formatBytes } from '../format';
import { t } from '../i18n';
import type { State } from '../state/machine';
import { Busy } from './Busy';

export function ScanningView({ state, onCancel }: { state: State; onCancel: () => void }) {
  return (
    <section className="wfu-hero">
      <Title3>{t('scanning.title')}</Title3>
      <ul className="wfu-groups">
        {state.groups.map((g) => (
          <li key={g.id} className="wfu-group">
            <span>{g.status === 'pending' ? <Busy /> : g.status === 'error' ? '!' : '✓'}</span>
            <div className="wfu-group-main">
              <span className="wfu-group-title">{groupName(g.id)}</span>
            </div>
            <span className="wfu-group-size">
              {g.status === 'pending'
                ? t('scanning.pending')
                : g.status === 'error'
                  ? t('scanning.failed')
                  : formatBytes(g.result?.total_bytes ?? 0)}
            </span>
          </li>
        ))}
      </ul>
      <div className="wfu-actions">
        {state.cancelling ? <Busy label={t('scanning.cancelling')} /> : <Button onClick={onCancel}>{t('scanning.cancel')}</Button>}
      </div>
    </section>
  );
}
```

`src/components/PreviewView.tsx`:

```tsx
import { useState } from 'react';
import { Badge, Body1, Button, Checkbox, Title3 } from '@fluentui/react-components';
import type { RiskLevel } from '../api/types';
import { groupDesc, groupName } from '../catalog';
import { formatBytes } from '../format';
import { t } from '../i18n';
import type { GroupState, State } from '../state/machine';
import { selectable, selectedBytes } from '../state/selectors';

const RISK_COLOR: Record<RiskLevel, 'success' | 'warning' | 'danger'> = {
  safe: 'success',
  caution: 'warning',
  risky: 'danger',
};

function GroupRow({ g, checked, onToggle }: { g: GroupState; checked: boolean; onToggle: (id: string) => void }) {
  const [open, setOpen] = useState(false);
  const bytes = g.result?.total_bytes ?? 0;
  const size = g.result?.estimated ? t('preview.estimated', { size: formatBytes(bytes) }) : formatBytes(bytes);
  return (
    <li className="wfu-group">
      <Checkbox checked={checked} disabled={!selectable(g)} onChange={() => onToggle(g.id)} aria-label={groupName(g.id)} />
      <div className="wfu-group-main">
        <span className="wfu-group-title">
          {groupName(g.id)}
          {g.risk && (
            <Badge appearance="tint" color={RISK_COLOR[g.risk]}>
              {t(`risk.${g.risk}`)}
            </Badge>
          )}
        </span>
        <span className="wfu-group-desc">{groupDesc(g.id)}</span>
        {g.status === 'error' && <span className="wfu-muted">{t('preview.groupError', { message: g.error ?? '' })}</span>}
        {g.result && g.result.top_items.length > 0 && (
          <Button appearance="transparent" size="small" onClick={() => setOpen(!open)}>
            {open ? t('preview.hideItems') : t('preview.showItems')}
          </Button>
        )}
      </div>
      <span className="wfu-group-size">{g.status === 'error' ? '—' : size}</span>
      {open && g.result && (
        <ul className="wfu-items">
          {g.result.top_items.map((it) => (
            <li key={it.path} className="wfu-item">
              <span className="wfu-item-path" title={it.path}>
                {it.path}
              </span>
              <span>{formatBytes(it.bytes)}</span>
            </li>
          ))}
        </ul>
      )}
    </li>
  );
}

export function PreviewView({
  state,
  onToggle,
  onClean,
  onRescan,
}: {
  state: State;
  onToggle: (id: string) => void;
  onClean: () => void;
  onRescan: () => void;
}) {
  const anything = state.groups.some(selectable);
  return (
    <section className="wfu-hero">
      <Title3>{t('preview.title')}</Title3>
      {!anything && <Body1>{t('preview.nothing')}</Body1>}
      <ul className="wfu-groups">
        {state.groups.map((g) => (
          <GroupRow key={g.id} g={g} checked={state.selected.includes(g.id)} onToggle={onToggle} />
        ))}
      </ul>
      <div className="wfu-actions">
        <Button appearance="primary" disabled={state.selected.length === 0} onClick={onClean}>
          {t('preview.clean', { size: formatBytes(selectedBytes(state)) })}
        </Button>
        <Button onClick={onRescan}>{t('preview.rescan')}</Button>
      </div>
    </section>
  );
}
```

`src/components/ConfirmDialog.tsx`:

```tsx
import { useState } from 'react';
import {
  Body1,
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Field,
  Input,
} from '@fluentui/react-components';
import { groupDesc, groupName } from '../catalog';
import { t } from '../i18n';
import type { State } from '../state/machine';
import { confirmKind, isConfirmWord } from '../state/selectors';

export function ConfirmDialog({ state, onAccept, onCancel }: { state: State; onAccept: () => void; onCancel: () => void }) {
  const [word, setWord] = useState('');
  const kind = confirmKind(state.groups, state.selected);
  const listed = state.groups.filter((g) => state.selected.includes(g.id) && g.risk !== 'safe');
  const allowed = kind !== 'risky' || isConfirmWord(word);
  return (
    <Dialog open modalType="alert">
      <DialogSurface>
        <DialogBody>
          <DialogTitle>{t('confirm.title')}</DialogTitle>
          <DialogContent>
            <Body1>{t('confirm.cautionIntro')}</Body1>
            <ul>
              {listed.map((g) => (
                <li key={g.id}>
                  <strong>{groupName(g.id)}</strong>
                  {' — '}
                  {g.risk ? t(`risk.${g.risk}`) : ''}: {groupDesc(g.id)}
                </li>
              ))}
            </ul>
            {kind === 'risky' && (
              <>
                <Body1>{t('confirm.riskyIntro')}</Body1>
                <Field label={t('confirm.riskyLabel')}>
                  <Input value={word} onChange={(_, data) => setWord(data.value)} autoFocus />
                </Field>
              </>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onCancel}>
              {t('confirm.cancel')}
            </Button>
            <Button appearance="primary" disabled={!allowed} onClick={onAccept}>
              {t('confirm.ok')}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
```

- [ ] **Step 4: Chạy test, thấy qua**

Run: `npx vitest run src/components/views1.test.tsx` rồi `npm run typecheck`
Expected: `Tests  12 passed (12)`; typecheck không lỗi.

- [ ] **Step 5: Commit**

```bash
rtk git add src/components/WelcomeView.tsx
rtk git add src/components/ScanningView.tsx
rtk git add src/components/PreviewView.tsx
rtk git add src/components/ConfirmDialog.tsx
rtk git add src/components/views1.test.tsx
rtk git commit -m "feat(ui): màn Chào, Đang quét, Xem trước và hộp Xác nhận theo mức rủi ro

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 15: Màn Điểm khôi phục, Đang dọn, Kết quả

**Files:**
- Create: `src/components/RestorePointView.tsx`
- Create: `src/components/CleaningView.tsx`
- Create: `src/components/ResultView.tsx`
- Test: `src/components/views2.test.tsx`

**Interfaces:**
- Consumes:
  - Task 10: `t`, `groupName`, `formatBytes`, kiểu `GroupClean`, `CleanSummary`.
  - Task 11: `State`, `initialState`, `reducer`; `reclaimedBytes(before, after)`, `dryRunBytes(summary)`.
  - Task 12: `Busy`; lớp `wfu-hero, wfu-hero-number, wfu-header, wfu-actions, wfu-groups, wfu-group, wfu-group-main, wfu-group-title, wfu-group-size, wfu-muted, wfu-canhbao`.
- Produces (App ở Task 16 dùng nguyên văn):
  - `RestorePointView({ state: State; onContinue: () => void; onAbort: () => void })`
  - `CleaningView({ state: State })` — **không có nút nào** (khoá thao tác chồng).
  - `ResultView({ state: State; onOpenLog: () => Promise<void>; onHome: () => void })` — số lớn = `reclaimedBytes` (thật) hoặc `dryRunBytes` (chạy thử); chờ đo thì có vòng quay; nút «Xem nhật ký» có vòng quay khi đang mở.

- [ ] **Step 1: Viết test hỏng**

`src/components/views2.test.tsx`:

```tsx
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import type { ReactNode } from 'react';
import type { CleanSummary, GroupClean, GroupScan } from '../api/types';
import { initialState, reducer, type Action, type State } from '../state/machine';
import { RestorePointView } from './RestorePointView';
import { CleaningView } from './CleaningView';
import { ResultView } from './ResultView';

const GB = 1024 ** 3;
const wrap = (ui: ReactNode) => render(<FluentProvider theme={webLightTheme}>{ui}</FluentProvider>);
const run = (s: State, ...a: Action[]) => a.reduce(reducer, s);

const scan = (id: string, risk: GroupScan['risk'], bytes: number): GroupScan => ({
  id,
  risk,
  default_selected: risk === 'safe',
  result: { total_bytes: bytes, file_count: 1, top_items: [], estimated: false, notices: [] },
  error: null,
});

const SCANS = [scan('user_temp', 'safe', GB), scan('component_store', 'caution', 2 * GB), scan('recycle_bin', 'caution', GB)];

const ok = (id: string, bytes: number, skipped = 0): GroupClean => ({
  id,
  report: { bytes_freed: bytes, files_deleted: 3, skipped_locked: skipped, errors: [], dry_run: false },
  error: null,
});

function atRestorePoint(): State {
  return run(
    initialState,
    { type: 'SCAN_STARTED' },
    { type: 'SCAN_FINISHED', groups: SCANS },
    { type: 'TOGGLE', id: 'recycle_bin' },
    { type: 'REQUEST_CLEAN' },
    { type: 'CONFIRM_ACCEPTED' },
  );
}

function atResult(summary: CleanSummary, free: { before?: number; after?: number } = {}): State {
  let s = run(
    initialState,
    { type: 'FREE_SPACE', bytes: free.before ?? 10 * GB },
    { type: 'SCAN_STARTED' },
    { type: 'SCAN_FINISHED', groups: SCANS },
    { type: 'REQUEST_CLEAN' },
    { type: 'CLEAN_DONE', summary },
  );
  if (free.after !== undefined) s = reducer(s, { type: 'FREE_AFTER', bytes: free.after });
  return s;
}

const SUMMARY: CleanSummary = { groups: [ok('user_temp', GB, 4)], log_path: 'C:\\Users\\a\\AppData\\Local\\WinFreeUp\\logs\\2026-09-25_101010.log', dry_run: false, log_write_failed: false };

describe('RestorePointView', () => {
  it('đang tạo điểm khôi phục thì có vòng quay', () => {
    wrap(<RestorePointView state={atRestorePoint()} onContinue={() => {}} onAbort={() => {}} />);
    expect(screen.getByRole('progressbar')).toBeTruthy();
    expect(screen.getByText('Đang tạo điểm khôi phục hệ thống…')).toBeTruthy();
  });

  it('không tạo được: băng hổ phách nguyên văn, người dùng chọn dọn tiếp hay dừng', () => {
    const s = reducer(atRestorePoint(), { type: 'RESTORE_RESULT', status: { status: 'failed', message: 'System Protection is off' } });
    const onContinue = vi.fn();
    const onAbort = vi.fn();
    wrap(<RestorePointView state={s} onContinue={onContinue} onAbort={onAbort} />);
    expect(screen.getByText(/System Protection is off/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Vẫn dọn tiếp' }));
    fireEvent.click(screen.getByRole('button', { name: 'Dừng lại' }));
    expect(onContinue).toHaveBeenCalled();
    expect(onAbort).toHaveBeenCalled();
  });
});

describe('CleaningView', () => {
  it('tiến độ từng nhóm, DISM có thanh % riêng, không có nút nào', () => {
    let s = run(
      initialState,
      { type: 'SCAN_STARTED' },
      { type: 'SCAN_FINISHED', groups: SCANS },
      { type: 'TOGGLE', id: 'component_store' },
      { type: 'REQUEST_CLEAN' },
      { type: 'CONFIRM_ACCEPTED' },
      { type: 'RESTORE_RESULT', status: { status: 'created' } },
    );
    s = run(
      s,
      { type: 'CLEAN_EVENT', event: { kind: 'started', id: 'user_temp' } },
      { type: 'CLEAN_EVENT', event: { kind: 'finished', result: ok('user_temp', GB) } },
      { type: 'CLEAN_EVENT', event: { kind: 'started', id: 'component_store' } },
      { type: 'CLEAN_EVENT', event: { kind: 'percent', id: 'component_store', percent: 55.5 } },
    );
    wrap(<CleaningView state={s} />);
    expect(screen.getByText('56%')).toBeTruthy();
    expect(screen.getByText('1 GB')).toBeTruthy();
    expect(screen.queryAllByRole('button')).toHaveLength(0);
    expect(screen.getAllByRole('progressbar').length).toBeGreaterThanOrEqual(2);
  });
});

describe('ResultView', () => {
  it('GB thực lấy lại = trống sau − trước', () => {
    wrap(<ResultView state={atResult(SUMMARY, { before: 10 * GB, after: 12.5 * GB })} onOpenLog={async () => {}} onHome={() => {}} />);
    expect(screen.getByText('Đã lấy lại')).toBeTruthy();
    expect(screen.getByText('2,5 GB')).toBeTruthy();
    expect(screen.getByText('bỏ qua 4 file đang bị khóa')).toBeTruthy();
  });

  // Review Focus 5
  it('trống sau nhỏ hơn trước thì hiện 0, không số âm', () => {
    wrap(<ResultView state={atResult(SUMMARY, { before: 10 * GB, after: 9 * GB })} onOpenLog={async () => {}} onHome={() => {}} />);
    expect(screen.getByText('0 MB')).toBeTruthy();
    expect(screen.queryByText(/^\s*-\s*\d/)).toBeNull();
  });

  it('đang đo thì có vòng quay; đo hỏng thì nói rõ', () => {
    const measuring = atResult(SUMMARY);
    const { rerender } = wrap(<ResultView state={measuring} onOpenLog={async () => {}} onHome={() => {}} />);
    expect(screen.getByText('Đang đo dung lượng trống…')).toBeTruthy();
    rerender(
      <FluentProvider theme={webLightTheme}>
        <ResultView state={reducer(measuring, { type: 'FREE_AFTER_FAILED' })} onOpenLog={async () => {}} onHome={() => {}} />
      </FluentProvider>,
    );
    expect(screen.getByText('Không đo được dung lượng trống sau khi dọn.')).toBeTruthy();
  });

  it('chạy thử: hiện số lẽ ra lấy lại', () => {
    const dry: CleanSummary = { ...SUMMARY, dry_run: true, groups: [ok('user_temp', GB), ok('recycle_bin', GB / 2)] };
    wrap(<ResultView state={atResult(dry)} onOpenLog={async () => {}} onHome={() => {}} />);
    expect(screen.getByText('Chạy thử xong — lẽ ra lấy lại')).toBeTruthy();
    expect(screen.getByText('1,5 GB')).toBeTruthy();
  });

  it('Xem nhật ký có vòng quay khi đang mở; Về đầu', async () => {
    let finish: () => void = () => {};
    const onOpenLog = vi.fn(() => new Promise<void>((r) => (finish = r)));
    const onHome = vi.fn();
    wrap(<ResultView state={atResult(SUMMARY, { after: 11 * GB })} onOpenLog={onOpenLog} onHome={onHome} />);
    fireEvent.click(screen.getByRole('button', { name: 'Xem nhật ký' }));
    expect(await screen.findByText('Đang mở…')).toBeTruthy();
    finish();
    expect(await screen.findByRole('button', { name: 'Xem nhật ký' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Về đầu' }));
    expect(onHome).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `npx vitest run src/components/views2.test.tsx`
Expected: FAIL — `Failed to resolve import "./RestorePointView"`.

- [ ] **Step 3: Viết mã**

`src/components/RestorePointView.tsx`:

```tsx
import { Button, MessageBar, MessageBarActions, MessageBarBody, MessageBarTitle } from '@fluentui/react-components';
import { t } from '../i18n';
import type { State } from '../state/machine';
import { Busy } from './Busy';

export function RestorePointView({ state, onContinue, onAbort }: { state: State; onContinue: () => void; onAbort: () => void }) {
  if (state.restore?.status === 'failed') {
    return (
      <MessageBar intent="warning" className="wfu-canhbao" layout="multiline">
        <MessageBarBody>
          <MessageBarTitle>{t('restore.failedTitle')}</MessageBarTitle>
          {t('restore.failedBody', { message: state.restore.message })}
        </MessageBarBody>
        <MessageBarActions>
          <Button appearance="primary" onClick={onContinue}>
            {t('restore.continue')}
          </Button>
          <Button onClick={onAbort}>{t('restore.abort')}</Button>
        </MessageBarActions>
      </MessageBar>
    );
  }
  return (
    <section className="wfu-hero">
      <Busy size="medium" label={t('restore.creating')} />
    </section>
  );
}
```

`src/components/CleaningView.tsx`:

```tsx
import { ProgressBar, Title3 } from '@fluentui/react-components';
import { groupName } from '../catalog';
import { formatBytes } from '../format';
import { t } from '../i18n';
import type { CleanRow, State } from '../state/machine';
import { Busy } from './Busy';

function rowLabel(r: CleanRow): string {
  if (r.status === 'waiting') return t('cleaning.waiting');
  if (r.status === 'running') return r.percent !== null ? t('cleaning.percent', { percent: Math.round(r.percent) }) : t('cleaning.running');
  if (r.result?.error || !r.result?.report) return t('cleaning.failed');
  return formatBytes(r.result.report.bytes_freed);
}

export function CleaningView({ state }: { state: State }) {
  return (
    <section className="wfu-hero">
      <div className="wfu-header">
        <Title3>{t('cleaning.title')}</Title3>
        <Busy />
      </div>
      <ul className="wfu-groups">
        {state.cleaning.map((r) => (
          <li key={r.id} className="wfu-group">
            <span>{r.status === 'running' ? <Busy /> : r.status === 'done' ? (r.result?.error ? '!' : '✓') : ''}</span>
            <div className="wfu-group-main">
              <span className="wfu-group-title">{groupName(r.id)}</span>
              {r.status === 'running' && r.percent !== null && (
                <ProgressBar value={Math.min(1, r.percent / 100)} thickness="large" aria-label={groupName(r.id)} />
              )}
            </div>
            <span className="wfu-group-size">{rowLabel(r)}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
```

`src/components/ResultView.tsx`:

```tsx
import { useState } from 'react';
import { Body1, Button, Caption1 } from '@fluentui/react-components';
import { groupName } from '../catalog';
import { formatBytes } from '../format';
import { t } from '../i18n';
import type { State } from '../state/machine';
import { dryRunBytes, reclaimedBytes } from '../state/selectors';
import { Busy } from './Busy';

export function ResultView({ state, onOpenLog, onHome }: { state: State; onOpenLog: () => Promise<void>; onHome: () => void }) {
  const [opening, setOpening] = useState(false);
  const summary = state.summary;
  if (!summary) return null;
  const amount = summary.dry_run ? dryRunBytes(summary) : reclaimedBytes(state.freeBefore, state.freeAfter);
  const measuring = !summary.dry_run && amount === null && !state.freeAfterFailed;
  const openLog = async () => {
    setOpening(true);
    try {
      await onOpenLog();
    } finally {
      setOpening(false);
    }
  };
  return (
    <section className="wfu-hero">
      <Body1>{summary.dry_run ? t('result.titleDryRun') : t('result.title')}</Body1>
      {measuring ? (
        <Busy label={t('result.measuring')} />
      ) : amount === null ? (
        <Body1 className="wfu-muted">{t('result.measureFailed')}</Body1>
      ) : (
        <span className="wfu-hero-number">{formatBytes(amount)}</span>
      )}
      <ul className="wfu-groups">
        {summary.groups.map((g) => (
          <li key={g.id} className="wfu-group">
            <span>{g.error ? '!' : '✓'}</span>
            <div className="wfu-group-main">
              <span className="wfu-group-title">{groupName(g.id)}</span>
              {g.report && g.report.skipped_locked > 0 && <span className="wfu-muted">{t('result.skipped', { count: g.report.skipped_locked })}</span>}
              {g.report && g.report.errors.length > 0 && <span className="wfu-muted">{t('result.groupItemErrors', { count: g.report.errors.length })}</span>}
              {g.error && <span className="wfu-muted">{t('result.groupError', { message: g.error })}</span>}
            </div>
            <span className="wfu-group-size">
              {g.report ? t('result.groupOk', { size: formatBytes(g.report.bytes_freed), files: g.report.files_deleted }) : '—'}
            </span>
          </li>
        ))}
      </ul>
      <Caption1 className="wfu-muted">{t('result.logPath', { path: summary.log_path })}</Caption1>
      <div className="wfu-actions">
        <Button onClick={openLog} disabled={opening}>
          {opening ? <Busy label={t('result.opening')} /> : t('result.openLog')}
        </Button>
        <Button appearance="primary" onClick={onHome}>
          {t('result.home')}
        </Button>
      </div>
    </section>
  );
}
```

- [ ] **Step 4: Chạy test, thấy qua**

Run: `npx vitest run src/components/views2.test.tsx` rồi `npm run typecheck`
Expected: `Tests  8 passed (8)`; typecheck không lỗi.

- [ ] **Step 5: Commit**

```bash
rtk git add src/components/RestorePointView.tsx
rtk git add src/components/CleaningView.tsx
rtk git add src/components/ResultView.tsx
rtk git add src/components/views2.test.tsx
rtk git commit -m "feat(ui): màn Điểm khôi phục, Đang dọn (khoá nút, % DISM) và Kết quả (GB thực lấy lại)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 16: Lắp `App` — một cửa sổ, một máy trạng thái

**Files:**
- Create: `index.html`
- Create: `src/main.tsx`
- Create: `src/App.tsx`
- Test: `src/App.test.tsx`

**Interfaces:**
- Consumes:
  - Task 10: `Api` (`src/api/types.ts`), `t`.
  - Task 11: `initialState`, `reducer`.
  - Task 12: `pushNotice`, `dismissNotice`, `installGlobalHooks`, `Notice`; `NoticeBar`, `ErrorBoundary`; `src/styles.css`.
  - Task 13: `tauriApi`; `createStore`; `createDeps`, `Notify`, `friendly`, `loadInitial`, `startScan`, `cancelScan`, `toggle`, `requestClean`, `acceptConfirm`, `cancelConfirm`, `continueAfterRestoreFailure`, `abortAfterRestoreFailure`, `openLog`, `goHome`.
  - Task 14: `WelcomeView`, `ScanningView`, `PreviewView`, `ConfirmDialog` (props như Task 14).
  - Task 15: `RestorePointView`, `CleaningView`, `ResultView` (props như Task 15).
- Produces: `App({ api?: Api })` (mặc định `tauriApi`); `npm run build` ra `dist/` cho Tauri (Task 17).

- [ ] **Step 1: Viết test hỏng**

`src/App.test.tsx`:

```tsx
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import type { Api, GroupScan } from './api/types';
import { App } from './App';

const GB = 1024 ** 3;

function fakeApi(over: Partial<Api> = {}): Api {
  const free = [50 * GB, 50 * GB, 52 * GB, 52 * GB];
  const groups: GroupScan[] = [
    { id: 'user_temp', risk: 'safe', default_selected: true, result: { total_bytes: GB, file_count: 3, top_items: [], estimated: false, notices: [] }, error: null },
  ];
  return {
    appInfo: vi.fn(async () => ({ version: '0.1.0', dry_run: false, system_drive: 'C:\\' })),
    diskFree: vi.fn(async () => free.shift() ?? 0),
    scanAll: vi.fn(async (onGroup) => {
      groups.forEach(onGroup);
      return groups;
    }),
    cancelScan: vi.fn(async () => {}),
    prepareRestorePoint: vi.fn(async () => ({ status: 'not_needed' as const })),
    clean: vi.fn(async (ids: string[]) => ({
      groups: ids.map((id) => ({ id, report: { bytes_freed: GB, files_deleted: 3, skipped_locked: 0, errors: [], dry_run: false }, error: null })),
      log_path: 'C:\\x.log',
      dry_run: false,
      log_write_failed: false,
    })),
    openLogFolder: vi.fn(async () => {}),
    ...over,
  };
}

describe('App', () => {
  it('đi trọn luồng Chào → Quét → Xem trước → Dọn → Kết quả → Về đầu', async () => {
    const api = fakeApi();
    render(<App api={api} />);
    expect(await screen.findByText('Ổ C: còn trống 50 GB')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Quét' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Dọn 1 GB' }));
    expect(await screen.findByText('Đã lấy lại')).toBeTruthy();
    expect(await screen.findByText('2 GB')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Về đầu' }));
    expect(await screen.findByRole('button', { name: 'Quét' })).toBeTruthy();
  });

  it('chế độ chạy thử có nhãn trên đầu', async () => {
    render(<App api={fakeApi({ appInfo: vi.fn(async () => ({ version: '0.1.0', dry_run: true, system_drive: 'C:\\' })) })} />);
    expect(await screen.findByText('Chạy thử — không xóa gì')).toBeTruthy();
  });

  it('lỗi JavaScript ngoài luồng hiện băng trên màn', async () => {
    const err = vi.spyOn(console, 'error').mockImplementation(() => {});
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    render(<App api={fakeApi()} />);
    await screen.findByText('Ổ C: còn trống 50 GB');
    window.dispatchEvent(new ErrorEvent('error', { message: 'chart is not defined', filename: 'app.js', lineno: 7 }));
    expect(await screen.findByText('chart is not defined (app.js:7)')).toBeTruthy();
    err.mockRestore();
    warn.mockRestore();
  });
});
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `npx vitest run src/App.test.tsx`
Expected: FAIL — `Failed to resolve import "./App"`.

- [ ] **Step 3: Viết mã**

`index.html`:

```html
<!doctype html>
<html lang="vi">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>WinFreeUp</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

`src/main.tsx`:

```tsx
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import './styles.css';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
```

`src/App.tsx`:

```tsx
import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import { Badge, FluentProvider, Title2, webDarkTheme, webLightTheme } from '@fluentui/react-components';
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

const DARK = '(prefers-color-scheme: dark)';

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

export function App({ api = tauriApi }: { api?: Api }) {
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
        <NoticeBar notices={notices} onDismiss={(id) => setNotices((l) => dismissNotice(l, id))} />
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
      </main>
    </FluentProvider>
  );
}
```

- [ ] **Step 4: Chạy test và build, thấy qua**

Run: `npm test` rồi `npm run build`
Expected: `Test Files  10 passed (10)`, `Tests  75 passed (75)`; build in `✓ built` ra `dist/index.html` và `dist/assets/*.js`. Cảnh báo `Some chunks are larger than 500 kB` là bình thường (Fluent UI, ứng dụng chạy cục bộ) — không cần tách chunk.

- [ ] **Step 5: Commit**

```bash
rtk git add index.html
rtk git add src/main.tsx
rtk git add src/App.tsx
rtk git add src/App.test.tsx
rtk git commit -m "feat(ui): lắp App — theme sáng/tối theo Windows, băng thông báo, ranh giới lỗi

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 17: Vỏ Tauri — manifest Admin, kiểm WebView2, lệnh và sự kiện, danh sách nhóm

**Files:**
- Modify: `crates/winfreeup-core/src/cleaners/registry.rs` (thay dòng khung từ Task 1)
- Modify: `Cargo.toml` (thêm member `src-tauri`)
- Modify: `.gitignore` (thêm `src-tauri/gen/`)
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/app.manifest`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/capabilities/default.json`
- Create: `src-tauri/app-icon.png`, `src-tauri/icons/icon.ico` (sinh bằng lệnh ở Step 5)
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/commands.rs`
- Create: `src-tauri/src/webview_check.rs`
- Test: `crates/winfreeup-core/src/cleaners/registry.rs` (module `tests`); vỏ Tauri kiểm bằng build + soi manifest trong exe (không `cargo test` được vì `os error 740` — xem Global Constraints)

**Interfaces:**
- Consumes:
  - Task 4–7: `cleaners::user_temp::cleaner()`, `system_temp::cleaner()`, `win_caches::cleaner()`, `delivery_opt::cleaner()` (trả `FileCleaner`), `browser_cache::BrowserCache`, `wu_download::WuDownload`, `component_store::ComponentStore`, `recycle_bin::RecycleBin`, `windows_old::WindowsOld` (struct đơn vị, `impl Cleaner`).
  - Task 8: `engine::{scan_all, prepare_restore_point, run_clean, GroupScan, CleanSummary, CleanEvent, RestorePointStatus}` (chữ ký ở Task 8), `log::log_dir(local_appdata: &Path) -> PathBuf`.
  - Task 9: `Env::from_system() -> Result<Env>`, `sys_windows::disk_free(path: &Path) -> Result<u64>`.
  - Task 1: `Env` (Clone; trường `now`, `local_appdata`, `system_drive`), `CancelToken`, `CleanOptions`, `ScanResult`, `Cleaner`.
  - Task 16: `npm run build` ra `dist/`; giao diện gọi đúng tên lệnh/sự kiện liệt kê ở Task 13.
- Produces:
  - `cleaners::registry::{ALL_IDS: [&str; 9], all_cleaners() -> Vec<Box<dyn Cleaner>>}`.
  - Lệnh Tauri: `app_info() -> AppInfo { version, dry_run, system_drive }`, `disk_free(drive: Option<String>) -> Result<u64, String>`, `scan_all() -> Result<Vec<GroupScan>, String>`, `cancel_scan()`, `prepare_restore_point(ids: Vec<String>, dry_run: bool) -> Result<RestorePointStatus, String>`, `clean(ids: Vec<String>, dry_run: bool) -> Result<CleanSummary, String>`, `open_log_folder() -> Result<(), String>`. Lỗi `"busy"` khi đã có thao tác đang chạy.
  - Sự kiện: `scan-progress` (payload `GroupScan`), `clean-progress` (payload `CleanEvent`).
  - `target/release/WinFreeUp.exe` — một file, manifest `requireAdministrator`, cờ `--dry-run`.

- [ ] **Step 1: Viết test hỏng cho danh sách nhóm**

Thay dòng khung của `crates/winfreeup-core/src/cleaners/registry.rs` bằng phần test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RiskLevel::{self, *};

    #[test]
    fn registry_matches_spec_table_in_order() {
        let expected: [(&str, RiskLevel, bool); 9] = [
            ("user_temp", Safe, true),
            ("system_temp", Safe, true),
            ("browser_cache", Safe, true),
            ("win_caches", Safe, true),
            ("delivery_opt", Safe, true),
            ("wu_download", Caution, false),
            ("component_store", Caution, false),
            ("recycle_bin", Caution, false),
            ("windows_old", Risky, false),
        ];
        let all = all_cleaners();
        let got: Vec<_> = all.iter().map(|c| (c.id(), c.risk(), c.default_selected())).collect();
        assert_eq!(got, expected.to_vec());
        assert_eq!(all.iter().map(|c| c.id()).collect::<Vec<_>>(), ALL_IDS.to_vec());
    }
}
```

- [ ] **Step 2: Chạy test, thấy hỏng**

Run: `cargo test -p winfreeup-core registry`
Expected: FAIL biên dịch — `cannot find function all_cleaners`.

- [ ] **Step 3: Viết `registry.rs` (trên khối test)**

```rust
//! Danh sách đầy đủ 9 nhóm, đúng thứ tự bảng spec mục 3 và `src/catalog.ts` của giao diện.
use super::{
    browser_cache, component_store, delivery_opt, recycle_bin, system_temp, user_temp, win_caches, windows_old,
    wu_download,
};
use crate::types::Cleaner;

pub const ALL_IDS: [&str; 9] = [
    "user_temp",
    "system_temp",
    "browser_cache",
    "win_caches",
    "delivery_opt",
    "wu_download",
    "component_store",
    "recycle_bin",
    "windows_old",
];

pub fn all_cleaners() -> Vec<Box<dyn Cleaner>> {
    vec![
        Box::new(user_temp::cleaner()),
        Box::new(system_temp::cleaner()),
        Box::new(browser_cache::BrowserCache),
        Box::new(win_caches::cleaner()),
        Box::new(delivery_opt::cleaner()),
        Box::new(wu_download::WuDownload),
        Box::new(component_store::ComponentStore),
        Box::new(recycle_bin::RecycleBin),
        Box::new(windows_old::WindowsOld),
    ]
}
```

Run: `cargo test -p winfreeup-core registry`
Expected: `1 passed`.

- [ ] **Step 4: Tạo cấu hình vỏ Tauri**

`Cargo.toml` (gốc) — đổi dòng `members`:

```toml
members = ["crates/winfreeup-core", "src-tauri"]
```

`.gitignore` — thêm dòng:

```
src-tauri/gen/
```

`src-tauri/Cargo.toml`:

```toml
[package]
name = "winfreeup"
version = "0.1.0"
edition = "2021"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
winfreeup-core = { path = "../crates/winfreeup-core" }
windows-sys = { version = "0.61", features = ["Win32_Foundation", "Win32_UI_WindowsAndMessaging"] }
```

`src-tauri/build.rs`:

```rust
fn main() {
    // Manifest riêng: Admin ngay khi mở (UAC) + Common Controls v6 (tauri-build yêu cầu giữ khi thay manifest).
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)).expect("failed to run tauri-build");
}
```

`src-tauri/app.manifest`:

```xml
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}" />
    </application>
  </compatibility>
</assembly>
```

`src-tauri/tauri.conf.json` (`bundle.active: false` ⇒ không trình cài đặt; `mainBinaryName` ⇒ `WinFreeUp.exe`; bỏ sửa CSP `style-src` vì Fluent UI chèn thẻ `<style>` lúc chạy):

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "WinFreeUp",
  "mainBinaryName": "WinFreeUp",
  "version": "0.1.0",
  "identifier": "io.github.ali33.winfreeup",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "windows": [
      {
        "title": "WinFreeUp",
        "width": 880,
        "height": 700,
        "minWidth": 640,
        "minHeight": 520
      }
    ],
    "security": {
      "csp": "default-src 'self' ipc: http://ipc.localhost; style-src 'self' 'unsafe-inline'; img-src 'self' data:",
      "dangerousDisableAssetCspModification": ["style-src"]
    }
  },
  "bundle": {
    "active": false,
    "icon": ["icons/icon.ico"]
  }
}
```

`src-tauri/capabilities/default.json` (lệnh của ứng dụng tự được phép; `core:default` cho phép `listen` sự kiện):

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Quyền cho cửa sổ chính",
  "windows": ["main"],
  "permissions": ["core:default"]
}
```

- [ ] **Step 5: Sinh biểu tượng**

Chạy trong PowerShell ở gốc repo (vẽ ô vuông xanh chữ «W» 1024×1024, rồi dùng Tauri CLI sinh `.ico` vào thư mục tạm và chỉ chép `icon.ico`):

```powershell
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap 1024, 1024
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = 'AntiAlias'
$g.Clear([System.Drawing.Color]::FromArgb(0, 103, 192))
$font = New-Object System.Drawing.Font 'Segoe UI', 560, ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel)
$fmt = New-Object System.Drawing.StringFormat
$fmt.Alignment = 'Center'; $fmt.LineAlignment = 'Center'
$g.DrawString('W', $font, [System.Drawing.Brushes]::White, (New-Object System.Drawing.RectangleF 0, 0, 1024, 1024), $fmt)
$bmp.Save("$PWD\src-tauri\app-icon.png", [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
npx tauri icon src-tauri/app-icon.png -o "$env:TEMP\wfu-icons"
New-Item -ItemType Directory -Force src-tauri\icons | Out-Null
Copy-Item "$env:TEMP\wfu-icons\icon.ico" src-tauri\icons\icon.ico
```

Expected: có `src-tauri/app-icon.png` và `src-tauri/icons/icon.ico`.

- [ ] **Step 6: Viết mã vỏ**

`src-tauri/src/webview_check.rs`:

```rust
//! Hộp thoại gốc Windows (trước khi có WebView) — không bao giờ để màn trắng.
use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, IDYES, MB_ICONERROR, MB_OK, MB_YESNO};

const WEBVIEW2_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn show_missing(detail: &str) {
    let text = format!(
        "WinFreeUp cần Microsoft Edge WebView2 Runtime để hiển thị giao diện, nhưng máy này chưa có.\n\n\
         Bấm Yes để mở trang tải chính thức của Microsoft (miễn phí). Cài xong, mở lại WinFreeUp.\n\n\
         Chi tiết: {detail}"
    );
    let (text, caption) = (wide(&text), wide("WinFreeUp"));
    let answer = unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_YESNO | MB_ICONERROR) };
    if answer == IDYES {
        let _ = std::process::Command::new("explorer.exe").arg(WEBVIEW2_URL).spawn();
    }
}

pub fn show_fatal(detail: &str) {
    let text = wide(&format!("WinFreeUp không khởi động được.\n\nChi tiết: {detail}"));
    let caption = wide("WinFreeUp");
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_OK | MB_ICONERROR);
    }
}
```

`src-tauri/src/commands.rs`:

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use winfreeup_core::cleaners::registry::all_cleaners;
use winfreeup_core::engine::{self, CleanSummary, GroupScan, RestorePointStatus};
use winfreeup_core::log::log_dir;
use winfreeup_core::sys_windows::disk_free as core_disk_free;
use winfreeup_core::{CancelToken, CleanOptions, Cleaner, Env, ScanResult};

pub struct AppState {
    env: Env,
    cleaners: Vec<Box<dyn Cleaner>>,
    cancel: Mutex<CancelToken>,
    scans: Mutex<HashMap<String, ScanResult>>,
    busy: AtomicBool,
    dry_run_cli: bool,
}

pub type Shared = Arc<AppState>;

impl AppState {
    pub fn new(env: Env, dry_run_cli: bool) -> Shared {
        Arc::new(AppState {
            env,
            cleaners: all_cleaners(),
            cancel: Mutex::new(CancelToken::new()),
            scans: Mutex::new(HashMap::new()),
            busy: AtomicBool::new(false),
            dry_run_cli,
        })
    }

    /// "Bây giờ" của mỗi thao tác, để luật "cũ hơn 24 giờ" không dùng giờ lúc mở ứng dụng.
    fn fresh_env(&self) -> Env {
        let mut e = self.env.clone();
        e.now = SystemTime::now();
        e
    }
}

/// Chặn thao tác chồng nhau ở phía lõi; tự nhả khi thao tác kết thúc (kể cả lỗi).
struct BusyGuard(Shared);

impl BusyGuard {
    fn acquire(state: &Shared) -> Result<BusyGuard, String> {
        if state.busy.swap(true, Ordering::SeqCst) {
            Err("busy".into())
        } else {
            Ok(BusyGuard(state.clone()))
        }
    }
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.busy.store(false, Ordering::SeqCst);
    }
}

#[derive(Serialize)]
pub struct AppInfo {
    version: String,
    dry_run: bool,
    system_drive: String,
}

#[tauri::command]
pub fn app_info(app: AppHandle, state: State<'_, Shared>) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        dry_run: state.dry_run_cli,
        system_drive: state.env.system_drive.display().to_string(),
    }
}

#[tauri::command]
pub fn disk_free(state: State<'_, Shared>, drive: Option<String>) -> Result<u64, String> {
    let path = drive.map(PathBuf::from).unwrap_or_else(|| state.env.system_drive.clone());
    core_disk_free(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scan_all(app: AppHandle, state: State<'_, Shared>) -> Result<Vec<GroupScan>, String> {
    let st: Shared = state.inner().clone();
    let guard = BusyGuard::acquire(&st)?;
    let token = CancelToken::new();
    *st.cancel.lock().map_err(|e| e.to_string())? = token.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let env = st.fresh_env();
        let groups = engine::scan_all(&st.cleaners, &env, &token, &|g| {
            let _ = app.emit("scan-progress", g);
        });
        if let Ok(mut map) = st.scans.lock() {
            map.clear();
            for g in &groups {
                if let Some(r) = &g.result {
                    map.insert(g.id.clone(), r.clone());
                }
            }
        }
        groups
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cancel_scan(state: State<'_, Shared>) {
    if let Ok(token) = state.cancel.lock() {
        token.cancel();
    }
}

#[tauri::command]
pub async fn prepare_restore_point(state: State<'_, Shared>, ids: Vec<String>, dry_run: bool) -> Result<RestorePointStatus, String> {
    let st: Shared = state.inner().clone();
    let guard = BusyGuard::acquire(&st)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let env = st.fresh_env();
        engine::prepare_restore_point(&st.cleaners, &env, &ids, dry_run || st.dry_run_cli)
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clean(app: AppHandle, state: State<'_, Shared>, ids: Vec<String>, dry_run: bool) -> Result<CleanSummary, String> {
    let st: Shared = state.inner().clone();
    let guard = BusyGuard::acquire(&st)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let env = st.fresh_env();
        let scans = st.scans.lock().map(|m| m.clone()).unwrap_or_default();
        let opts = CleanOptions { dry_run: dry_run || st.dry_run_cli };
        engine::run_clean(&st.cleaners, &env, &ids, &scans, &opts, &log_dir(&env.local_appdata), &|ev| {
            let _ = app.emit("clean-progress", ev);
        })
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn open_log_folder(state: State<'_, Shared>) -> Result<(), String> {
    let dir = log_dir(&state.env.local_appdata);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::process::Command::new("explorer.exe").arg(&dir).spawn().map(|_| ()).map_err(|e| e.to_string())
}
```

`src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
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
    let result = tauri::Builder::default()
        .manage(commands::AppState::new(env, dry_run))
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::disk_free,
            commands::scan_all,
            commands::cancel_scan,
            commands::prepare_restore_point,
            commands::clean,
            commands::open_log_folder,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        webview_check::show_fatal(&e.to_string());
    }
}
```

- [ ] **Step 7: Build và soi kết quả**

Run (PowerShell, gốc repo):

```powershell
npm run build
cargo build -p winfreeup
npx tauri build --no-bundle
findstr /m requireAdministrator target\release\WinFreeUp.exe
(Get-Item target\release\WinFreeUp.exe).Length / 1MB
cargo test -p winfreeup-core
npm test
```

Expected: `cargo build` và `tauri build` không lỗi; `findstr` in `target\release\WinFreeUp.exe` (manifest Admin đã nhúng); kích thước khoảng 6 MB (đo thử khi viết kế hoạch: 6,01 MB — nhỏ hơn ước tính 8–12 MB của spec; ghi số đo thật vào commit message); `cargo test -p winfreeup-core` và `npm test` đều `0 failed`.

- [ ] **Step 8: Thử khởi động (tay, máy dev)**

Mở `target\release\WinFreeUp.exe` bằng Explorer. Expected: UAC hỏi ngay; đồng ý ⇒ cửa sổ «WinFreeUp — Dọn ổ đĩa» có giao diện Fluent, dòng «Ổ C: còn trống … GB». **Không bấm Dọn trên máy chính** — chỉ bấm Quét rồi đóng. Chạy thêm `target\release\WinFreeUp.exe --dry-run` ⇒ có nhãn «Chạy thử — không xóa gì».

- [ ] **Step 9: Commit**

```bash
rtk git add crates/winfreeup-core/src/cleaners/registry.rs
rtk git add Cargo.toml
rtk git add Cargo.lock
rtk git add .gitignore
rtk git add src-tauri/Cargo.toml
rtk git add src-tauri/build.rs
rtk git add src-tauri/app.manifest
rtk git add src-tauri/tauri.conf.json
rtk git add src-tauri/capabilities/default.json
rtk git add src-tauri/app-icon.png
rtk git add src-tauri/icons/icon.ico
rtk git add src-tauri/src/main.rs
rtk git add src-tauri/src/commands.rs
rtk git add src-tauri/src/webview_check.rs
rtk git commit -m "feat(shell): vỏ Tauri 2 — requireAdministrator, kiểm WebView2 bằng MessageBoxW, 7 lệnh, 2 sự kiện

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 18: CI GitHub Actions và danh sách thử tay

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `docs/thu-tay-v0.1.md`

**Interfaces:**
- Consumes: `npm ci`, `npm test`, `npm run build` (Task 10, 16); `cargo test -p winfreeup-core` (Task 1–9, 17); `npx tauri build --no-bundle` ⇒ `target/release/WinFreeUp.exe` (Task 17); cờ `--dry-run`.
- Produces: workflow `CI` chạy trên `windows-latest` cho mỗi push/PR, xuất artifact `WinFreeUp.exe`; bảng thử tay cho các nhóm cần Admin.

- [ ] **Step 1: Viết workflow**

`.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main, "feat/**"]
  pull_request:

jobs:
  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: npm ci
      - name: Vitest
        run: npm test
      - name: cargo test (lõi)
        run: cargo test -p winfreeup-core
      - name: clippy (lõi)
        run: cargo clippy -p winfreeup-core --all-targets -- -D warnings
      - name: tauri build (một file exe)
        run: npx tauri build --no-bundle
      - name: Kiểm manifest Admin
        shell: cmd
        run: findstr /m requireAdministrator target\release\WinFreeUp.exe
      - uses: actions/upload-artifact@v4
        with:
          name: WinFreeUp-exe
          path: target/release/WinFreeUp.exe
```

- [ ] **Step 2: Kiểm cục bộ đúng các lệnh CI sẽ chạy**

Run (PowerShell, gốc repo, cây sạch):

```powershell
npm ci
npm test
cargo test -p winfreeup-core
cargo clippy -p winfreeup-core --all-targets -- -D warnings
npx tauri build --no-bundle
cmd /c "findstr /m requireAdministrator target\release\WinFreeUp.exe"
```

Expected: tất cả thoát mã 0. Workflow chỉ chạy thật trên GitHub sau khi **người dùng đồng ý push** (CLAUDE.md mục 6) — chưa push trong task này.

- [ ] **Step 3: Viết danh sách thử tay**

`docs/thu-tay-v0.1.md`:

````markdown
# Thử tay WinFreeUp v0.1

Chỉ thử trong **Windows Sandbox** hoặc máy ảo — không bao giờ trên máy chính.
Bật Sandbox: `Turn Windows features on or off` → `Windows Sandbox`. Chép `WinFreeUp.exe` (artifact CI hoặc
`target\release\`) vào Sandbox. Sau mỗi lượt dọn, mở `Xem nhật ký` và đọc file log.

| # | Việc làm | Kỳ vọng |
|---|---|---|
| 1 | Nhấp đúp `WinFreeUp.exe` | UAC hỏi ngay; đồng ý ⇒ cửa sổ có «Ổ C: còn trống …» |
| 2 | Chạy `WinFreeUp.exe --dry-run`, Quét, chọn mọi nhóm, Dọn (gõ `xóa` ở hộp Rủi ro) | Nhãn «Chạy thử»; kết quả «lẽ ra lấy lại …»; không file nào mất; log toàn dòng `WOULD_DELETE` |
| 3 | Tạo file cũ: `fsutil file createnew %TEMP%\old.bin 50000000`, rồi PowerShell `(Get-Item $env:TEMP\old.bin).LastWriteTime = (Get-Date).AddDays(-2)`; Quét, Dọn | `File tạm của bạn` ≥ 47 MB; sau dọn file biến mất; GB thực lấy lại ≈ 0,05 |
| 4 | Mở Edge, Quét | Băng hổ phách «Edge đang mở — …»; Dọn vẫn chạy, file khóa được đếm «bỏ qua … file» |
| 5 | Chọn `wu_download`, Dọn; trong lúc đó chạy `sc query wuauserv` | Dịch vụ dừng trong lúc xóa; sau khi xong `STATE: RUNNING` (nếu trước đó đang chạy) |
| 6 | Chọn `component_store`, Dọn | Thanh % riêng chạy tới 100%; log có `DISM /StartComponentCleanup`; KHÔNG có `/ResetBase` |
| 7 | Chọn `recycle_bin` (xóa vài file vào Thùng rác trước) | Không tạo điểm khôi phục; Thùng rác trống |
| 8 | Sandbox không có System Restore: chọn `wu_download`, Dọn | Băng hổ phách «Không tạo được điểm khôi phục» + hai nút; «Dừng lại» ⇒ không xóa gì |
| 9 | Trên **máy ảo** có System Protection bật: lặp lại #8 | Điểm khôi phục «WinFreeUp» xuất hiện trong `rstrui`; lặp lại lần 2 trong 24 giờ ⇒ băng hổ phách (Windows giới hạn 1 điểm/24 giờ) |
| 10 | Giả `Windows.old`: `mkdir C:\Windows.old\sub`, `fsutil file createnew C:\Windows.old\sub\a.bin 1000000`, `mkdir C:\canary`, `echo x > C:\canary\giu.txt`, `mklink /J C:\Windows.old\link C:\canary`; chọn `windows_old`, gõ `XOA`, Dọn | `C:\Windows.old` biến mất; `C:\canary\giu.txt` **còn nguyên** và vẫn thuộc quyền sở hữu cũ (`icacls C:\canary`) |
| 11 | Thiếu WebView2: PowerShell `$env:WEBVIEW2_BROWSER_EXECUTABLE_FOLDER='C:\khong-co'; .\WinFreeUp.exe` | Hộp thoại tiếng Việt «cần Microsoft Edge WebView2 Runtime…»; Yes mở trang tải Microsoft; không có màn trắng |
| 12 | Bật `Settings → Accessibility → Visual effects → Animation effects: Off`, Quét | Vòng quay đổi thành nhịp mờ tỏ, vẫn chuyển động |
| 13 | Đổi Windows sang giao diện tối, mở lại | Giao diện tối theo |

Ghi kết quả từng dòng (đạt/không đạt + ảnh chụp) vào PR của bản phát hành.
````

- [ ] **Step 4: Commit**

```bash
rtk git add .github/workflows/ci.yml
rtk git add docs/thu-tay-v0.1.md
rtk git commit -m "ci: GitHub Actions windows-latest (Vitest, cargo test, clippy, tauri build) + danh sách thử tay

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Sau Đợt 5 — trước khi báo xong

- [ ] Trên `feat/v0.1-don-o-dia` đã gộp đủ: `cargo test -p winfreeup-core` (Expected `82 passed; 0 failed`), `npm test` (Expected `Tests  75 passed`), `npx tauri build --no-bundle` thành công.
- [ ] Quét tìm dấu hiệu làm dở trong file đã đổi: `rtk grep -n "TODO\|todo!\|unimplemented!\|\.only(\|\.skip(" crates src src-tauri` ⇒ không có kết quả.
- [ ] Rà toàn nhánh bằng một reviewer mới (`superpowers:requesting-code-review`).
- [ ] **Dừng và hỏi người dùng** trước khi push `feat/v0.1-don-o-dia` hoặc merge vào `main` (CLAUDE.md mục 6).
