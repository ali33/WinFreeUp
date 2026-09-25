//! Tên thân thiện (`FileDescription`) và biểu tượng của một exe cho bảng ứng dụng.
//!
//! Chỉ đọc exe trên ổ CỤC BỘ: đường dẫn lấy từ tiến trình đang chạy, mà tiến trình có thể chạy từ ổ mạng
//! (UNC, WebDAV). Mở file ở đó từ một tiến trình Admin là gửi thông tin xác thực ra ngoài và có thể treo
//! cả luồng lấy mẫu. Không cục bộ ⇒ giao diện dùng tên file và biểu tượng mặc định.
use std::path::{Component, Path, Prefix};

use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, HBITMAP,
};
use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, PrivateExtractIconsW, HICON, ICONINFO};

use crate::util::{from_wide, wide};

/// `GetDriveTypeW`: ổ tháo rời được / ổ cố định (hằng của `Win32_System_WindowsProgramming`, feature chưa bật).
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;

/// Đường dẫn tuyệt đối trên ổ có chữ cái (`C:\…` hoặc `\?\C:\…`) là ổ cố định hoặc tháo rời được.
/// Từ chối UNC, `\?\UNC\…`, `\.\…`, WebDAV (`\host@80\…`), đường tương đối và ổ mạng đã gán chữ cái.
/// Chỉ hỏi loại ổ của gốc `X:\`, không mở file nào.
pub(crate) fn is_local_file(path: &Path) -> bool {
    let letter = match path.components().next() {
        Some(Component::Prefix(p)) => match p.kind() {
            Prefix::Disk(l) | Prefix::VerbatimDisk(l) => l,
            _ => return false,
        },
        _ => return false,
    };
    if !path.has_root() || !letter.is_ascii_alphabetic() {
        return false;
    }
    let root = wide(format!("{}:\\", letter as char));
    // SAFETY: root là chuỗi UTF-16 kết thúc NUL.
    let kind = unsafe { GetDriveTypeW(root.as_ptr()) };
    kind == DRIVE_FIXED || kind == DRIVE_REMOVABLE
}

/// Vị trí (byte) của `ptr` trong `data`; `None` khi con trỏ nằm ngoài bộ đệm.
fn field_offset(data: &[u8], ptr: *const u8) -> Option<usize> {
    let off = (ptr as usize).checked_sub(data.as_ptr() as usize)?;
    (off < data.len()).then_some(off)
}

/// Lát `data[offset .. offset + len]`, kẹp đuôi vào biên bộ đệm; `offset` ngoài bộ đệm ⇒ `None`.
fn clamp_field(data: &[u8], offset: usize, len: usize) -> Option<&[u8]> {
    if offset >= data.len() {
        return None;
    }
    Some(&data[offset..offset.saturating_add(len).min(data.len())])
}

fn le_u16s(bytes: &[u8]) -> Vec<u16> {
    bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
}

/// Bảng `\VarFileInfo\Translation`: các cặp (ngôn ngữ, bảng mã) u16 ⇒ `"040904b0"`.
fn translations(bytes: &[u8]) -> Vec<String> {
    le_u16s(bytes).chunks_exact(2).map(|c| format!("{:04x}{:04x}", c[0], c[1])).collect()
}

/// Chuỗi UTF-16 LE tới NUL đầu tiên (hoặc hết lát).
fn utf16_text(bytes: &[u8]) -> String {
    from_wide(&le_u16s(bytes))
}

