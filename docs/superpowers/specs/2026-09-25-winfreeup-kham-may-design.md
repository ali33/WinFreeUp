# WinFreeUp — Dự án con 2: Khám máy (chẩn đoán)

- Ngày: 2026-09-25
- Trạng thái: chờ duyệt
- Làm sau: dự án con 1 (`2026-09-25-winfreeup-don-o-dia-design.md`), dùng chung khung và lõi

## 1. Mục tiêu và bối cảnh

**Tình huống:** máy dùng lâu ngày trở nên chậm, ổ đĩa đầy. Người dùng mở WinFreeUp để **đánh giá**:
thư mục nào nặng (dạng cây), cái gì ngốn RAM, cái gì gây chậm/giật — rồi xử lý ngay tại chỗ bằng các
hành động an toàn.

### Tiêu chí thành công

Trong **dưới một phút** sau khi mở tab, người dùng biết: thư mục nào chiếm chỗ nhiều nhất, app nào ngốn
RAM nhiều nhất, và danh sách vấn đề của máy kèm gợi ý xử lý.

### Ràng buộc (kế thừa v0.1)

- Một file `.exe`, luôn chạy quyền Admin (`requireAdministrator`), Windows 10 1903+ / 11, 64-bit.
- Tauri 2 + React + TypeScript + Fluent UI React v9; lõi Rust `winfreeup-core`.
- Tiếng Việt; chuỗi hiển thị trong `src/i18n/vi.json`.
- Cùng luật lỗi (băng đỏ/hổ phách, móc lỗi toàn cục), cùng nhật ký `%ProgramData%\WinFreeUp\<SID>\logs\` (thư mục DACL chỉ Admin/SYSTEM).
- **Không cài driver nhân** dưới bất kỳ hình thức nào.

## 2. Bố cục

Tab mới **"Khám máy"** cạnh tab "Dọn dẹp", gồm ba mục con:

1. **Tổng quan** — khám nhanh (mục 4). Mặc định mở mục này.
2. **Ổ đĩa** — cây thư mục (mục 3).
3. **Bộ nhớ & Hiệu năng** — theo dõi trực tiếp (mục 5).

## 3. Ổ đĩa: cây thư mục

### 3.1 Quét (lõi Rust, module `disk_tree`)

```rust
pub trait DiskScanner {
    fn scan(&self, volume: &Volume, cancel: &CancelToken,
            progress: &dyn Progress) -> Result<DiskTree>;
}
// Cài đặt: MftScanner (đọc MFT, NTFS cục bộ) và WalkScanner (FindFirstFileExW song song)
```

- **Bộ chọn:** ổ NTFS cục bộ ⇒ `MftScanner`; ổ khác (FAT32, exFAT, USB, mạng) hoặc MFT lỗi ⇒
  `WalkScanner`, kèm băng hổ phách "Đang dùng chế độ quét chậm vì <lý do nguyên văn>".
- **Dung lượng** = dung lượng thực chiếm trên đĩa (allocated size), không phải kích thước logic.
  **Hard link đếm một lần.** Không đi theo reparse point.
- Thư mục không đọc được vẫn là một nút, gắn cờ `unreadable`, hiển thị "Không đọc được".
- `DiskTree` giữ trong bộ nhớ lõi dạng mảng nút gọn (tên, byte, số file, ngày sửa, cha, con).
  Giao diện **không nhận cả cây**:
  - `disk_volumes()` → danh sách ổ (ký tự, nhãn, hệ tệp, tổng, trống).
  - `disk_scan(volume)` + sự kiện `disk-scan-progress` (số file, byte, đường dẫn đang quét); `disk_scan_cancel()`.
  - `tree_children(node_id)` → tối đa **200 con lớn nhất** + một dòng tổng "(+N mục nhỏ khác, X GB)".

### 3.2 Giao diện

- Chọn ổ → Quét (thanh tiến độ, số file, nút Hủy).
- Cây mở/đóng; mỗi dòng: tên · dung lượng · thanh % so với cha · ngày sửa gần nhất. Mặc định sắp theo dung lượng.
- Đường dẫn được bảo vệ hiện 🔒: vẫn mở xuống xem được, không xóa được.

### 3.3 Hành động

- **Mở trong Explorer**, **Sao chép đường dẫn**.
- **Xóa vào Thùng rác** (`IFileOperation` với `FOFX_RECYCLEONDELETE`): hộp xác nhận ghi tên, dung lượng,
  số file. Xong thì trừ dung lượng khỏi cây ngay (không quét lại), ghi nhật ký.
- **Danh sách được bảo vệ** (chặn xóa, so sau khi `canonicalize`): thư mục gốc mọi ổ, `%WINDIR%`,
  `Program Files`, `Program Files (x86)`, `ProgramData`, `%USERPROFILE%` và `C:\Users`,
  `pagefile.sys`, `hiberfil.sys`, `swapfile.sys`, `System Volume Information`, `$Recycle.Bin`.
  Chặn cả chính các đường dẫn đó; con bên trong `%USERPROFILE%` (vd `Downloads\abc`) vẫn xóa được,
  con bên trong các thư mục hệ thống còn lại thì không.
- `hiberfil.sys` có dòng gợi ý "Tắt ngủ đông để lấy lại X GB" kèm hướng dẫn — chỉ là chữ, không tự tắt.

## 4. Tổng quan: khám nhanh

Chạy trong khoảng 5 giây, ra danh sách vấn đề. Mỗi vấn đề: mức (🔴 Nghiêm trọng / 🟠 Nên xử lý /
🟢 Ổn), một câu giải thích dễ hiểu, một nút dẫn tới chỗ xử lý.

Quy tắc là **hàm thuần** `evaluate(metrics: &HealthMetrics) -> Vec<Finding>` — không gọi hệ thống,
nên test được từng ngưỡng.

| id | Kiểm tra | Ngưỡng | Nút |
|---|---|---|---|
| `disk_full` | Ổ hệ thống sắp đầy | 🟠 trống < 15% · 🔴 trống < 5% | Dọn dẹp / Ổ đĩa |
| `disk_health` | Sức khỏe ổ cứng | 🔴 khi `MSFT_PhysicalDisk.HealthStatus` ≠ Healthy hoặc SMART báo sắp hỏng | Hướng dẫn sao lưu |
| `system_hdd` | Ổ hệ thống là HDD | 🟠 (`MediaType` = HDD) | Gợi ý chuyển SSD |
| `ram_pressure` | RAM thiếu | 🟠 dùng > 85% · 🔴 commit charge > 90% giới hạn | Bộ nhớ & Hiệu năng |
| `startup_apps` | App khởi động cùng máy | 🟠 > 8 app đang bật | Danh sách khởi động |
| `uptime` | Lâu chưa khởi động lại | 🟠 > 7 ngày; giải thích Fast Startup làm "Shut down" không tính là khởi động lại | Gợi ý |
| `power_saver` | Tiết kiệm pin khi đang cắm sạc | 🟠 | Mở cài đặt Nguồn |
| `cpu_throttle` | CPU bị hạ xung | 🟠 theo mục 5.4 (lấy mẫu 10 giây lúc khám) | Bộ nhớ & Hiệu năng |
| `cpu_hot` | CPU quá nóng | 🟠 > 90 °C, chỉ khi đọc được nhiệt độ (mục 5.5) | Gợi ý vệ sinh / tản nhiệt |

Nguồn số liệu nào không đọc được ⇒ dòng đó hiện "Không đo được: <lý do>", không phải 🟢.

**Danh sách khởi động:** nguồn `HKCU/HKLM\...\Run`, thư mục Startup của người dùng và chung.
Công tắc bật/tắt ghi vào `...\Explorer\StartupApproved\{Run,Run32,StartupFolder}` — đúng cơ chế
Task Manager, bật lại được. Mỗi lần đổi ghi nhật ký.

## 5. Bộ nhớ & Hiệu năng: theo dõi trực tiếp

Chỉ lấy mẫu khi mục này đang hiển thị; rời mục ⇒ dừng. Lấy mẫu **mỗi 1 giây**, giữ **5 phút** gần nhất.
Chi phí của chính WinFreeUp mục tiêu < 2% CPU; vượt ⇒ giãn chu kỳ còn 2 giây.

### 5.1 Biểu đồ

Bốn biểu đồ: **CPU**, **RAM**, **Hoạt động đĩa** (% active time), **Mạng** (tải lên / tải xuống).
Cơn giật (5.3) tô dải đỏ trên cả bốn.

### 5.2 Bảng ứng dụng

- Gộp tiến trình theo app (theo đường dẫn exe; tên thân thiện lấy từ `FileDescription`, kèm biểu tượng).
  Bấm dòng ⇒ mở ra từng tiến trình.
- Cột: Tên · RAM (private working set) · CPU % · Đĩa (MB/s) · Mạng (KB/s). Sắp được theo mọi cột,
  mặc định RAM.
- Hành động: **Kết thúc app** (xác nhận), **Mở vị trí file**.
- **Chặn kết thúc** tiến trình thiết yếu: `System`, `Registry`, `smss`, `csrss`, `wininit`, `winlogon`,
  `services`, `lsass`, `dwm`, `svchost`, `fontdrvhost`, và chính WinFreeUp. Chặn theo tên **và** đường
  dẫn nằm trong `%WINDIR%\System32` (không bị lừa bởi exe trùng tên ở chỗ khác — trường hợp đó vẫn cho kết thúc).

### 5.3 Cơn giật vừa ghi nhận

- Định nghĩa: CPU **hoặc** Hoạt động đĩa > 90% trong **≥ 3 mẫu liên tiếp** (≥ 3 giây).
- Hai cơn cách nhau < 2 giây ⇒ gộp làm một.
- Mỗi cơn ghi: giờ bắt đầu, thời lượng, chỉ số nào vượt, **3 app ngốn nhất** (trung bình trong cơn,
  theo chỉ số đã vượt), và cờ "trùng lúc hạ xung" nếu có.
- Bộ phát hiện là hàm thuần trên chuỗi mẫu ⇒ test bằng chuỗi dựng sẵn.

### 5.4 Hạ xung (không cần driver)

- Bộ đếm PDH `\Processor Information(_Total)\% Processor Performance`.
- **CPU tải > 80% và hiệu năng < 70% liên tục ≥ 10 giây** ⇒ 🟠 "CPU đang bị hạ xung (thường do
  nóng hoặc chế độ nguồn)".
- Bộ đếm không có ⇒ ẩn chỉ báo, ghi "Không đo được".

### 5.5 Nhiệt độ

- Đọc `MSAcpi_ThermalZoneTemperature` (WMI `root\wmi`). Đổi đơn vị từ phần mười Kelvin.
- Không có lớp đó, hoặc giá trị ngoài khoảng 20–110 °C, hoặc đứng yên suốt 60 giây ⇒ coi là không đọc
  được: hiện "Máy này không cho đọc nhiệt độ". **Không** dùng driver (WinRing0 và tương tự nằm trong
  danh sách driver có lỗ hổng bị Microsoft chặn).

### 5.6 Mạng theo app

- Tổng mạng: bộ đếm giao diện mạng (`GetIfTable2`).
- Theo app: phiên ETW thời gian thực, provider `Microsoft-Windows-Kernel-Network` (cần Admin — đã có).
  Phiên đặt tên riêng `WinFreeUp-Net`; khởi động thấy phiên cùng tên còn sót ⇒ dừng rồi tạo lại;
  đóng app ⇒ dừng phiên.
- ETW không khởi động được ⇒ cột Mạng hiện "–", băng hổ phách nêu lý do; biểu đồ tổng vẫn chạy.

## 6. Xử lý lỗi

- Luật chung như v0.1: lỗi chặn ⇒ băng đỏ nguyên văn; lỗi một phần ⇒ băng hổ phách; móc `error` và
  `unhandledrejection`; gộp trùng, tối đa 5 dòng; log đầy đủ.
- **Mỗi nguồn số liệu hỏng độc lập**: MFT, SMART/HealthStatus, nhiệt độ, ETW, PDH, WMI — cái nào hỏng
  thì chỗ đó ghi "Không đo được: <lý do>", phần còn lại vẫn chạy. Không bao giờ để trống cả tab.
- Mọi thao tác chờ (quét ổ, khám nhanh, mở tầng cây, xóa, kết thúc app) có chỉ báo động; tải lại một
  phần thì phủ mờ, không xoá trắng; chặn thao tác chồng (đang xóa thì khóa nút xóa khác).

## 7. Kiểm thử

- `WalkScanner`: cây thư mục giả — hard link, junction trỏ ra ngoài, thư mục bị từ chối quyền, file sparse.
- `MftScanner`: **đối chiếu** với `WalkScanner` trên một VHD nhỏ tạo trong test (cần Admin; đánh dấu
  `#[ignore]` khi không có quyền, CI chạy với quyền) — tổng byte và số file phải khớp.
