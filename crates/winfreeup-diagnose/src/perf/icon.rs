//! Tên thân thiện (`FileDescription`) và biểu tượng của một exe cho bảng ứng dụng.
use std::path::Path;

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, HBITMAP,
};
use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows_sys::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

use crate::util::{from_wide, wide};

/// `FileDescription` trong tài nguyên phiên bản của exe (vd «Google Chrome»); không có ⇒ chuỗi rỗng.
pub fn file_description(path: &Path) -> String {
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
    let query = |sub: &str| -> Option<(*const u8, u32)> {
        let q = wide(sub);
        let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        // SAFETY: data là khối phiên bản do GetFileVersionInfoW ghi; ptr trả về trỏ vào bên trong data.
        let ok = unsafe { VerQueryValueW(data.as_ptr().cast(), q.as_ptr(), &mut ptr, &mut len) };
        (ok != 0 && !ptr.is_null() && len > 0).then_some((ptr as *const u8, len))
    };
    let mut langs = Vec::new();
    if let Some((p, len)) = query(r"\VarFileInfo\Translation") {
        // SAFETY: bảng dịch là các cặp u16 dài `len` byte, nằm trong data.
        let pairs = unsafe { std::slice::from_raw_parts(p as *const u16, len as usize / 2) };
        langs.extend(pairs.chunks_exact(2).map(|c| format!("{:04x}{:04x}", c[0], c[1])));
    }
    // Nhiều exe khai sai bảng dịch: thử thêm tiếng Anh Mỹ với hai bảng mã hay gặp.
    langs.extend(["040904b0".to_string(), "040904e4".to_string()]);
    for l in langs {
        if let Some((p, len)) = query(&format!(r"\StringFileInfo\{l}\FileDescription")) {
            // SAFETY: giá trị chuỗi dài `len` ký tự UTF-16 (gồm NUL), nằm trong data.
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

/// Biểu tượng 32×32 của exe dạng `data:image/png;base64,…`; không lấy được ⇒ `None` (giao diện hiện ô trống).
pub fn icon_data_url(path: &Path) -> Option<String> {
    // SHGetFileInfo không nhận dấu `/`.
    let path = std::path::PathBuf::from(path.to_string_lossy().replace('/', "\\"));
    // Luồng riêng: SHGetFileInfo cần COM STA trên luồng gọi, không đụng chế độ COM của luồng người gọi.
    std::thread::spawn(move || unsafe {
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
    fn missing_file_gives_empty_description() {
        assert_eq!(file_description(Path::new(r"C:\khong-co\x.exe")), "");
    }
}