/// `FileDescription` trong tài nguyên phiên bản của exe (vd «Google Chrome»); không có hoặc không phải
/// file cục bộ ⇒ chuỗi rỗng.
pub fn file_description(path: &Path) -> String {
    if !is_local_file(path) {
        return String::new();
    }
    let w = wide(path);
    // SAFETY: w là chuỗi UTF-16 kết thúc NUL; tham số handle được phép null.
    let size = unsafe { GetFileVersionInfoSizeW(w.as_ptr(), std::ptr::null_mut()) };
    if size == 0 {
        return String::new();
    }
    let mut data = vec![0u8; size as usize];
    // SAFETY: data có đúng `size` byte khả ghi.
    if unsafe { GetFileVersionInfoW(w.as_ptr(), 0, size, data.as_mut_ptr().cast()) } == 0 {
        return String::new();
    }
    // Trả lát đã kẹp trong `data`: không tin độ dài/con trỏ VerQueryValueW trả về (file có thể cố tình hỏng).
    let query = |sub: &str, len_unit: usize| -> Option<&[u8]> {
        let q = wide(sub);
        let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        // SAFETY: data là khối phiên bản do GetFileVersionInfoW ghi; q kết thúc NUL; ptr/len là biến cục bộ.
        let ok = unsafe { VerQueryValueW(data.as_ptr().cast(), q.as_ptr(), &mut ptr, &mut len) };
        if ok == 0 || ptr.is_null() || len == 0 {
            return None;
        }
        clamp_field(&data, field_offset(&data, ptr as *const u8)?, (len as usize).saturating_mul(len_unit))
    };
    // Translation: `len` tính bằng byte. Chuỗi: `len` tính bằng ký tự UTF-16.
    let mut langs = query(r"\VarFileInfo\Translation", 1).map(translations).unwrap_or_default();
    // Nhiều exe khai sai bảng dịch: thử thêm tiếng Anh Mỹ với hai bảng mã hay gặp.
    langs.extend(["040904b0".to_string(), "040904e4".to_string()]);
    for l in langs {
        if let Some(bytes) = query(&format!(r"\StringFileInfo\{l}\FileDescription"), 2) {
            let s = utf16_text(bytes);
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
        let a = if has_alpha {
            p[3]
        } else if mask_opaque.is_some_and(|m| m.get(i).copied().unwrap_or(true)) {
            255
        } else {
            0
        };
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
///
/// # Safety
/// `hbm` phải là bitmap GDI hợp lệ (hoặc null — khi đó trả `None`).
unsafe fn read_bitmap(hbm: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
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
    if dc.is_null() {
        return None;
    }
    let lines = GetDIBits(dc, hbm, 0, h as u32, px.as_mut_ptr().cast(), &mut info, DIB_RGB_COLORS);
    DeleteDC(dc);
    (lines == h).then_some((w as u32, h as u32, px))
}

/// Biểu tượng 32×32 của exe dạng `data:image/png;base64,…`; không lấy được, không phải `.exe` hoặc không
/// phải file cục bộ ⇒ `None` (giao diện hiện biểu tượng mặc định).
///
/// Dùng `PrivateExtractIconsW` (đọc thẳng tài nguyên icon của exe) thay `SHGetFileInfoW`: không nạp phần
/// mở rộng Shell của bên thứ ba vào tiến trình Admin, không cần COM.
pub fn icon_data_url(path: &Path) -> Option<String> {
    if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) || !is_local_file(path) {
        return None;
    }
    let w = wide(path);
    let mut icon: HICON = std::ptr::null_mut();
    let mut id = 0u32;
    // SAFETY: w kết thúc NUL; xin đúng 1 icon vào `icon`/`id` là biến cục bộ.
    let n = unsafe { PrivateExtractIconsW(w.as_ptr(), 0, 32, 32, &mut icon, &mut id, 1, 0) };
    if n == 0 || n == u32::MAX || icon.is_null() {
        return None;
    }
    // SAFETY: icon là HICON hợp lệ do API vừa tạo, hủy đúng một lần; hai bitmap của ICONINFO thuộc về ta.
    unsafe {
        let mut ii: ICONINFO = std::mem::zeroed();
        let ok = GetIconInfo(icon, &mut ii);
        DestroyIcon(icon);
        if ok == 0 {
            return None;
        }
        let color = read_bitmap(ii.hbmColor);
        let mask = read_bitmap(ii.hbmMask);
        if !ii.hbmColor.is_null() {
            DeleteObject(ii.hbmColor);
        }
        if !ii.hbmMask.is_null() {
            DeleteObject(ii.hbmMask);
        }
        let (w, h, px) = color?;
        // Mặt nạ AND: điểm đen (0) là phần hình, trắng là nền trong suốt.
        let opaque: Option<Vec<bool>> = mask.map(|(_, _, m)| m.chunks_exact(4).map(|p| p[0] == 0).collect());
        let png = bgra_to_png(w, h, &px, opaque.as_deref())?;
        Some(format!("data:image/png;base64,{}", base64(&png)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notepad() -> std::path::PathBuf {
        crate::util::system_dir().unwrap().join("notepad.exe")
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
    fn only_local_drive_paths_are_read() {
        assert!(is_local_file(&notepad()));
        for p in [
            r"\\127.0.0.1\c$\Windows\notepad.exe",
            r"\\?\UNC\127.0.0.1\c$\Windows\notepad.exe",
            r"\\.\C:\Windows\notepad.exe",
            r"\\host@80\share\x.exe",
            r"Windows\notepad.exe",
        ] {
            assert!(!is_local_file(Path::new(p)), "{p}");
        }
        let verbatim = format!(r"\\?\{}", notepad().display());
        assert!(is_local_file(Path::new(&verbatim)));
    }

    #[test]
    fn network_paths_give_nothing_without_touching_the_network() {
        let unc = Path::new(r"\\127.0.0.1\c$\Windows\notepad.exe");
        let t = std::time::Instant::now();
        assert_eq!(file_description(unc), "");
        assert_eq!(icon_data_url(unc), None);
        assert!(t.elapsed() < std::time::Duration::from_millis(500), "không được đi ra mạng");
    }

    #[test]
    fn icons_only_for_exe_files() {
        let ini = crate::util::windows_dir().unwrap().join("win.ini");
        assert_eq!(icon_data_url(&ini), None);
    }

    #[test]
    fn version_fields_are_clamped_to_the_buffer() {
        let data = [1u8, 2, 3, 4, 5, 6];
        assert_eq!(clamp_field(&data, 2, 2), Some(&data[2..4]));
        assert_eq!(clamp_field(&data, 4, 100), Some(&data[4..6]), "độ dài vượt biên bị kẹp");
        assert_eq!(clamp_field(&data, 6, 1), None, "offset ở ngoài bộ đệm");
        assert_eq!(clamp_field(&data, usize::MAX, 2), None);
        assert_eq!(field_offset(&data, data.as_ptr().wrapping_add(3)), Some(3));
        assert_eq!(field_offset(&data, data.as_ptr().wrapping_add(6)), None);
        assert_eq!(field_offset(&data, data.as_ptr().wrapping_sub(1)), None);
    }

    #[test]
    fn translation_and_text_parse_from_bytes() {
        // Cặp (0x0409, 0x04B0) + nửa cặp lẻ bị bỏ.
        assert_eq!(translations(&[0x09, 0x04, 0xB0, 0x04, 0x07]), vec!["040904b0".to_string()]);
        let mut text: Vec<u8> = "Notepad".encode_utf16().flat_map(u16::to_le_bytes).collect();
        text.extend_from_slice(&[0, 0, b'x', 0]);
        assert_eq!(utf16_text(&text), "Notepad");
        assert_eq!(utf16_text(&[b'A', 0, b'B']), "A", "byte lẻ cuối bị bỏ");
    }

    #[test]
    fn missing_file_gives_empty_description() {
        assert_eq!(file_description(Path::new(r"C:\khong-co\x.exe")), "");
    }
}
