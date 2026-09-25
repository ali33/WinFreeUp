# WinFreeUp — Dự án con 1: Dọn ổ đĩa (kèm lõi chung)

- Ngày: 2026-09-25
- Trạng thái: chờ duyệt
- Phạm vi: bản phát hành đầu tiên (v0.1)

## 1. Mục tiêu và bối cảnh

WinFreeUp là công cụ dọn máy Windows cho **người dùng phổ thông**, phát hành công khai.
Tầm nhìn gồm bốn mảng độc lập, mỗi mảng là một dự án con có spec riêng:

1. **Dọn ổ đĩa** — spec này
2. Khởi động và tiến trình nền — sau
3. Gỡ bloatware — sau
4. Quyền riêng tư (telemetry, quảng cáo, gợi ý) — sau

Dự án con 1 đồng thời dựng **lõi chung** (quét → xem trước → thực hiện → nhật ký) để ba mảng sau
cắm vào dưới dạng module.

### Tiêu chí thành công của v0.1

Người dùng tải một file `.exe`, mở ra (đồng ý UAC), bấm **Quét**, thấy tổng dung lượng lấy lại được
chia theo từng nhóm, bấm **Dọn**, thấy số GB thực sự đã lấy lại — và máy không hỏng gì.

### Ràng buộc

- Windows 10 (1903 trở lên) và Windows 11, 64-bit.
- Phát hành dạng **một file .exe chạy liền**, không trình cài đặt.
- Giao diện **tiếng Việt**; chuỗi hiển thị nằm trong file ngôn ngữ để thêm tiếng Anh sau.
- **An toàn trên hết**: không xóa gì trước khi người dùng thấy danh sách và bấm đồng ý.
- Không quảng cáo, không bản trả phí.

## 2. Kiến trúc

```
┌──────────────────────────────────────────────────────┐
│ Giao diện: React + TypeScript + Fluent UI React v9   │  hiển thị, hỏi người dùng
├──────────── lệnh Tauri (invoke) + sự kiện (emit) ────┤
│ Vỏ Tauri 2 (src-tauri)                               │  nối UI ↔ lõi, kiểm WebView2
├──────────────────────────────────────────────────────┤
│ Lõi Rust: crate `winfreeup-core`                     │  quét, dọn, luật an toàn, nhật ký
│   └─ module dọn (mỗi module một file)                │
└──────────────────────────────────────────────────────┘
```

### 2.1 Lõi Rust (`winfreeup-core`)

Không phụ thuộc Tauri, test độc lập bằng `cargo test`.

Khuôn chung cho mọi module:

```rust
pub enum RiskLevel { Safe, Caution, Risky }   // An toàn / Cân nhắc / Rủi ro

pub trait Cleaner: Send + Sync {
    fn id(&self) -> &'static str;               // khoá kỹ thuật, vd "user_temp"
    fn risk(&self) -> RiskLevel;
    fn default_selected(&self) -> bool;         // tích sẵn hay không
    fn allowed_roots(&self, env: &Env) -> Vec<PathBuf>;
    fn scan(&self, env: &Env, cancel: &CancelToken) -> Result<ScanResult>;
    fn clean(&self, env: &Env, scan: &ScanResult, opts: &CleanOptions,
             progress: &dyn Progress) -> Result<CleanReport>;
}
```

- `Env` chứa mọi đường dẫn hệ thống (`%TEMP%`, `%WINDIR%`, `%LOCALAPPDATA%`…). Chạy thật thì lấy từ
  hệ thống; test thì trỏ vào cây thư mục giả.
- `ScanResult`: tổng số byte, số file, và **20 mục lớn nhất** (đường dẫn + dung lượng) để xem trước.
- `CleanReport`: số byte đã xóa, số file bỏ qua vì bị khóa, danh sách lỗi (thông điệp nguyên văn).
- Tên tiếng Việt và dòng giải thích của mỗi nhóm nằm ở **giao diện** (file ngôn ngữ), tra theo `id`.
  Lõi không chứa chữ hiển thị.

### 2.2 Vỏ Tauri

- Lệnh: `scan_all`, `cancel_scan`, `clean(ids, dry_run)`, `disk_free(drive)`, `open_log_folder`.
- Sự kiện: `scan-progress` (nhóm nào xong đẩy lên ngay), `clean-progress` (kèm % cho DISM).
- Manifest đặt `requireAdministrator` — UAC hỏi ngay khi mở; mọi nhóm quét được từ đầu.
  Hệ quả chấp nhận: không kéo-thả được từ Explorer vào cửa sổ (v0.1 không cần).