- `tree_children`: cắt 200 + dòng tổng đúng.
- Danh sách bảo vệ đường dẫn và tiến trình thiết yếu: test riêng, gồm exe trùng tên ngoài System32.
- `evaluate()` Tổng quan: từng ngưỡng, biên (đúng 15%, 5%, 85%…), nguồn thiếu ⇒ "Không đo được".
- Bộ phát hiện cơn giật và hạ xung: chuỗi mẫu — đúng 3 giây, 2 giây, hai cơn cách 1 giây (gộp) và 3 giây (tách).
- Thử tay trên máy thật/VM: SMART, nhiệt độ, ETW, bật/tắt app khởi động.

## 8. Thứ tự làm (mỗi bước tự dùng được)

1. Ổ đĩa — cây thư mục với `WalkScanner` + hành động.
2. Bộ nhớ & Hiệu năng — biểu đồ CPU/RAM/đĩa, bảng app, cơn giật, hạ xung.
3. Tổng quan — khám nhanh + danh sách khởi động.
4. `MftScanner` — đường nhanh.
5. Mạng (ETW + biểu đồ thứ tư) và nhiệt độ.

## 9. Ngoài phạm vi

- Ghi số liệu nền dài hạn khi app không mở.
- Treemap, lọc theo loại file, top file lớn toàn ổ.
- Tự tắt ngủ đông, tắt dịch vụ Windows (thuộc dự án con Khởi động & tiến trình).
- Driver nhân, đọc cảm biến ngoài ACPI.
