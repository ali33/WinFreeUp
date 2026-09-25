# WinFreeUp

Công cụ dọn máy Windows cho người dùng phổ thông: một file `.exe` chạy liền, quyền Admin,
Tauri 2 + React + TypeScript + Fluent UI React v9, lõi Rust `winfreeup-core`. Giao diện tiếng Việt.

## Lộ trình

| # | Dự án con | Spec |
|---|---|---|
| 1 | Dọn ổ đĩa (v0.1) | `docs/superpowers/specs/2026-09-25-winfreeup-don-o-dia-design.md` |
| 2 | Khám máy | `docs/superpowers/specs/2026-09-25-winfreeup-kham-may-design.md` |
| 3 | Tinh chỉnh | `docs/superpowers/specs/2026-09-25-winfreeup-tinh-chinh-design.md` |

Kế hoạch triển khai nằm ở `docs/superpowers/plans/`. Dự án con sau dùng lại lõi của dự án con trước,
nên làm theo đúng thứ tự trên.

## Quy trình thi công

Kế hoạch triển khai viết xong ⇒ **thi công ngay, không chờ duyệt lại**, chia **nhiều đội song song**.

1. **Chia đợt theo phụ thuộc.** Đọc mục Interfaces (Consumes/Produces) của từng task để xếp đợt.
   Đợt 0 là khung dự án và các kiểu dùng chung — **một đội**, xong mới mở đợt sau.
2. **Mỗi đội một git worktree, một nhánh riêng** (`superpowers:using-git-worktrees`), nhánh đặt tên
   `task/<số>-<tên-ngắn>`, tách từ nhánh tính năng hiện hành (vd `feat/v0.1-don-o-dia`).
   Không bao giờ để hai đội sửa cùng một worktree.
3. **Không giao hai task sửa cùng một file vào cùng một đợt.** Nếu kế hoạch buộc hai task chạm
   chung file (vd cùng đăng ký lệnh Tauri, cùng thêm chuỗi vào `vi.json`) thì xếp chúng vào hai đợt
   khác nhau, hoặc gom về một đội.
4. **Mỗi task một lượt rà riêng** (reviewer khác người làm) trước khi gộp. Rà không qua ⇒ trả về
   đúng đội đó sửa.
5. **Gộp vào nhánh tính năng**, rồi chạy **toàn bộ** `cargo test` + Vitest trên nhánh đã gộp.
   Đỏ ⇒ sửa xong mới mở đợt kế tiếp. Gộp xong thì xoá worktree và nhánh task.
6. **Dừng và hỏi người dùng** trước khi: push hoặc merge vào `main`, cài công cụ hệ thống
   **ngoài** danh sách dưới, hoặc gặp quyết định nghiệp vụ mà spec chưa trả lời.
7. **Được phép cài/nâng cấp không cần hỏi** (người dùng cho phép 2026-09-25): Rust toolchain
   (`rustup`, `cargo`), Node.js/npm, Tauri CLI, cùng gói npm và crate mà kế hoạch yêu cầu. Máy lúc đó
   đã có Rust 1.90.0, Node 22.14.0, Tauri CLI 2.11.4, MSVC Build Tools.

Vì sao: người dùng muốn tiến độ nhanh nên chạy song song; worktree riêng để các đội không ghi đè
nhau; kiểm lại toàn bộ sau mỗi đợt vì từng task qua test riêng chưa có nghĩa là gộp lại vẫn chạy.

## Commit

- Tiền tố lệnh bằng `rtk` nếu có (`rtk git commit`…).
- Commit message kết thúc bằng: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`