- `main.rs` kiểm tra WebView2 **trước khi** tạo cửa sổ. Không có ⇒ `MessageBoxW` tiếng Việt kèm
  link tải WebView2 của Microsoft. Không bao giờ để màn trắng.

### 2.3 Giao diện

- React + TypeScript, Fluent UI React v9 (giao diện kiểu Windows 11).
- Một cửa sổ, một máy trạng thái (mục 4). Chuỗi hiển thị trong `src/i18n/vi.json`.

### 2.4 Nhật ký

`%LOCALAPPDATA%\WinFreeUp\logs\YYYY-MM-DD_HHmmss.log`: mỗi lượt dọn ghi thời điểm, nhóm, từng
đường dẫn đã xóa/bỏ qua, byte, lỗi. Nút "Xem nhật ký" mở thư mục này.

## 3. Các nhóm dọn của v0.1

**Nguyên tắc xóa:** đưa vào Thùng rác không giải phóng dung lượng, nên cache và file tạm (chương trình
tự tạo lại được) thì **xóa hẳn**. v0.1 **không đụng file của người dùng**.

| id | Nhóm | Mức | Tích sẵn | Admin | Ghi chú |
|---|---|---|---|---|---|
| `user_temp` | File tạm người dùng (`%TEMP%`) | An toàn | ✅ | – | Chỉ file cũ hơn 24 giờ; file bị khóa bỏ qua |
| `system_temp` | File tạm hệ thống (`%WINDIR%\Temp`) | An toàn | ✅ | ✔ | Như trên |
| `browser_cache` | Cache Chrome, Edge, Firefox, Cốc Cốc | An toàn | ✅ | – | Chỉ thư mục cache (mọi profile). Không đụng cookie, lịch sử, mật khẩu. Trình duyệt đang mở ⇒ cảnh báo, bỏ qua file khóa |
| `win_caches` | Cache ảnh thu nhỏ, báo cáo lỗi (WER), crash dump | An toàn | ✅ | ✔ | Windows tự tạo lại |
| `delivery_opt` | Cache Delivery Optimization | An toàn | ✅ | ✔ | |
| `wu_download` | File tải về của Windows Update (`SoftwareDistribution\Download`) | Cân nhắc | ☐ | ✔ | Dừng dịch vụ `wuauserv`, xóa, **luôn** bật lại kể cả khi lỗi |
| `component_store` | DISM `/Online /Cleanup-Image /StartComponentCleanup` | Cân nhắc | ☐ | ✔ | 5–15 phút, % lấy từ đầu ra DISM. **Cấm** `/ResetBase`. Dung lượng quét là ước tính (DISM `/AnalyzeComponentStore`) |
| `recycle_bin` | Thùng rác (mọi ổ) | Cân nhắc | ☐ | – | Qua `SHEmptyRecycleBinW`. Không lấy lại được |
| `windows_old` | Bản Windows cũ (`C:\Windows.old`) | Rủi ro | ☐ | ✔ | Mất khả năng quay về Windows trước; phải gõ `XOA` |

**Hoãn sang bản sau:** file lớn, file trùng, thư mục Downloads, rác lập trình
(`node_modules`, `bin/obj`, `target`).

**Điểm khôi phục:** trước khi dọn bất kỳ nhóm Cân nhắc/Rủi ro nào (trừ `recycle_bin`), tạo điểm
khôi phục hệ thống. Không tạo được (System Protection tắt, hoặc Windows giới hạn 1 điểm/24 giờ) ⇒
cảnh báo hổ phách, người dùng tự chọn dọn tiếp hay dừng.

## 4. Luồng màn hình

```
[Chào] → [Đang quét] → [Xem trước] → [Xác nhận]? → [Đang dọn] → [Kết quả]
```

