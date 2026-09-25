# Thử tay WinFreeUp v0.1

Chỉ thử trong **Windows Sandbox** hoặc máy ảo — không bao giờ trên máy chính.
Bật Sandbox: `Turn Windows features on or off` → `Windows Sandbox`. Chép `WinFreeUp.exe` (artifact CI hoặc
`target\release\`) vào Sandbox. Sau mỗi lượt dọn, mở `Xem nhật ký` và đọc file log.

## Bảng thử tay cơ bản

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

## Bổ sung từ rà bảo mật

Các mục dưới đây phát sinh khi rà bảo mật riêng, đánh số **SB-1 … SB-17** (không trùng bảng trên).
Mục có gắn nhãn **CHẶN PHÁT HÀNH nếu chưa chạy** thì bắt buộc phải thử và đạt trước khi phát hành bản
chính thức; các mục còn lại ghi nhận hành vi/theo dõi thêm, không chặn phát hành.

- [ ] **SB-1 — Windows.old do user tự tạo (không phải sau nâng cấp).** *(CHẶN PHÁT HÀNH nếu chưa chạy)*
  Bước làm: Tạo `C:\Windows.old` bằng tài khoản người dùng thường (`mkdir C:\Windows.old`,
  `fsutil file createnew C:\Windows.old\moi.bin 1000000`) — chủ sở hữu là user hiện tại, không phải
  `TrustedInstaller`/`Administrators` như Windows.old thật. Chọn `windows_old`, gõ `XOA`, bấm Dọn.
  Mong đợi: bị từ chối, thông báo nêu chủ sở hữu hiện tại dạng SID `S-1-5-21-…` không phải chủ hợp lệ;
  `icacls C:\Windows.old` chụp trước và sau thao tác giống hệt nhau (không ACL nào bị đổi).

- [ ] **SB-2 — Windows.old là junction thẳng vào C:\Windows.** *(CHẶN PHÁT HÀNH nếu chưa chạy)*
  Bước làm: Xoá/di chuyển `C:\Windows.old` nếu có, rồi `mklink /J C:\Windows.old C:\Windows`. Chọn
  `windows_old`, gõ `XOA`, Dọn.
  Mong đợi: thông báo/log chứa cụm "root is a reparse point"; `icacls C:\Windows` và
  `icacls C:\Windows\System32` (chụp trước và sau) giống hệt nhau.

- [ ] **SB-3 — Windows.old thật với bẫy lồng nhau (junction vào System32, hard link ra ngoài cây, tráo thư mục giữa chừng).** *(CHẶN PHÁT HÀNH nếu chưa chạy — bổ sung cho #10 ở bảng trên, giữ cả hai kịch bản)*
  Bước làm: Dựng cây giả gần giống Windows.old thật sau nâng cấp tại chỗ: `mkdir C:\Windows.old\Users\tester`
  (thư mục con này chủ sở hữu = user hiện tại), trong `C:\Windows.old\Windows` tạo
  `mklink /J C:\Windows.old\Windows\System32 C:\Windows\System32` (junction trỏ vào System32 thật), tạo file
  ngoài cây `C:\ngoai-cay\giu.bin` rồi hard link vào trong: `mklink /H C:\Windows.old\out.bin C:\ngoai-cay\giu.bin`.
  Trong lúc app đang chạy `take_ownership` trên các thư mục con, tráo một thư mục con đã liệt kê nhưng chưa xử
  lý thành junction trỏ ra ngoài cây (script chạy song song để mô phỏng race condition). Chọn `windows_old`,
  gõ `XOA`, Dọn.
  Mong đợi: chủ sở hữu và DACL của `C:\Windows\System32` không đổi; `C:\ngoai-cay\giu.bin` không đổi nội dung
  hay quyền; nhật ký có dòng nêu rõ mục bị bỏ qua vì nằm "outside the tree" (hoặc "skipped").

- [ ] **SB-4 — HKCU\Environment TEMP trỏ vào System32.** *(CHẶN PHÁT HÀNH nếu chưa chạy)*
  Bước làm: `reg add "HKCU\Environment" /v TEMP /t REG_EXPAND_SZ /d C:\Windows\System32 /f` (và tương tự
  `TMP`); đăng xuất rồi đăng nhập lại để giá trị nạp vào phiên; chọn nhóm `user_temp`, Dọn.
  Mong đợi: ứng dụng vẫn dọn đúng đường dẫn cố định `%USERPROFILE%\AppData\Local\Temp` (không tin biến môi
  trường TEMP đã bị người dùng đổi hướng); `C:\Windows\System32` không mất file nào.

- [ ] **SB-5 — AppData\Local\Temp là junction trỏ vào System32.** *(CHẶN PHÁT HÀNH nếu chưa chạy)*
  Bước làm: Đổi tên `%USERPROFILE%\AppData\Local\Temp` thật sang nơi khác để giữ lại, rồi
  `mklink /J "%USERPROFILE%\AppData\Local\Temp" C:\Windows\System32`; chọn `user_temp`, Dọn.
  Mong đợi: băng hổ phách hiện dòng "bỏ qua … liên kết" (ứng dụng không đi theo reparse point); không file
  nào trong `C:\Windows\System32` bị xoá.

- [ ] **SB-6 — PSModulePath giả trong HKCU\Environment.** *(không chặn phát hành — ghi nhận, theo dõi thêm)*
  Bước làm: `reg add "HKCU\Environment" /v PSModulePath /t REG_EXPAND_SZ /d "C:\gia\Modules" /f`, tạo module
  giả tên `Microsoft.PowerShell.Management` tại `C:\gia\Modules\Microsoft.PowerShell.Management\` (kèm file
  đánh dấu để biết có bị nạp hay không), đồng thời tạo bản giả thứ hai tại
  `Documents\WindowsPowerShell\Modules\Microsoft.PowerShell.Management\`. Chạy các thao tác dừng/bật dịch vụ
  (`wu_download`) và tạo điểm khôi phục hệ thống (nhóm Rủi ro).
  Mong đợi: module giả không được nạp — file đánh dấu bên trong module giả không xuất hiện dấu vết đã chạy;
  ứng dụng dùng đường dẫn PowerShell module hệ thống mặc định, không bị PSModulePath của user chi phối.

- [ ] **SB-7 — DISM StartComponentCleanup, huỷ giữa chừng.** *(CHẶN PHÁT HÀNH nếu chưa chạy)*
  Bước làm: Chọn `component_store`, Dọn. LƯU Ý: màn "Đang dọn" KHÔNG có nút Hủy vì lõi DISM không hủy được
  ở bước clean; thử hủy ở bước quét (`AnalyzeComponentStore`) bằng nút Hủy trên màn "Đang quét".
  Mong đợi: `Dism.exe` và `DismHost.exe` cùng thoát trong 1–2 giây, giao diện không treo; `DismHost.exe`
  chạy từ `C:\Windows\Temp\WinFreeUp-…` (không có DismHost nào chạy từ `%TEMP%` của user); thư mục ScratchDir
  bị xoá sau khi kết thúc; sau đó chạy `DISM /Online /Cleanup-Image /ScanHealth` không báo kho thành phần hỏng.

- [ ] **SB-8 — Thao tác vào ScratchDir trong lúc DISM chạy.** *(không chặn phát hành)*
  Bước làm: Trong lúc SB-7 đang chạy DISM, mở terminal khác thử liệt kê, ghi file, hoặc đổi tên vào thư mục
  ScratchDir mà ứng dụng đang dùng.
  Mong đợi: các thao tác đó bị từ chối (quyền/khoá file), không phá được tiến trình dọn.

- [ ] **SB-9 — Đặc quyền tiến trình sau khi dọn xong.** *(CHẶN PHÁT HÀNH nếu chưa chạy)*
  Bước làm: Sau khi hoàn tất dọn `windows_old` (đã bật `SeBackupPrivilege`/`SeRestorePrivilege`/
  `SeTakeOwnershipPrivilege` để cấp quyền), mở Process Explorer, xem tab Security/Privileges của tiến trình
  WinFreeUp.exe.
  Mong đợi: ba đặc quyền trên trở về trạng thái `Disabled` (không còn bật thường trực sau khi thao tác cần
  chúng đã xong).

- [ ] **SB-10 — `cargo test --workspace` với quyền Admin.** *(CHẶN PHÁT HÀNH nếu chưa chạy)*
  Bước làm: Mở terminal **Run as administrator**, chạy `cargo test --workspace`.
  Mong đợi: các test `take_ownership` khi chạy với quyền Admin kiểm thêm chủ sở hữu mong đợi `O:BA`
  (built-in Administrators); toàn bộ test chỉ chạm thư mục tạm (`tempfile`), không đụng hệ thống thật; test
  pass.

- [ ] **SB-11 — wu_download: dịch vụ, môi trường tiến trình con, điểm khôi phục.** *(CHẶN PHÁT HÀNH nếu chưa chạy)*
  Bước làm: Chọn `wu_download`, Dọn; theo dõi `sc query wuauserv` trong lúc chạy và cả khi ép lỗi giữa chừng
  (ví dụ khoá file trong SoftwareDistribution để gây lỗi xoá); riêng biệt, kiểm `Stop-Service`/`Start-Service`/
  `Checkpoint-Computer` chạy được khi môi trường tiến trình con bị giới hạn tối thiểu; và thử trên máy đã tắt
  System Protection hoặc đã có điểm khôi phục tạo trong 24 giờ gần nhất.
  Mong đợi: dịch vụ `wuauserv` luôn được bật lại kể cả khi xoá lỗi giữa chừng; `Stop-Service`/`Start-Service`/
  `Checkpoint-Computer` không lỗi vì thiếu biến môi trường; khi System Protection tắt hoặc điểm khôi phục đã
  tạo trong 24 giờ ⇒ băng hổ phách hiện ra, người dùng tự chọn Tiếp tục hay Dừng lại.

- [ ] **SB-12 — Thư mục có ACE OWNER RIGHTS hạn chế.** *(không chặn phát hành)*
  Bước làm: Trên một thư mục thử, đặt ACE `OWNER RIGHTS` hạn chế (không cấp quyền) và DACL không cấp gì cho
  nhóm Administrators (qua `icacls` hoặc GUI Advanced Security). Chọn nhóm dọn tương ứng, Dọn.
  Mong đợi: bước cấp quyền (`take_ownership`) vẫn thành công, không bị OWNER RIGHTS chặn.

- [ ] **SB-13 — Đường dẫn dài hơn 260 ký tự trong Windows.old.** *(không chặn phát hành)*
  Bước làm: Trong `C:\Windows.old`, tạo cây thư mục lồng nhiều cấp để đường dẫn một file vượt quá 260 ký tự,
  đặt file trong đó. Chọn `windows_old`, gõ `XOA`, Dọn.
  Mong đợi: đổi chủ sở hữu, cấp quyền, và xoá vẫn chạy đúng cho đường dẫn dài.

- [ ] **SB-14 — Thiếu WebView2, UAC phải hỏi trước.** *(không chặn phát hành — mở rộng góc bảo mật cho #11 ở bảng trên)*
  Bước làm: PowerShell `$env:WEBVIEW2_BROWSER_EXECUTABLE_FOLDER='C:\khong-co'`, mở `WinFreeUp.exe`.
  Mong đợi: UAC hỏi ngay khi mở exe (không lộ bất kỳ nội dung nào trước khi có quyền Admin); sau khi đồng ý,
  hộp thoại tiếng Việt báo cần WebView2 Runtime kèm link tải, không có màn trắng.

- [ ] **SB-15 — Nâng quyền bằng tài khoản admin khác.** *(không chặn phát hành — xác nhận hành vi đã biết, không phải lỗi)*
  Bước làm: Khi UAC hỏi, chọn nhập mật khẩu một tài khoản Admin KHÁC với tài khoản đang đăng nhập; để ứng
  dụng dọn `user_temp` và `recycle_bin`.
  Mong đợi: ứng dụng dọn Temp/Thùng rác của tài khoản Admin vừa nhập mật khẩu, KHÔNG phải của người đang
  ngồi máy — đây là giới hạn đã biết của mô hình UAC, ghi nhận đúng hành vi chứ không phải lỗi cần sửa.

- [ ] **SB-16 — Sự kiện tiến độ vẫn tới giao diện sau khi thu hẹp quyền Tauri.** *(CHẶN PHÁT HÀNH nếu chưa chạy)*
  Bước làm: Mở `WinFreeUp.exe`, bấm Quét, rồi Dọn vài nhóm An toàn.
  Mong đợi: màn Đang quét hiện từng nhóm xong dần (sự kiện `scan-progress`), màn Đang dọn chuyển trạng thái
  từng nhóm và thanh % của DISM chạy (sự kiện `clean-progress`). Nếu màn đứng yên tới cuối rồi mới nhảy kết
  quả ⇒ quyền `core:event:allow-listen` trong `src-tauri/capabilities/default.json` chưa đủ.

- [ ] **SB-17 — Không dọn được nhóm chưa quét lại.** *(không chặn phát hành)*
  Bước làm: Quét, Dọn một nhóm; không quét lại, gọi lại lệnh dọn cùng nhóm (vd qua DevTools của bản dev).
  Mong đợi: nhóm đó báo câu tiếng Việt «Chưa quét lại — bỏ qua để tránh xóa nhầm…», không xóa gì; quét lại
  thì dọn bình thường.

Ghi kết quả từng mục SB (đạt/không đạt + ảnh chụp) vào PR của bản phát hành, cùng chỗ với bảng cơ bản.