| Trạng thái | Nội dung |
|---|---|
| Chào | Dung lượng trống ổ C, nút **Quét** |
| Đang quét | Quét song song; mỗi nhóm có vòng quay riêng, xong nhóm nào hiện nhóm đó. Nút **Hủy** |
| Xem trước | Mỗi nhóm: tên tiếng Việt, một dòng giải thích dễ hiểu, nhãn mức rủi ro có màu, dung lượng, ô tích. Bấm mở ⇒ 20 mục lớn nhất. Nút **Dọn X GB** luôn đúng số đang chọn |
| Xác nhận | Chỉ An toàn ⇒ bỏ qua bước này. Có Cân nhắc ⇒ hộp xác nhận liệt kê các nhóm đó. Có Rủi ro ⇒ phải gõ `XOA` |
| Đang dọn | Khóa mọi nút (chặn thao tác chồng). Tiến độ từng nhóm; DISM có thanh % riêng |
| Kết quả | **GB thực lấy lại** = dung lượng trống sau − trước (không phải số dự tính). Nhóm thành công / bỏ qua bao nhiêu file khóa / lỗi. Nút **Xem nhật ký**, **Về đầu** |

## 5. Xử lý lỗi

- **Lỗi chặn** (không đọc được ổ, lõi hỏng): băng đỏ, nguyên văn thông điệp lỗi.
- **Lỗi một phần** (một nhóm hỏng, file khóa, không tạo được điểm khôi phục): băng hổ phách, phần
  còn lại vẫn chạy và vẫn hiện.
- Móc `window.addEventListener('error')` và `'unhandledrejection'` ⇒ lỗi ngoài luồng cũng lên màn.
- Gộp trùng, tối đa 5 dòng. Chi tiết đầy đủ vẫn ghi `console` và file log.
- Mọi thao tác chờ lõi đều có vòng quay/chuyển động; tải lại một phần thì phủ mờ, không xoá trắng.
- `prefers-reduced-motion`: đổi hiệu ứng quay thành nhịp mờ tỏ, không bỏ hẳn.

## 6. Luật an toàn trong lõi

1. **Danh sách thư mục gốc được phép** cho mỗi module. Trước khi xóa, chuẩn hóa đường dẫn
   (`canonicalize`) và kiểm tra nằm trong một gốc được phép. Nằm ngoài ⇒ từ chối, ghi lỗi.
2. **Không đi theo reparse point** (junction, symlink, mount point). Gặp thì chỉ gỡ bản thân liên
   kết, không duyệt vào trong.
3. **Chế độ chạy thử** (`dry_run`, cờ dòng lệnh `--dry-run`): chạy đủ các bước, không xóa gì, ghi
   nhật ký những gì lẽ ra sẽ xóa.
4. File bị khóa/không có quyền ⇒ bỏ qua và đếm, không bao giờ làm hỏng cả nhóm.
5. Dịch vụ Windows đã dừng thì **luôn** được bật lại (kiểu RAII/`Drop`), kể cả khi lỗi giữa chừng.

## 7. Kiểm thử

- **Lõi Rust (`cargo test`)**: mỗi module test trên cây thư mục giả trong thư mục tạm (file cũ/mới,
  file đang mở khóa, junction trỏ ra ngoài). Test riêng cho luật an toàn: xóa ngoài gốc bị chặn,
  junction không bị duyệt, `dry_run` không xóa gì. Test không bao giờ đụng hệ thống thật.
- **Giao diện (Vitest)**: máy trạng thái, tính tổng GB đang chọn, bước xác nhận theo mức rủi ro.
- **Thử tay**: các nhóm cần Admin (`wu_download`, `component_store`, `windows_old`, điểm khôi phục)
  chạy trong **Windows Sandbox** hoặc máy ảo, không thử trên máy chính.
- **CI**: GitHub Actions `windows-latest` — `cargo test`, Vitest, `tauri build`.

## 8. Đóng gói và phát hành

- `tauri build` không bundler ⇒ một file `WinFreeUp.exe` (ước tính 8–12 MB).
- Manifest `requireAdministrator`; kiểm WebView2 lúc khởi động (mục 2.2).
- **Ký số** file .exe trước khi phát hành công khai để tránh SmartScreen/antivirus chặn — thuộc bước
  phát hành, ngoài phạm vi code v0.1.

## 9. Ngoài phạm vi v0.1

- Ba mảng còn lại (khởi động/tiến trình, bloatware, quyền riêng tư).
- Tìm file lớn, file trùng, Downloads, rác lập trình.
- Màn Lịch sử (nhật ký đã có sẵn để làm sau).
- Tiếng Anh, tự cập nhật, trình cài đặt, lên lịch dọn tự động.
